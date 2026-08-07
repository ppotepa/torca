package com.torca.app

import android.content.Intent
import android.os.Build
import android.os.Bundle
import com.torca.host.AndroidKeystoreBridge
import com.torca.host.TorcaForegroundService
import io.flutter.embedding.android.FlutterActivity

/** Thin Android activity; product workflows live in the shared Flutter/Rust client. */
class MainActivity : FlutterActivity() {
    companion object {
        init {
            System.loadLibrary("torca_bridge")
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        AndroidKeystoreBridge.initialize(applicationContext)
        val service = Intent(applicationContext, TorcaForegroundService::class.java)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            applicationContext.startForegroundService(service)
        } else {
            applicationContext.startService(service)
        }
        super.onCreate(savedInstanceState)
    }
}
