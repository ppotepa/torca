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
import com.torca.app.MainActivity
import org.json.JSONObject

/** Process-level owner for the Rust Torca runtime independent from FlutterActivity recreation. */
class TorcaForegroundService : Service() {
    private val handler = Handler(Looper.getMainLooper())
    private val knownInboundMessages = HashSet<String>()
    private var notificationSnapshotSeeded = false
    private val notificationPoller = object : Runnable {
        override fun run() {
            pollMessageNotifications()
            handler.postDelayed(this, NOTIFICATION_POLL_MS)
        }
    }

    override fun onCreate() {
        super.onCreate()
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
        check(NativeRuntimeBridge.nativeEnsureRuntime()) {
            "Unable to initialize Torca process runtime"
        }
        pollMessageNotifications()
        handler.postDelayed(notificationPoller, NOTIFICATION_POLL_MS)
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int = START_STICKY
    override fun onBind(intent: Intent?): IBinder? = null

    override fun onDestroy() {
        handler.removeCallbacks(notificationPoller)
        super.onDestroy()
    }

    private fun pollMessageNotifications() {
        val raw = NativeRuntimeBridge.nativeNotificationSnapshotJson() ?: return
        val snapshot = try { JSONObject(raw) } catch (_: Exception) { return }
        val contactNames = HashMap<String, String>()
        val contacts = snapshot.optJSONArray("contacts")
        if (contacts != null) {
            for (index in 0 until contacts.length()) {
                val contact = contacts.optJSONObject(index) ?: continue
                val id = contact.optString("id")
                val name = contact.optString("displayName", "Torca contact")
                if (id.isNotEmpty()) contactNames[id] = name
            }
        }
        val conversationContacts = HashMap<String, String>()
        val conversations = snapshot.optJSONArray("conversations")
        if (conversations != null) {
            for (index in 0 until conversations.length()) {
                val conversation = conversations.optJSONObject(index) ?: continue
                val id = conversation.optString("id")
                val contactId = conversation.optString("contactId")
                if (id.isNotEmpty() && contactId.isNotEmpty()) {
                    conversationContacts[id] = contactId
                }
            }
        }
        val newMessages = ArrayList<Triple<String, String, String>>()
        val messages = snapshot.optJSONArray("messages")
        if (messages != null) {
            for (index in 0 until messages.length()) {
                val message = messages.optJSONObject(index) ?: continue
                if (message.optString("direction") != "inbound") continue
                val messageId = message.optString("id")
                val conversationId = message.optString("conversationId")
                if (messageId.isEmpty() || conversationId.isEmpty()) continue
                if (!knownInboundMessages.add(messageId)) continue
                val contactId = conversationContacts[conversationId]
                val displayName = contactId?.let(contactNames::get) ?: "Torca contact"
                newMessages.add(Triple(messageId, conversationId, displayName))
            }
        }
        if (!notificationSnapshotSeeded) {
            notificationSnapshotSeeded = true
            return
        }
        if (!messageNotificationsEnabled() || MainActivity.isVisible) return
        for ((messageId, conversationId, displayName) in newMessages) {
            showMessageNotification(messageId, conversationId, displayName)
        }
    }

    private fun messageNotificationsEnabled(): Boolean =
        getSharedPreferences(MainActivity.NOTIFICATION_PREFERENCES, MODE_PRIVATE)
            .getBoolean(MainActivity.NOTIFICATION_ENABLED, true)

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
        const val NOTIFICATION_POLL_MS = 1500L
    }
}

object NativeRuntimeBridge {
    init { System.loadLibrary("torca_bridge") }
    @JvmStatic external fun nativeEnsureRuntime(): Boolean
    @JvmStatic external fun nativeNotificationSnapshotJson(): String?
    @JvmStatic external fun nativeShutdownRuntime()
}
