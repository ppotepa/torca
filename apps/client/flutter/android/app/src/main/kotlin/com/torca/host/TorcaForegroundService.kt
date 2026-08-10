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
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkRequest
import android.os.Build
import android.os.Handler
import android.os.HandlerThread
import android.os.IBinder
import android.util.Log
import com.torca.app.MainActivity
import org.json.JSONObject

/** Process-level owner for the Rust Torca runtime independent from FlutterActivity recreation. */
class TorcaForegroundService : Service() {
    // Reading the native event cursor can briefly contend with the runtime actor.
    // It must never run on Android's main looper: doing so caused visible UI stalls
    // while the foreground service was polling for notifications.
    private val notificationThread = HandlerThread("TorcaNotificationPoller")
    private lateinit var notificationHandler: Handler
    private var notificationCursor = 0L
    private var notificationRuntimeId = ""
    private lateinit var connectivityManager: ConnectivityManager
    private val networkCallback = object : ConnectivityManager.NetworkCallback() {
        override fun onAvailable(network: Network) = notifyNetworkChanged()
        override fun onLost(network: Network) = notifyNetworkChanged()
        override fun onCapabilitiesChanged(
            network: Network,
            capabilities: android.net.NetworkCapabilities,
        ) = notifyNetworkChanged()
    }
    private val notificationPoller = object : Runnable {
        override fun run() {
            if (!NativeRuntimeBridge.nativeRuntimeAvailable()) {
                notificationHandler.postDelayed(this, RUNTIME_WAIT_MS)
                return
            }
            pollMessageNotifications()
            notificationHandler.postDelayed(this, NOTIFICATION_POLL_MS)
        }
    }

    override fun onCreate() {
        super.onCreate()
        notificationThread.start()
        notificationHandler = Handler(notificationThread.looper)
        notificationCursor = getSharedPreferences(NOTIFICATION_CURSOR_PREFERENCES, MODE_PRIVATE)
            .getLong(NOTIFICATION_CURSOR, 0L)
        notificationRuntimeId = getSharedPreferences(NOTIFICATION_CURSOR_PREFERENCES, MODE_PRIVATE)
            .getString(NOTIFICATION_RUNTIME_ID, "") ?: ""
        AndroidKeystoreBridge.initialize(applicationContext)
        connectivityManager = getSystemService(ConnectivityManager::class.java)
        runCatching {
            connectivityManager.registerNetworkCallback(
                NetworkRequest.Builder()
                    .addCapability(android.net.NetworkCapabilities.NET_CAPABILITY_INTERNET)
                    .build(),
                networkCallback,
            )
        }.onFailure { Log.w(TAG, "Could not register network callback", it) }
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
                } else {
                    // Re-arm relay probing against the network that actually
                    // exists after service startup (Wi-Fi may have changed
                    // while the process was being restored).
                    notifyNetworkChanged()
                }
            } catch (error: Throwable) {
                Log.e(TAG, "Native Torca runtime startup failed", error)
            }
        }.start()
        notificationHandler.post(notificationPoller)
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int = START_STICKY
    override fun onBind(intent: Intent?): IBinder? = null

    override fun onDestroy() {
        runCatching { connectivityManager.unregisterNetworkCallback(networkCallback) }
        notificationHandler.removeCallbacks(notificationPoller)
        notificationThread.quitSafely()
        super.onDestroy()
    }

    private fun notifyNetworkChanged() {
        if (NativeRuntimeBridge.nativeRuntimeAvailable()) {
            NativeRuntimeBridge.nativeLifecycleEvent("network_changed")
        }
    }

    private fun pollMessageNotifications() {
        val raw = NativeRuntimeBridge.nativeNotificationSnapshotJson(notificationCursor) ?: return
        val snapshot = try { JSONObject(raw) } catch (_: Exception) { return }
        val runtimeId = snapshot.optString("runtimeId")
        if (runtimeId.isNotEmpty() && runtimeId != notificationRuntimeId) {
            // Native notification cursors are scoped to one process runtime. Retaining a cursor
            // from a previous runtime would otherwise discard every new event until it exceeded
            // the old value.
            notificationRuntimeId = runtimeId
            notificationCursor = 0L
            getSharedPreferences(NOTIFICATION_CURSOR_PREFERENCES, MODE_PRIVATE)
                .edit()
                .putString(NOTIFICATION_RUNTIME_ID, runtimeId)
                .putLong(NOTIFICATION_CURSOR, notificationCursor)
                .apply()
            return
        }
        val events = snapshot.optJSONArray("events") ?: return
        val newEvents = ArrayList<RuntimeNotificationEvent>()
        for (index in 0 until events.length()) {
            val event = events.optJSONObject(index) ?: continue
            val cursor = event.optLong("cursor", 0L)
            if (cursor <= notificationCursor) continue
            notificationCursor = maxOf(notificationCursor, cursor)
            getSharedPreferences(NOTIFICATION_CURSOR_PREFERENCES, MODE_PRIVATE)
                .edit().putLong(NOTIFICATION_CURSOR, notificationCursor).apply()
            val eventId = event.optString("eventId")
            val conversationId = event.optString("conversationId")
            val kind = event.optString("kind")
            if (eventId.isNotEmpty() && conversationId.isNotEmpty()) {
                newEvents.add(
                    RuntimeNotificationEvent(
                        eventId = eventId,
                        conversationId = conversationId,
                        contactDisplayName = event.optString("contactDisplayName", "Torca contact"),
                        kind = kind,
                    ),
                )
            }
        }
        if (MainActivity.isVisible) return
        for (event in newEvents) {
            showRuntimeNotification(event)
        }
    }

    private fun showRuntimeNotification(event: RuntimeNotificationEvent) {
        if (Build.VERSION.SDK_INT >= 33 &&
            checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED
        ) return
        val pendingIntent = PendingIntent.getActivity(
            this,
            event.eventId.hashCode(),
            Intent(this, MainActivity::class.java).apply {
                flags = Intent.FLAG_ACTIVITY_SINGLE_TOP or Intent.FLAG_ACTIVITY_CLEAR_TOP
                putExtra(MainActivity.EXTRA_CONVERSATION_ID, event.conversationId)
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
            .setContentTitle(event.contactDisplayName)
            .setContentText(
                if (event.kind == "contact_added") "New contact added" else "New private message",
            )
            .setCategory(
                if (event.kind == "contact_added") Notification.CATEGORY_SOCIAL else Notification.CATEGORY_MESSAGE,
            )
            .setAutoCancel(true)
            .setOnlyAlertOnce(true)
            .setContentIntent(pendingIntent)
            .build()
        getSystemService(NotificationManager::class.java)
            .notify(event.eventId.hashCode(), notification)
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
        const val NOTIFICATION_RUNTIME_ID = "runtime_id"
        const val NOTIFICATION_POLL_MS = 1500L
        const val RUNTIME_WAIT_MS = 250L
        const val TAG = "TorcaRuntime"
    }

    private data class RuntimeNotificationEvent(
        val eventId: String,
        val conversationId: String,
        val contactDisplayName: String,
        val kind: String,
    )
}

object NativeRuntimeBridge {
    init { System.loadLibrary("torca_native") }
    @JvmStatic external fun nativeEnsureRuntime(): Boolean
    @JvmStatic external fun nativeRuntimeAvailable(): Boolean
    @JvmStatic external fun nativeLifecycleEvent(event: String): Boolean
    @JvmStatic external fun nativeNotificationSnapshotJson(afterCursor: Long): String?
}
