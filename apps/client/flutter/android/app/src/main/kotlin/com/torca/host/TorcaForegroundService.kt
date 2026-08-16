package com.torca.host

import android.Manifest
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Intent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.IntentFilter
import android.content.pm.PackageManager
import android.content.pm.ServiceInfo
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkRequest
import android.os.Build
import android.os.Handler
import android.os.HandlerThread
import android.os.IBinder
import android.os.PowerManager
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
    // This is the native runtime revision, distinct from the durable
    // notification cursor.  The service blocks on the revision and only
    // fetches notifications after a real runtime change.
    private var runtimeRevision = 0L
    private lateinit var connectivityManager: ConnectivityManager
    private var warmupWakeLock: PowerManager.WakeLock? = null
    private val energyReceiver = object : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) {
            val event = when (intent.action) {
                PowerManager.ACTION_POWER_SAVE_MODE_CHANGED -> {
                    val saver = getSystemService(PowerManager::class.java)?.isPowerSaveMode == true
                    if (saver) "power_saver_on" else "power_saver_off"
                }
                Intent.ACTION_POWER_CONNECTED -> "charging_on"
                Intent.ACTION_POWER_DISCONNECTED -> "charging_off"
                else -> return
            }
            if (NativeRuntimeBridge.nativeRuntimeAvailable()) {
                NativeRuntimeBridge.nativeLifecycleEvent(event)
            }
        }
    }
    private var networkChangePending = false
    private val networkLock = Any()
    private var defaultNetwork: Network? = null
    private var defaultNetworkFingerprint: NetworkFingerprint? = null
    private var lastMetered: Boolean? = null
    private var lastValidated: Boolean? = null
    private val networkChangeRunnable = Runnable {
        networkChangePending = false
        if (NativeRuntimeBridge.nativeRuntimeAvailable()) {
            NativeRuntimeBridge.nativeLifecycleEvent("network_changed")
        }
    }
    private val networkCallback = object : ConnectivityManager.NetworkCallback() {
        override fun onAvailable(network: Network) = observeAvailableNetwork(network)
        override fun onLost(network: Network) = observeLostNetwork(network)
        override fun onCapabilitiesChanged(
            network: Network,
            capabilities: android.net.NetworkCapabilities,
        ) = observeCapabilities(network, capabilities)
    }
    private val notificationPoller = object : Runnable {
        override fun run() {
            if (!NativeRuntimeBridge.nativeRuntimeAvailable()) {
                notificationHandler.postDelayed(this, RUNTIME_WAIT_MS)
                return
            }
            val waitResult = NativeRuntimeBridge.nativeWaitForRevision(
                runtimeRevision,
                notificationCursor,
                EVENT_WAIT_TIMEOUT_MS,
            )
            if (waitResult < 0) {
                // A transient native restart should not strand the service;
                // retry at a bounded low-frequency fallback interval.
                notificationHandler.postDelayed(this, RUNTIME_WAIT_MS)
                return
            }
            runtimeRevision = NativeRuntimeBridge.nativeRuntimeRevision().coerceAtLeast(runtimeRevision)
            pollMessageNotifications()
            // A timeout simply re-enters the blocking wait. No periodic
            // notification query is performed when the runtime is idle.
            notificationHandler.post(this)
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
            val filter = IntentFilter().apply {
                addAction(PowerManager.ACTION_POWER_SAVE_MODE_CHANGED)
                addAction(Intent.ACTION_POWER_CONNECTED)
                addAction(Intent.ACTION_POWER_DISCONNECTED)
            }
            if (Build.VERSION.SDK_INT >= 33) {
                registerReceiver(energyReceiver, filter, Context.RECEIVER_NOT_EXPORTED)
            } else {
                @Suppress("DEPRECATION")
                registerReceiver(energyReceiver, filter)
            }
            val powerManager = getSystemService(PowerManager::class.java)
            if (powerManager?.isPowerSaveMode == true) {
                NativeRuntimeBridge.nativeLifecycleEvent("power_saver_on")
            }
        }.onFailure { Log.w(TAG, "Could not register energy callbacks", it) }
        runCatching {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) {
                connectivityManager.registerDefaultNetworkCallback(networkCallback)
            } else {
                connectivityManager.registerNetworkCallback(
                    NetworkRequest.Builder()
                        .addCapability(android.net.NetworkCapabilities.NET_CAPABILITY_INTERNET)
                        .build(),
                    networkCallback,
                )
            }
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
            warmupWakeLock = getSystemService(PowerManager::class.java)?.newWakeLock(
                PowerManager.PARTIAL_WAKE_LOCK,
                "Torca::warmup",
            )?.apply {
                setReferenceCounted(false)
                acquire(WARMUP_WAKELOCK_MS)
            }
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
            } finally {
                warmupWakeLock?.let { lock -> if (lock.isHeld) lock.release() }
                warmupWakeLock = null
            }
        }.start()
        notificationHandler.post(notificationPoller)
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int = START_STICKY
    override fun onBind(intent: Intent?): IBinder? = null

    override fun onDestroy() {
        warmupWakeLock?.let { lock -> if (lock.isHeld) lock.release() }
        warmupWakeLock = null
        runCatching { connectivityManager.unregisterNetworkCallback(networkCallback) }
        runCatching { unregisterReceiver(energyReceiver) }
        notificationHandler.removeCallbacks(notificationPoller)
        notificationHandler.removeCallbacks(networkChangeRunnable)
        NativeRuntimeBridge.nativeCancelRevisionWait()
        notificationThread.quitSafely()
        super.onDestroy()
    }

    private fun notifyNetworkChanged() {
        if (networkChangePending) return
        networkChangePending = true
        notificationHandler.postDelayed(networkChangeRunnable, NETWORK_CHANGE_DEBOUNCE_MS)
    }

    /**
     * Android reports frequent capability updates for an unchanged default
     * route. They are useful facts but must not make the native runtime close
     * every Tor and peer stream as if Wi-Fi/LTE had changed.
     */
    private fun observeAvailableNetwork(network: Network) {
        val changed = synchronized(networkLock) {
            val changed = defaultNetwork != network
            defaultNetwork = network
            if (changed) defaultNetworkFingerprint = null
            changed
        }
        if (changed) notifyNetworkChanged()
    }

    private fun observeLostNetwork(network: Network) {
        val lostDefault = synchronized(networkLock) {
            if (defaultNetwork != network) return@synchronized false
            defaultNetwork = null
            defaultNetworkFingerprint = null
            true
        }
        if (lostDefault) notifyNetworkChanged()
    }

    private fun observeCapabilities(
        network: Network,
        capabilities: android.net.NetworkCapabilities,
    ) {
        val fingerprint = NetworkFingerprint.from(capabilities)
        val metered = !capabilities.hasCapability(
            android.net.NetworkCapabilities.NET_CAPABILITY_NOT_METERED,
        )
        val validated = capabilities.hasCapability(
            android.net.NetworkCapabilities.NET_CAPABILITY_VALIDATED,
        )
        if (metered != lastMetered) {
            lastMetered = metered
            if (NativeRuntimeBridge.nativeRuntimeAvailable()) {
                NativeRuntimeBridge.nativeLifecycleEvent(
                    if (metered) "metered_network_on" else "metered_network_off",
                )
            }
        }
        if (validated != lastValidated) {
            lastValidated = validated
            if (NativeRuntimeBridge.nativeRuntimeAvailable()) {
                NativeRuntimeBridge.nativeLifecycleEvent(
                    if (validated) "network_validated" else "network_unvalidated",
                )
            }
        }
        val routeReplaced = synchronized(networkLock) {
            when {
                defaultNetwork == null || defaultNetwork != network -> {
                    defaultNetwork = network
                    defaultNetworkFingerprint = fingerprint
                    true
                }
                else -> {
                    val changed = defaultNetworkFingerprint != fingerprint
                    defaultNetworkFingerprint = fingerprint
                    changed
                }
            }
        }
        if (routeReplaced) notifyNetworkChanged()
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
            runtimeRevision = 0L
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
            if (eventId.isNotEmpty() &&
                (conversationId.isNotEmpty() || kind == "pairing_request")
            ) {
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
                if (event.conversationId.isNotEmpty()) {
                    putExtra(MainActivity.EXTRA_CONVERSATION_ID, event.conversationId)
                }
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
                when (event.kind) {
                    "contact_added" -> "New contact added"
                    "pairing_request" -> "Tap to accept or reject the invitation"
                    else -> "New private message"
                },
            )
            .setCategory(
                if (event.kind == "message_received") {
                    Notification.CATEGORY_MESSAGE
                } else {
                    Notification.CATEGORY_SOCIAL
                },
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
        // Zero selects the native condvar wait. It returns only on a runtime
        // revision/cursor change or explicit service shutdown cancellation.
        // A bounded wait gives shutdown a deterministic upper bound and
        // avoids releasing the native runtime while JNI is still blocked.
        const val EVENT_WAIT_TIMEOUT_MS = 0
        const val RUNTIME_WAIT_MS = 250L
        const val NETWORK_CHANGE_DEBOUNCE_MS = 750L
        const val WARMUP_WAKELOCK_MS = 10 * 60 * 1000L
        const val TAG = "TorcaRuntime"
    }

    private data class RuntimeNotificationEvent(
        val eventId: String,
        val conversationId: String,
        val contactDisplayName: String,
        val kind: String,
    )

    private data class NetworkFingerprint(
        val validated: Boolean,
        val internet: Boolean,
        val metered: Boolean,
        val wifi: Boolean,
        val cellular: Boolean,
    ) {
        companion object {
            fun from(capabilities: android.net.NetworkCapabilities) = NetworkFingerprint(
                validated = capabilities.hasCapability(
                    android.net.NetworkCapabilities.NET_CAPABILITY_VALIDATED,
                ),
                internet = capabilities.hasCapability(
                    android.net.NetworkCapabilities.NET_CAPABILITY_INTERNET,
                ),
                metered = !capabilities.hasCapability(
                    android.net.NetworkCapabilities.NET_CAPABILITY_NOT_METERED,
                ),
                wifi = capabilities.hasTransport(android.net.NetworkCapabilities.TRANSPORT_WIFI),
                cellular = capabilities.hasTransport(
                    android.net.NetworkCapabilities.TRANSPORT_CELLULAR,
                ),
            )
        }
    }
}

object NativeRuntimeBridge {
    init { System.loadLibrary("torca_native") }
    @JvmStatic external fun nativeEnsureRuntime(): Boolean
    @JvmStatic external fun nativeRuntimeAvailable(): Boolean
    @JvmStatic external fun nativeLifecycleEvent(event: String): Boolean
    @JvmStatic external fun nativeNotificationSnapshotJson(afterCursor: Long): String?
    @JvmStatic external fun nativeRuntimeRevision(): Long
    @JvmStatic external fun nativeWaitForRevision(
        afterRevision: Long,
        afterCursor: Long,
        timeoutMs: Int,
    ): Int
    @JvmStatic external fun nativeCancelRevisionWait(): Int
}
