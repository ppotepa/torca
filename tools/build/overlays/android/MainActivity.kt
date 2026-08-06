package com.torca.app

import android.os.Bundle
import com.torca.host.AndroidKeystoreBridge
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
        super.onCreate(savedInstanceState)
    }
}
