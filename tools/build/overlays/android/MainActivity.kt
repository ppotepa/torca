package com.torca.app

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import android.view.WindowManager
import com.torca.host.AndroidKeystoreBridge
import com.torca.host.TorcaForegroundService
import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.MethodChannel

/** Thin Android activity; product workflows live in the shared Flutter/Rust client. */
class MainActivity : FlutterActivity() {
    private var notificationChannel: MethodChannel? = null
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
}
