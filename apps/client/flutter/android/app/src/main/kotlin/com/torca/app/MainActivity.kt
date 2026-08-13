package com.torca.app

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import android.graphics.Bitmap
import android.media.MediaMetadataRetriever
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
    private var pendingConversationId: String? = null

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
