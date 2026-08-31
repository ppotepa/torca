package com.torca.host

import android.Manifest
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.app.Service
import android.content.Intent
import android.content.IntentFilter
import android.content.pm.PackageManager
import android.content.pm.ServiceInfo
import android.net.ConnectivityManager
import android.net.ConnectivityDiagnosticsManager
import android.net.Network
import android.net.NetworkRequest
import android.os.Build
import android.os.BatteryManager
import android.os.Handler
import android.os.HandlerThread
import android.os.IBinder
import android.os.PowerManager
import android.util.Log
import java.util.concurrent.Executors
import java.util.concurrent.atomic.AtomicBoolean
import com.torca.app.MainActivity
import org.json.JSONObject

/** Process-level owner for the Rust Torca runtime independent from FlutterActivity recreation. */
class TorcaForegroundService : Service() {
    // Reading the native event cursor can briefly contend with the runtime actor.
    // It must never run on Android's main looper. The worker blocks in the
    // native event hub until a notification arrives instead of polling.
    private val notificationThread = HandlerThread("TorcaNotificationWaiter")
    private lateinit var notificationHandler: Handler
    // The notification handler intentionally blocks in nativeWaitForNotification;
    // network callbacks therefore need their own queue for debounced wakeups.
    private val networkThread = HandlerThread("TorcaNetworkEvents")
    private lateinit var networkHandler: Handler
    @Volatile private var stopping = false
    private var notificationCursor = 0L
    private var notificationRuntimeId = ""
    private lateinit var connectivityManager: ConnectivityManager
    private var connectivityDiagnosticsManager: ConnectivityDiagnosticsManager? = null
    private val connectivityDiagnosticsExecutor = Executors.newSingleThreadExecutor { runnable ->
        Thread(runnable, "TorcaConnectivityDiagnostics")
    }
    private val dataStallActive = AtomicBoolean(false)
    private var warmupWakeLock: PowerManager.WakeLock? = null
    private val networkChangePending = AtomicBoolean(false)
    private val networkLock = Any()
    private var defaultNetwork: Network? = null
    private var defaultNetworkFingerprint: NetworkFingerprint? = null
    private var lastMetered: Boolean? = null
    private var lastValidated: Boolean? = null
    private val networkChangeRunnable = Runnable {
        networkChangePending.set(false)
        if (NativeRuntimeBridge.nativeRuntimeAvailable()) {
            val accepted = NativeRuntimeBridge.nativeLifecycleEvent("network_changed")
            Log.d(TAG, "network_changed dispatched to native accepted=$accepted")
        }
    }
    private val energyReceiver = object : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) {
            val event = when (intent.action) {
                PowerManager.ACTION_POWER_SAVE_MODE_CHANGED -> {
                    if (getSystemService(PowerManager::class.java)?.isPowerSaveMode == true) {
                        "power_saver_on"
                    } else "power_saver_off"
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
    private val networkCallback = object : ConnectivityManager.NetworkCallback() {
        override fun onAvailable(network: Network) = observeAvailableNetwork(network)
        override fun onLost(network: Network) = observeLostNetwork(network)
        override fun onCapabilitiesChanged(
            network: Network,
            capabilities: android.net.NetworkCapabilities,
        ) = observeCapabilities(network, capabilities)

        // A mobile/Wi-Fi handoff can replace routes, DNS servers or the local
        // address while Android keeps the same capability set. The native
        // transport must re-probe those paths as well; the debounced notifier
        // coalesces the several callbacks normally emitted for one handoff.
        override fun onLinkPropertiesChanged(
            network: Network,
            linkProperties: android.net.LinkProperties,
        ) = observeLinkProperties(network, linkProperties)
    }
    private val connectivityDiagnosticsCallback =
        object : ConnectivityDiagnosticsManager.ConnectivityDiagnosticsCallback() {
            override fun onDataStallSuspected(
                report: ConnectivityDiagnosticsManager.DataStallReport,
            ) {
                if (NativeRuntimeBridge.nativeRuntimeAvailable() &&
                    dataStallActive.compareAndSet(false, true)
                ) {
                    NativeRuntimeBridge.nativeLifecycleEvent("data_stall_on")
                }
            }

            override fun onConnectivityReportAvailable(
                report: ConnectivityDiagnosticsManager.ConnectivityReport,
            ) {
                if (NativeRuntimeBridge.nativeRuntimeAvailable() &&
                    dataStallActive.compareAndSet(true, false)
                ) {
                    NativeRuntimeBridge.nativeLifecycleEvent("data_stall_off")
                }
            }
        }
    private val notificationWaiter = object : Runnable {
        override fun run() {
            if (stopping) return
            if (!NativeRuntimeBridge.nativeRuntimeAvailable()) {
                notificationHandler.postDelayed(this, RUNTIME_WAIT_MS)
                return
            }
            val result = NativeRuntimeBridge.nativeWaitForNotification(notificationCursor, 0)
            if (stopping) return
            if (result >= 0) {
                pollMessageNotifications()
                notificationHandler.post(this)
            } else {
                // A closed runtime generation is retried with backoff; the
                // healthy path remains fully event-driven.
                notificationHandler.postDelayed(this, RUNTIME_RETRY_MS)
            }
        }
    }

    override fun onCreate() {
        super.onCreate()
        notificationThread.start()
        notificationHandler = Handler(notificationThread.looper)
        networkThread.start()
        networkHandler = Handler(networkThread.looper)
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
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            runCatching {
                connectivityDiagnosticsManager =
                    getSystemService(ConnectivityDiagnosticsManager::class.java)
                connectivityDiagnosticsManager?.registerConnectivityDiagnosticsCallback(
                    NetworkRequest.Builder()
                        .addCapability(android.net.NetworkCapabilities.NET_CAPABILITY_INTERNET)
                        .build(),
                    connectivityDiagnosticsExecutor,
                    connectivityDiagnosticsCallback,
                )
            }.onFailure { Log.w(TAG, "Could not register connectivity diagnostics", it) }
        }
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
            .setContentText("Private messaging over Iroh is active")
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
                    // The registration-time broadcast can arrive before the
                    // native runtime exists. Replay the current state after
                    // initialization so battery policy starts with reality.
                    val powerSaver = getSystemService(PowerManager::class.java)?.isPowerSaveMode == true
                    NativeRuntimeBridge.nativeLifecycleEvent(
                        if (powerSaver) "power_saver_on" else "power_saver_off",
                    )
                    val batteryIntent = registerReceiver(
                        null,
                        IntentFilter(Intent.ACTION_BATTERY_CHANGED),
                    )
                    val batteryStatus = batteryIntent?.getIntExtra(
                        BatteryManager.EXTRA_STATUS,
                        BatteryManager.BATTERY_STATUS_UNKNOWN,
                    ) ?: BatteryManager.BATTERY_STATUS_UNKNOWN
                    NativeRuntimeBridge.nativeLifecycleEvent(
                        if (batteryStatus == BatteryManager.BATTERY_STATUS_CHARGING ||
                            batteryStatus == BatteryManager.BATTERY_STATUS_FULL
                        ) "charging_on" else "charging_off",
                    )
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
        notificationHandler.post(notificationWaiter)
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int = START_STICKY
    override fun onBind(intent: Intent?): IBinder? = null

    override fun onDestroy() {
        stopping = true
        warmupWakeLock?.let { lock -> if (lock.isHeld) lock.release() }
        warmupWakeLock = null
        runCatching { connectivityManager.unregisterNetworkCallback(networkCallback) }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            runCatching {
                connectivityDiagnosticsManager?.unregisterConnectivityDiagnosticsCallback(
                    connectivityDiagnosticsCallback,
                )
            }
        }
        connectivityDiagnosticsExecutor.shutdownNow()
        runCatching { unregisterReceiver(energyReceiver) }
        notificationHandler.removeCallbacks(notificationWaiter)
        networkHandler.removeCallbacks(networkChangeRunnable)
        runCatching { NativeRuntimeBridge.nativeCancelRevisionWait() }
        notificationThread.quitSafely()
        networkThread.quitSafely()
        super.onDestroy()
    }

    private fun notifyNetworkChanged() {
        if (!networkChangePending.compareAndSet(false, true)) return
        networkHandler.postDelayed(networkChangeRunnable, NETWORK_CHANGE_DEBOUNCE_MS)
    }

    private fun observeAvailableNetwork(network: Network) {
        val changed = synchronized(networkLock) {
            val changed = defaultNetwork != network
            defaultNetwork = network
            if (changed) defaultNetworkFingerprint = null
            changed
        }
        if (changed) {
            Log.d(TAG, "default network available id=$network")
            notifyNetworkChanged()
        }
    }

    private fun observeLostNetwork(network: Network) {
        val lostDefault = synchronized(networkLock) {
            if (defaultNetwork != network) return@synchronized false
            defaultNetwork = null
            defaultNetworkFingerprint = null
            true
        }
        if (lostDefault) {
            Log.d(TAG, "default network lost id=$network")
            notifyNetworkChanged()
        }
    }

    private fun observeCapabilities(
        network: Network,
        capabilities: android.net.NetworkCapabilities,
    ) {
        val fingerprint = NetworkFingerprint.from(capabilities)
        if (fingerprint.metered != lastMetered) {
            lastMetered = fingerprint.metered
            if (NativeRuntimeBridge.nativeRuntimeAvailable()) {
                NativeRuntimeBridge.nativeLifecycleEvent(
                    if (fingerprint.metered) "metered_network_on" else "metered_network_off",
                )
            }
        }
        if (fingerprint.validated != lastValidated) {
            lastValidated = fingerprint.validated
            if (NativeRuntimeBridge.nativeRuntimeAvailable()) {
                NativeRuntimeBridge.nativeLifecycleEvent(
                    if (fingerprint.validated) "network_validated" else "network_unvalidated",
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
        if (routeReplaced) {
            Log.d(TAG, "network capabilities changed id=$network fingerprint=$fingerprint")
            notifyNetworkChanged()
        }
    }

    private fun observeLinkProperties(
        network: Network,
        linkProperties: android.net.LinkProperties,
    ) {
        Log.d(
            TAG,
            "network link properties changed id=$network routes=${linkProperties.routes.size} " +
                "dns=${linkProperties.dnsServers.size}",
        )
        notifyNetworkChanged()
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
        const val RUNTIME_WAIT_MS = 250L
        const val RUNTIME_RETRY_MS = 5_000L
        const val NETWORK_CHANGE_DEBOUNCE_MS = 750L
        // Runtime warm-up is one-shot; cap the emergency CPU hold so a
        // stalled native initialization cannot drain the battery for minutes.
        const val WARMUP_WAKELOCK_MS = 90 * 1000L
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
                validated = capabilities.hasCapability(android.net.NetworkCapabilities.NET_CAPABILITY_VALIDATED),
                internet = capabilities.hasCapability(android.net.NetworkCapabilities.NET_CAPABILITY_INTERNET),
                metered = !capabilities.hasCapability(android.net.NetworkCapabilities.NET_CAPABILITY_NOT_METERED),
                wifi = capabilities.hasTransport(android.net.NetworkCapabilities.TRANSPORT_WIFI),
                cellular = capabilities.hasTransport(android.net.NetworkCapabilities.TRANSPORT_CELLULAR),
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
    @JvmStatic external fun nativeWaitForNotification(afterCursor: Long, timeoutMs: Int): Int
    @JvmStatic external fun nativeCancelRevisionWait(): Int
}
