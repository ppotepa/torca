package com.torca.app

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.media.AudioAttributes
import android.media.AudioFocusRequest
import android.media.AudioFormat
import android.media.AudioManager
import android.media.AudioRecord
import android.media.MediaRecorder
import android.os.Build
import android.os.Bundle
import android.provider.Settings
import android.view.WindowManager
import com.torca.host.AndroidKeystoreBridge
import com.torca.host.TorcaForegroundService
import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.MethodChannel

/** Thin Android activity; product workflows live in the shared Flutter/Rust client. */
class MainActivity : FlutterActivity() {
    private var notificationChannel: MethodChannel? = null
    private var deviceChannel: MethodChannel? = null
    private var audioChannel: MethodChannel? = null
    private var pendingMicrophoneResult: MethodChannel.Result? = null
    private var pendingConversationId: String? = null
    private var radioRecorder: AudioRecord? = null
    private var radioThread: Thread? = null
    private var radioAudioFocusRequest: AudioFocusRequest? = null
    private val radioLock = Any()

    companion object {
        const val EXTRA_CONVERSATION_ID = "torca.conversation_id"
        @Volatile var isVisible: Boolean = false
        init { System.loadLibrary("torca_native") }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        AndroidKeystoreBridge.initialize(applicationContext)
        super.onCreate(savedInstanceState)
        // Torca never exposes private conversation content to screenshots, screen recording,
        // recent-app thumbnails or OS capture surfaces from the Android activity.
        window.addFlags(WindowManager.LayoutParams.FLAG_SECURE)
        pendingConversationId = intent?.getStringExtra(EXTRA_CONVERSATION_ID)
        val service = Intent(applicationContext, TorcaForegroundService::class.java)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            applicationContext.startForegroundService(service)
        } else {
            applicationContext.startService(service)
        }
        if (Build.VERSION.SDK_INT >= 33 &&
            checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED
        ) {
            requestPermissions(arrayOf(Manifest.permission.POST_NOTIFICATIONS), 1001)
        }
    }

    override fun configureFlutterEngine(flutterEngine: FlutterEngine) {
        super.configureFlutterEngine(flutterEngine)
        notificationChannel = MethodChannel(
            flutterEngine.dartExecutor.binaryMessenger,
            "torca/notifications",
        ).also { channel ->
            channel.setMethodCallHandler { call, result ->
                when (call.method) {
                    "takeInitialConversation" -> {
                        val value = pendingConversationId
                        pendingConversationId = null
                        result.success(value)
                    }
                    else -> result.notImplemented()
                }
            }
        }
        deviceChannel = MethodChannel(
            flutterEngine.dartExecutor.binaryMessenger,
            "torca/device",
        ).also { channel ->
            channel.setMethodCallHandler { call, result ->
                when (call.method) {
                    "stableDeviceId" -> {
                        val identifier = Settings.Secure.getString(
                            contentResolver,
                            Settings.Secure.ANDROID_ID,
                        )
                        if (identifier.isNullOrBlank()) {
                            result.error(
                                "UNAVAILABLE",
                                "Android stable device identifier is unavailable",
                                null,
                            )
                        } else {
                            result.success(identifier)
                        }
                    }
                    else -> result.notImplemented()
                }
            }
        }
        audioChannel = MethodChannel(
            flutterEngine.dartExecutor.binaryMessenger,
            "torca/audio",
        ).also { channel ->
            channel.setMethodCallHandler { call, result ->
                when (call.method) {
                    "hasMicrophonePermission" -> result.success(
                        Build.VERSION.SDK_INT < Build.VERSION_CODES.M ||
                            checkSelfPermission(Manifest.permission.RECORD_AUDIO) ==
                            PackageManager.PERMISSION_GRANTED,
                    )
                    "requestMicrophonePermission" -> requestMicrophonePermission(result)
                    "setCommunicationAudioMode" -> {
                        setCommunicationAudioMode(call.argument<Boolean>("enabled") == true)
                        result.success(null)
                    }
                    "startNativeRadioCapture" -> result.success(startNativeRadioCapture())
                    "stopNativeRadioCapture" -> {
                        stopNativeRadioCapture()
                        result.success(null)
                    }
                    else -> result.notImplemented()
                }
            }
        }
    }

    private fun requestMicrophonePermission(result: MethodChannel.Result) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.M ||
            checkSelfPermission(Manifest.permission.RECORD_AUDIO) == PackageManager.PERMISSION_GRANTED
        ) {
            result.success(true)
            return
        }
        if (pendingMicrophoneResult != null) {
            result.error("BUSY", "A microphone permission request is already active", null)
            return
        }
        pendingMicrophoneResult = result
        requestPermissions(arrayOf(Manifest.permission.RECORD_AUDIO), 1002)
    }

    override fun onRequestPermissionsResult(
        requestCode: Int,
        permissions: Array<out String>,
        grantResults: IntArray,
    ) {
        if (requestCode == 1002) {
            val result = pendingMicrophoneResult
            pendingMicrophoneResult = null
            result?.success(
                grantResults.isNotEmpty() &&
                    grantResults[0] == PackageManager.PERMISSION_GRANTED,
            )
        }
        super.onRequestPermissionsResult(requestCode, permissions, grantResults)
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        val conversationId = intent.getStringExtra(EXTRA_CONVERSATION_ID) ?: return
        pendingConversationId = conversationId
        notificationChannel?.invokeMethod("openConversation", conversationId)
    }

    override fun onResume() {
        super.onResume()
        isVisible = true
    }

    override fun onPause() {
        isVisible = false
        stopNativeRadioCapture()
        super.onPause()
    }

    override fun onDestroy() {
        stopNativeRadioCapture()
        super.onDestroy()
    }

    private fun setCommunicationAudioMode(enabled: Boolean) {
        val manager = getSystemService(AudioManager::class.java) ?: return
        if (enabled) {
            manager.mode = AudioManager.MODE_IN_COMMUNICATION
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                val request = AudioFocusRequest.Builder(AudioManager.AUDIOFOCUS_GAIN_TRANSIENT_EXCLUSIVE)
                    .setAudioAttributes(
                        AudioAttributes.Builder()
                            .setUsage(AudioAttributes.USAGE_VOICE_COMMUNICATION)
                            .setContentType(AudioAttributes.CONTENT_TYPE_SPEECH)
                            .build(),
                    )
                    .setOnAudioFocusChangeListener { }
                    .build()
                radioAudioFocusRequest = request
                manager.requestAudioFocus(request)
            } else {
                @Suppress("DEPRECATION")
                manager.requestAudioFocus(
                    { },
                    AudioManager.STREAM_VOICE_CALL,
                    AudioManager.AUDIOFOCUS_GAIN_TRANSIENT_EXCLUSIVE,
                )
            }
        } else {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                radioAudioFocusRequest?.let { manager.abandonAudioFocusRequest(it) }
                radioAudioFocusRequest = null
            }
            manager.mode = AudioManager.MODE_NORMAL
        }
    }

    private fun startNativeRadioCapture(): Boolean {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M &&
            checkSelfPermission(Manifest.permission.RECORD_AUDIO) != PackageManager.PERMISSION_GRANTED
        ) return false
        synchronized(radioLock) {
            stopNativeRadioCaptureLocked()
            val minBuffer = AudioRecord.getMinBufferSize(
                8000,
                AudioFormat.CHANNEL_IN_MONO,
                AudioFormat.ENCODING_PCM_16BIT,
            )
            if (minBuffer <= 0) return false
            val bufferSize = maxOf(minBuffer, 320 * 4)
            val recorder = try {
                AudioRecord(
                    MediaRecorder.AudioSource.VOICE_COMMUNICATION,
                    8000,
                    AudioFormat.CHANNEL_IN_MONO,
                    AudioFormat.ENCODING_PCM_16BIT,
                    bufferSize,
                )
            } catch (_: Throwable) {
                return false
            }
            if (recorder.state != AudioRecord.STATE_INITIALIZED) {
                recorder.release()
                return false
            }
            radioRecorder = recorder
            try {
                recorder.startRecording()
            } catch (_: Throwable) {
                stopNativeRadioCaptureLocked()
                return false
            }
            AndroidKeystoreBridge.nativeSetRadioCaptureActive(true)
            val worker = Thread({
                val pcm = ByteArray(320 * 2)
                try {
                    while (!Thread.currentThread().isInterrupted) {
                        val count = recorder.read(pcm, 0, pcm.size, AudioRecord.READ_BLOCKING)
                        if (count > 0) {
                            AndroidKeystoreBridge.nativePushRadioPcm(pcm.copyOf(count))
                        } else if (count == AudioRecord.ERROR_DEAD_OBJECT ||
                            count == AudioRecord.ERROR_INVALID_OPERATION
                        ) {
                            break
                        }
                    }
                } catch (_: Throwable) {
                    // Native capture state is cleared below and wakes Rust.
                } finally {
                    AndroidKeystoreBridge.nativeSetRadioCaptureActive(false)
                }
            }, "TorcaRadioAudioRecord")
            radioThread = worker
            worker.start()
            return true
        }
    }

    private fun stopNativeRadioCapture() {
        synchronized(radioLock) { stopNativeRadioCaptureLocked() }
        setCommunicationAudioMode(false)
    }

    private fun stopNativeRadioCaptureLocked() {
        AndroidKeystoreBridge.nativeSetRadioCaptureActive(false)
        radioThread?.interrupt()
        radioThread = null
        radioRecorder?.let { recorder ->
            runCatching { recorder.stop() }
            recorder.release()
        }
        radioRecorder = null
    }
}
