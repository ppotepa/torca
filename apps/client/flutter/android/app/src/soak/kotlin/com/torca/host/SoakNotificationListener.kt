package com.torca.host

import android.service.notification.NotificationListenerService
import android.service.notification.StatusBarNotification
import org.json.JSONObject
import java.io.File

/**
 * SOAK-only notification oracle. It records notification metadata locally so
 * the host can assert system delivery after measurement without polling ADB
 * throughout the battery window.
 */
class SoakNotificationListener : NotificationListenerService() {
    override fun onNotificationPosted(sbn: StatusBarNotification) {
        if (sbn.packageName != packageName) return
        val notification = sbn.notification
        val extras = notification.extras
        val record = JSONObject()
            .put("schema", 1)
            .put("postedAtMs", System.currentTimeMillis())
            .put("package", sbn.packageName)
            .put("id", sbn.id)
            .put("tag", sbn.tag ?: "")
            .put("channel", if (android.os.Build.VERSION.SDK_INT >= 26) notification.channelId else "")
            .put("title", extras?.getCharSequence("android.title")?.toString() ?: "")
            .put("category", notification.category ?: "")
        append(record.toString())
    }

    private fun append(line: String) {
        runCatching {
            val directory = File(filesDir, "torca")
            directory.mkdirs()
            File(directory, "soak-notifications.jsonl").appendText(line + "\n")
        }
    }
}
