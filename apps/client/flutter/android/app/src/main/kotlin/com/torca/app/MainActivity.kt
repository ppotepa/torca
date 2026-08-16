package com.torca.app

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import android.provider.Settings
import android.graphics.Bitmap
import android.media.MediaMetadataRetriever
import android.media.AudioManager
import android.view.WindowManager
import java.io.ByteArrayOutputStream
import com.torca.host.AndroidKeystoreBridge
import com.torca.host.TorcaForegroundService
import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.MethodChannel

/** Thin Android activity; product workflows live in the shared Flutter/Rust client. */
class MainActivity : FlutterActivity() {
    private var notificationChannel: MethodChannel? = null
    private var mediaChannel: MethodChannel? = null
    private var audioChannel: MethodChannel? = null
    private var deviceChannel: MethodChannel? = null
    private var microphonePermissionResult: MethodChannel.Result? = null
    private var pendingConversationId: String? = null

    companion object {
        const val EXTRA_CONVERSATION_ID = "torca.conversation_id"
        const val EXTRA_ALLOW_SCREEN_CAPTURE = "torca.allow_screen_capture"
        private const val PRIVACY_PREFERENCES = "torca.privacy"
        private const val ALLOW_SCREEN_CAPTURE = "allow_screen_capture"
        const val REQUEST_MICROPHONE_PERMISSION = 1002
        @Volatile var isVisible: Boolean = false
        init { System.loadLibrary("torca_native") }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        AndroidKeystoreBridge.initialize(applicationContext)
        super.onCreate(savedInstanceState)
        applyScreenCapturePolicy(intent)
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
        mediaChannel = MethodChannel(
            flutterEngine.dartExecutor.binaryMessenger,
            "torca/media",
        ).also { channel ->
            channel.setMethodCallHandler { call, result ->
                if (call.method != "videoThumbnail") {
                    result.notImplemented()
                    return@setMethodCallHandler
                }
                val path = call.argument<String>("sourcePath")
                if (path.isNullOrBlank()) {
                    result.success(null)
                    return@setMethodCallHandler
                }
                Thread {
                    val preview = videoThumbnail(path)
                    runOnUiThread { result.success(preview) }
                }.start()
            }
        }
        audioChannel = MethodChannel(
            flutterEngine.dartExecutor.binaryMessenger,
            "torca/audio",
        ).also { channel ->
            channel.setMethodCallHandler { call, result ->
                when (call.method) {
                    "hasMicrophonePermission" -> result.success(
                        checkSelfPermission(Manifest.permission.RECORD_AUDIO) ==
                            PackageManager.PERMISSION_GRANTED,
                    )
                    "requestMicrophonePermission" -> {
                        if (checkSelfPermission(Manifest.permission.RECORD_AUDIO) ==
                            PackageManager.PERMISSION_GRANTED
                        ) {
                            result.success(true)
                        } else if (microphonePermissionResult != null) {
                            result.error(
                                "permission_request_active",
                                "A microphone permission request is already active.",
                                null,
                            )
                        } else {
                            microphonePermissionResult = result
                            requestPermissions(
                                arrayOf(Manifest.permission.RECORD_AUDIO),
                                REQUEST_MICROPHONE_PERMISSION,
                            )
                        }
                    }
                    "setCommunicationAudioMode" -> {
                        val enabled = call.argument<Boolean>("enabled") == true
                        val audioManager = getSystemService(AudioManager::class.java)
                        if (enabled) {
                            audioManager.mode = AudioManager.MODE_IN_COMMUNICATION
                        } else if (audioManager.mode == AudioManager.MODE_IN_COMMUNICATION) {
                            audioManager.mode = AudioManager.MODE_NORMAL
                        }
                        result.success(null)
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
                    "stableDeviceId" -> result.success(
                        Settings.Secure.getString(
                            applicationContext.contentResolver,
                            Settings.Secure.ANDROID_ID,
                        ),
                    )
                    else -> result.notImplemented()
                }
            }
        }
    }

    override fun cleanUpFlutterEngine(flutterEngine: FlutterEngine) {
        notificationChannel?.setMethodCallHandler(null)
        mediaChannel?.setMethodCallHandler(null)
        audioChannel?.setMethodCallHandler(null)
        deviceChannel?.setMethodCallHandler(null)
        microphonePermissionResult?.error(
            "activity_destroyed",
            "The microphone permission request was interrupted.",
            null,
        )
        microphonePermissionResult = null
        notificationChannel = null
        mediaChannel = null
        audioChannel = null
        deviceChannel = null
        super.cleanUpFlutterEngine(flutterEngine)
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        applyScreenCapturePolicy(intent)
        val conversationId = intent.getStringExtra(EXTRA_CONVERSATION_ID) ?: return
        pendingConversationId = conversationId
        notificationChannel?.invokeMethod("openConversation", conversationId)
    }

    private fun applyScreenCapturePolicy(source: Intent?) {
        val preferences = getSharedPreferences(PRIVACY_PREFERENCES, MODE_PRIVATE)
        if (source?.hasExtra(EXTRA_ALLOW_SCREEN_CAPTURE) == true) {
            preferences.edit()
                .putBoolean(ALLOW_SCREEN_CAPTURE, source.getBooleanExtra(EXTRA_ALLOW_SCREEN_CAPTURE, false))
                .apply()
        }
        val allowCapture = preferences.getBoolean(ALLOW_SCREEN_CAPTURE, false)
        if (allowCapture) {
            window.clearFlags(WindowManager.LayoutParams.FLAG_SECURE)
        } else {
            // Strict is the safe default: private conversation content is not exposed to
            // screenshots, screen recording, recent-app thumbnails, or OS capture surfaces.
            window.addFlags(WindowManager.LayoutParams.FLAG_SECURE)
        }
    }

    override fun onRequestPermissionsResult(
        requestCode: Int,
        permissions: Array<out String>,
        grantResults: IntArray,
    ) {
        super.onRequestPermissionsResult(requestCode, permissions, grantResults)
        if (requestCode != REQUEST_MICROPHONE_PERMISSION) return
        val granted = grantResults.firstOrNull() == PackageManager.PERMISSION_GRANTED
        microphonePermissionResult?.success(granted)
        microphonePermissionResult = null
    }

    override fun onResume() {
        super.onResume()
        isVisible = true
    }

    override fun onPause() {
        isVisible = false
        super.onPause()
    }

    private fun videoThumbnail(path: String): ByteArray? {
        val retriever = MediaMetadataRetriever()
        try {
            retriever.setDataSource(path)
            val frame = retriever.getFrameAtTime(0, MediaMetadataRetriever.OPTION_CLOSEST_SYNC)
                ?: return null
            val scaled = scaleToEdge(frame, 320)
            if (scaled !== frame) frame.recycle()
            return encodePreview(scaled)
        } catch (_: Exception) {
            return null
        } finally {
            try { retriever.release() } catch (_: Exception) { }
        }
    }

    private fun scaleToEdge(source: Bitmap, maximumEdge: Int): Bitmap {
        val longest = maxOf(source.width, source.height)
        if (longest <= maximumEdge) return source
        val ratio = maximumEdge.toFloat() / longest.toFloat()
        return Bitmap.createScaledBitmap(
            source,
            (source.width * ratio).toInt().coerceAtLeast(1),
            (source.height * ratio).toInt().coerceAtLeast(1),
            true,
        )
    }

    private fun encodePreview(bitmap: Bitmap): ByteArray? {
        try {
            for (quality in intArrayOf(74, 62, 50, 40, 32)) {
                val output = ByteArrayOutputStream()
                bitmap.compress(Bitmap.CompressFormat.JPEG, quality, output)
                val bytes = output.toByteArray()
                if (bytes.size <= 24 * 1024) return bytes
            }
            return null
        } finally {
            bitmap.recycle()
        }
    }
}
