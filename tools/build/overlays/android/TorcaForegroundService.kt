package com.torca.host

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import com.torca.app.MainActivity

/**
 * Process-level owner that keeps the Rust Torca runtime alive independently from FlutterActivity.
 * The Rust runtime itself owns Tor, peer sessions, delivery and persistence recovery.
 */
class TorcaForegroundService : Service() {
    override fun onCreate() {
        super.onCreate()
        AndroidKeystoreBridge.initialize(applicationContext)
        createChannel()
        val notification = android.app.Notification.Builder(
            this,
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) CHANNEL_ID else null,
        )
            .setSmallIcon(applicationInfo.icon)
            .setContentTitle("Torca")
            .setContentText("Private messaging over Tor is active")
            .setOngoing(true)
            .setCategory(android.app.Notification.CATEGORY_SERVICE)
            .setContentIntent(
                PendingIntent.getActivity(
                    this,
                    0,
                    Intent(this, MainActivity::class.java).apply {
                        flags = Intent.FLAG_ACTIVITY_SINGLE_TOP or Intent.FLAG_ACTIVITY_CLEAR_TOP
                    },
                    PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
                ),
            )
            .build()

        if (Build.VERSION.SDK_INT >= 34) {
            startForeground(
                NOTIFICATION_ID,
                notification,
                ServiceInfo.FOREGROUND_SERVICE_TYPE_REMOTE_MESSAGING,
            )
        } else {
            startForeground(NOTIFICATION_ID, notification)
        }

        check(NativeRuntimeBridge.nativeEnsureRuntime()) {
            "Unable to initialize Torca process runtime"
        }
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int = START_STICKY

    override fun onBind(intent: Intent?): IBinder? = null

    private fun createChannel() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
        val manager = getSystemService(NotificationManager::class.java)
        manager.createNotificationChannel(
            NotificationChannel(
                CHANNEL_ID,
                "Torca background messaging",
                NotificationManager.IMPORTANCE_LOW,
            ).apply {
                description = "Keeps private Tor messaging active while Torca is in the background"
                setShowBadge(false)
            },
        )
    }

    private companion object {
        const val CHANNEL_ID = "torca_remote_messaging"
        const val NOTIFICATION_ID = 1001
    }
}

/** JNI bridge to the process-scoped Rust runtime registry. */
object NativeRuntimeBridge {
    init {
        System.loadLibrary("torca_bridge")
    }

    @JvmStatic external fun nativeEnsureRuntime(): Boolean
    @JvmStatic external fun nativeShutdownRuntime()
}
