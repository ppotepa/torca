package com.torca.host

import io.flutter.plugin.common.BinaryMessenger
import io.flutter.plugin.common.MethodCall
import io.flutter.plugin.common.MethodChannel

/** Primitive native boundary implemented by the Rust library adapter. */
interface NativeEngine : AutoCloseable {
    fun execute(command: Map<String, Any?>): Map<String, Any?>
    fun snapshot(): Map<String, Any?>
    override fun close()
}

/** Registers the versioned Flutter channel without duplicating engine workflows in Kotlin. */
class TorcaEngineChannel(
    messenger: BinaryMessenger,
    private val engine: NativeEngine,
) : MethodChannel.MethodCallHandler, AutoCloseable {
    private val channel = MethodChannel(messenger, CHANNEL_NAME)
    private var closed = false

    init {
        channel.setMethodCallHandler(this)
    }

    override fun onMethodCall(call: MethodCall, result: MethodChannel.Result) {
        if (closed) {
            result.error("engine_closed", "Torca engine is closed", null)
            return
        }

        runCatching {
            when (call.method) {
                "snapshot" -> {
                    requireContract(call.arguments)
                    engine.snapshot().withVersion()
                }
                "execute" -> {
                    val arguments = requireArguments(call.arguments)
                    requireContract(arguments)
                    val command = arguments["command"] as? Map<*, *>
                        ?: error("command must be a map")
                    @Suppress("UNCHECKED_CAST")
                    val resultMap = engine.execute(command as Map<String, Any?>)
                    publishSnapshot()
                    resultMap
                }
                else -> throw UnsupportedOperationException(call.method)
            }
        }.onSuccess(result::success).onFailure { error ->
            when (error) {
                is UnsupportedOperationException -> result.notImplemented()
                is IllegalArgumentException,
                is IllegalStateException -> result.error(
                    "invalid_engine_request",
                    error.message ?: "Invalid native engine request",
                    null,
                )
                else -> result.error(
                    "native_engine_failure",
                    "Native Torca engine operation failed",
                    null,
                )
            }
        }
    }

    fun publishSnapshot() {
        if (!closed) {
            channel.invokeMethod("snapshotChanged", engine.snapshot().withVersion())
        }
    }

    override fun close() {
        if (closed) return
        closed = true
        channel.setMethodCallHandler(null)
        engine.close()
    }

    private fun requireContract(value: Any?) {
        requireContract(requireArguments(value))
    }

    private fun requireContract(arguments: Map<*, *>) {
        require(arguments["contractVersion"] == CONTRACT_VERSION) {
            "Unsupported Torca contract version"
        }
    }

    private fun requireArguments(value: Any?): Map<*, *> =
        value as? Map<*, *> ?: error("arguments must be a map")

    private fun Map<String, Any?>.withVersion(): Map<String, Any?> =
        if (this["contractVersion"] == CONTRACT_VERSION) this
        else this + ("contractVersion" to CONTRACT_VERSION)

    private companion object {
        const val CHANNEL_NAME = "torca.engine.v1"
        const val CONTRACT_VERSION = 1
    }
}
