package com.torca.host

import android.Manifest
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Intent
import android.content.pm.PackageManager
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.util.Log
import com.torca.app.MainActivity
import org.json.JSONObject

/** Process-level owner for the Rust Torca runtime independent from FlutterActivity recreation. */
class TorcaForegroundService : Service() {
    private val handler = Handler(Looper.getMainLooper())
    private var notificationCursor = 0L
    private val notificationPoller = object : Runnable {
        override fun run() {
            if (!NativeRuntimeBridge.nativeRuntimeAvailable()) {
                handler.postDelayed(this, RUNTIME_WAIT_MS)
                return
            }
            pollMessageNotifications()
            handler.postDelayed(this, NOTIFICATION_POLL_MS)
        }
    }

    override fun onCreate() {
        super.onCreate()
        notificationCursor = getSharedPreferences(NOTIFICATION_CURSOR_PREFERENCES, MODE_PRIVATE)
            .getLong(NOTIFICATION_CURSOR, 0L)
        AndroidKeystoreBridge.initialize(applicationContext)
        createServiceChannel()
        createMessageChannel()
        val builder = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            Notification.Builder(this, SERVICE_CHANNEL_ID)
        } else {
            @Suppress("DEPRECATION")
            Notification.Builder(this)
        }
        val notification = builder
            .setSmallIcon(applicationInfo.icon)
            .setContentTitle("Torca")
            .setContentText("Private messaging over Tor is active")
            .setOngoing(true)
            .setCategory(Notification.CATEGORY_SERVICE)
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
                SERVICE_NOTIFICATION_ID,
                notification,
                ServiceInfo.FOREGROUND_SERVICE_TYPE_REMOTE_MESSAGING,
            )
        } else {
            startForeground(SERVICE_NOTIFICATION_ID, notification)
        }
        // The service is the process-level owner of the native runtime. Flutter only
        // observes the same runtime through FFI; it must not be the sole starter,
        // otherwise a restarted foreground service can remain alive without Tor.
        Thread {
            try {
                val available = NativeRuntimeBridge.nativeEnsureRuntime()
                if (!available) {
                    Log.e(TAG, "Native Torca runtime reported unavailable")
                }
            } catch (error: Throwable) {
                Log.e(TAG, "Native Torca runtime startup failed", error)
            }
        }.start()
        handler.post(notificationPoller)
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int = START_STICKY
    override fun onBind(intent: Intent?): IBinder? = null

    override fun onDestroy() {
        handler.removeCallbacks(notificationPoller)
        super.onDestroy()
    }

    private fun pollMessageNotifications() {
        val raw = NativeRuntimeBridge.nativeNotificationSnapshotJson(notificationCursor) ?: return
        val snapshot = try { JSONObject(raw) } catch (_: Exception) { return }
        val events = snapshot.optJSONArray("events") ?: return
        val newEvents = ArrayList<Triple<String, String, String>>()
        for (index in 0 until events.length()) {
            val event = events.optJSONObject(index) ?: continue
            val cursor = event.optLong("cursor", 0L)
            if (cursor <= notificationCursor) continue
            notificationCursor = maxOf(notificationCursor, cursor)
            getSharedPreferences(NOTIFICATION_CURSOR_PREFERENCES, MODE_PRIVATE)
                .edit().putLong(NOTIFICATION_CURSOR, notificationCursor).apply()
            val eventId = event.optString("eventId")
            val conversationId = event.optString("conversationId")
            if (eventId.isNotEmpty() && conversationId.isNotEmpty()) {
                newEvents.add(Triple(eventId, conversationId, event.optString("contactDisplayName", "Torca contact")))
            }
        }
        if (MainActivity.isVisible) return
        for ((eventId, conversationId, displayName) in newEvents) {
            showMessageNotification(eventId, conversationId, displayName)
        }
    }

    private fun showMessageNotification(
        messageId: String,
        conversationId: String,
        displayName: String,
    ) {
        if (Build.VERSION.SDK_INT >= 33 &&
            checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED
        ) return
        val pendingIntent = PendingIntent.getActivity(
            this,
            messageId.hashCode(),
            Intent(this, MainActivity::class.java).apply {
                flags = Intent.FLAG_ACTIVITY_SINGLE_TOP or Intent.FLAG_ACTIVITY_CLEAR_TOP
                putExtra(MainActivity.EXTRA_CONVERSATION_ID, conversationId)
            },
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )
        val builder = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            Notification.Builder(this, MESSAGE_CHANNEL_ID)
        } else {
            @Suppress("DEPRECATION")
            Notification.Builder(this)
        }
        val notification = builder
            .setSmallIcon(applicationInfo.icon)
            .setContentTitle(displayName)
            .setContentText("New private message")
            .setCategory(Notification.CATEGORY_MESSAGE)
            .setAutoCancel(true)
            .setOnlyAlertOnce(true)
            .setContentIntent(pendingIntent)
            .build()
        getSystemService(NotificationManager::class.java)
            .notify(messageId.hashCode(), notification)
    }

    private fun createServiceChannel() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
        getSystemService(NotificationManager::class.java).createNotificationChannel(
            NotificationChannel(
                SERVICE_CHANNEL_ID,
                "Torca background messaging",
                NotificationManager.IMPORTANCE_LOW,
            ).apply {
                description = "Keeps private Tor messaging active while Torca is in the background"
                setShowBadge(false)
            },
        )
    }

    private fun createMessageChannel() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
        getSystemService(NotificationManager::class.java).createNotificationChannel(
            NotificationChannel(
                MESSAGE_CHANNEL_ID,
                "Private messages",
                NotificationManager.IMPORTANCE_DEFAULT,
            ).apply {
                description = "New private Torca message notifications"
                setShowBadge(true)
            },
        )
    }

    private companion object {
        const val SERVICE_CHANNEL_ID = "torca_remote_messaging"
        const val MESSAGE_CHANNEL_ID = "torca_private_messages"
        const val SERVICE_NOTIFICATION_ID = 1001
        const val NOTIFICATION_CURSOR_PREFERENCES = "torca_notification_cursor"
        const val NOTIFICATION_CURSOR = "cursor"
        const val NOTIFICATION_POLL_MS = 1500L
        const val RUNTIME_WAIT_MS = 250L
        const val TAG = "TorcaRuntime"
    }
}

object NativeRuntimeBridge {
    init { System.loadLibrary("torca_native") }
    @JvmStatic external fun nativeEnsureRuntime(): Boolean
    @JvmStatic external fun nativeRuntimeAvailable(): Boolean
    @JvmStatic external fun nativeNotificationSnapshotJson(afterCursor: Long): String?
}
