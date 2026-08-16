package com.torca.host

import android.content.Context
import android.media.AudioFormat
import android.media.AudioRecord
import android.media.MediaRecorder
import android.media.audiofx.AcousticEchoCanceler
import android.media.audiofx.AutomaticGainControl
import android.media.audiofx.NoiseSuppressor
import android.os.Build
import android.util.Log
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import java.io.File
import java.util.concurrent.atomic.AtomicBoolean
import kotlin.math.max
import java.nio.ByteBuffer
import java.nio.file.Files
import java.nio.file.StandardCopyOption
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

class AndroidKeystoreSecretStore(
    context: Context,
    private val namespace: String,
) {
    init {
        require(namespace.matches(Regex("[a-z][a-z0-9-]{0,31}"))) { "invalid secret namespace" }
    }

    private val root = File(context.noBackupFilesDir, "torca/protected-secrets/$namespace")
    private val keyStore = KeyStore.getInstance(KEYSTORE).apply { load(null) }

    init {
        if (!root.exists() && !root.mkdirs()) error("could not create protected secret directory")
        ensureMasterKey()
    }

    @Synchronized
    fun insert(keyId: String, secret: ByteArray) {
        validateKeyId(keyId)
        val target = file(keyId)
        check(!target.exists()) { "protected secret already exists" }
        val cipher = Cipher.getInstance(TRANSFORMATION)
        cipher.init(Cipher.ENCRYPT_MODE, masterKey())
        cipher.updateAAD(aad(keyId))
        val ciphertext = cipher.doFinal(secret)
        val nonce = cipher.iv
        val encoded = ByteBuffer.allocate(4 + nonce.size + ciphertext.size)
            .putInt(nonce.size).put(nonce).put(ciphertext).array()
        writeAtomic(target, encoded)
        encoded.fill(0)
        ciphertext.fill(0)
    }

    @Synchronized
    fun load(keyId: String): ByteArray? {
        validateKeyId(keyId)
        val target = file(keyId)
        if (!target.exists()) return null
        val encoded = target.readBytes()
        try {
            val buffer = ByteBuffer.wrap(encoded)
            if (buffer.remaining() < 4) error("protected secret is malformed")
            val nonceLength = buffer.int
            if (nonceLength !in 12..32 || buffer.remaining() <= nonceLength) {
                error("protected secret is malformed")
            }
            val nonce = ByteArray(nonceLength)
            buffer.get(nonce)
            val ciphertext = ByteArray(buffer.remaining())
            buffer.get(ciphertext)
            return try {
                val cipher = Cipher.getInstance(TRANSFORMATION)
                cipher.init(Cipher.DECRYPT_MODE, masterKey(), GCMParameterSpec(128, nonce))
                cipher.updateAAD(aad(keyId))
                cipher.doFinal(ciphertext)
            } finally {
                nonce.fill(0)
                ciphertext.fill(0)
            }
        } finally {
            encoded.fill(0)
        }
    }

    @Synchronized
    fun delete(keyId: String): Boolean {
        validateKeyId(keyId)
        val target = file(keyId)
        if (!target.exists()) return false
        val length = target.length().coerceAtMost(MAX_OVERWRITE_BYTES.toLong()).toInt()
        if (length > 0) runCatching { target.writeBytes(ByteArray(length)) }
        return target.delete()
    }

    private fun ensureMasterKey() {
        if (keyStore.containsAlias(MASTER_KEY_ALIAS)) return
        val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, KEYSTORE)
        generator.init(
            KeyGenParameterSpec.Builder(
                MASTER_KEY_ALIAS,
                KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT,
            )
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(256)
                .build(),
        )
        generator.generateKey()
    }

    private fun masterKey(): SecretKey =
        (keyStore.getEntry(MASTER_KEY_ALIAS, null) as KeyStore.SecretKeyEntry).secretKey
    private fun file(keyId: String) = File(root, "$keyId.secret")
    private fun aad(keyId: String) = "$namespace:$keyId".toByteArray(Charsets.UTF_8)
    private fun validateKeyId(keyId: String) {
        require(keyId.matches(Regex("[0-9a-f]{32}"))) { "invalid protected secret handle" }
    }
    private fun writeAtomic(target: File, bytes: ByteArray) {
        val temporary = File(root, ".${target.name}.tmp")
        temporary.outputStream().use { output ->
            output.write(bytes)
            output.fd.sync()
        }
        try {
            Files.move(temporary.toPath(), target.toPath(), StandardCopyOption.ATOMIC_MOVE)
        } catch (_: Exception) {
            Files.move(temporary.toPath(), target.toPath(), StandardCopyOption.REPLACE_EXISTING)
        }
    }

    companion object {
        private const val KEYSTORE = "AndroidKeyStore"
        private const val MASTER_KEY_ALIAS = "torca.protected-secrets.v1"
        private const val TRANSFORMATION = "AES/GCM/NoPadding"
        private const val MAX_OVERWRITE_BYTES = 1024 * 1024
    }
}

/** Narrow static JNI target. Product workflow remains in Rust. */
object AndroidKeystoreBridge {
    private lateinit var applicationContext: Context
    private lateinit var databaseSecrets: AndroidKeystoreSecretStore
    private lateinit var identitySecrets: AndroidKeystoreSecretStore
    private lateinit var peerSecrets: AndroidKeystoreSecretStore
    private lateinit var databaseFile: File
    private lateinit var runtimeRoot: File
    private var radioCaptureThread: Thread? = null
    private var radioCaptureStop: AtomicBoolean? = null

    @JvmStatic
    fun initialize(context: Context) {
        val appContext = context.applicationContext
        applicationContext = appContext
        databaseSecrets = AndroidKeystoreSecretStore(appContext, "database")
        identitySecrets = AndroidKeystoreSecretStore(appContext, "identity")
        peerSecrets = AndroidKeystoreSecretStore(appContext, "peer")

        val torcaRoot = File(appContext.noBackupFilesDir, "torca")
        if (!torcaRoot.exists() && !torcaRoot.mkdirs()) error("could not create Torca root directory")
        val dataDirectory = File(torcaRoot, "data")
        if (!dataDirectory.exists() && !dataDirectory.mkdirs()) {
            error("could not create Torca data directory")
        }
        databaseFile = File(dataDirectory, "torca.db")

        val stableRuntime = File(torcaRoot, "runtime")
        runtimeRoot = stableRuntime
        if (!runtimeRoot.exists() && !runtimeRoot.mkdirs()) {
            error("could not create Torca runtime directory")
        }
        check(nativeBindRuntime()) { "could not bind Android runtime bridge" }
        check(nativeInitializeAudioContext(appContext)) {
            "could not initialize Android audio context"
        }
    }

    @JvmStatic
    fun insert(namespace: String, keyId: String, secret: ByteArray) {
        store(namespace).insert(keyId, secret)
    }
    @JvmStatic
    fun load(namespace: String, keyId: String): ByteArray? = store(namespace).load(keyId)
    @JvmStatic
    fun delete(namespace: String, keyId: String): Boolean = store(namespace).delete(keyId)
    @JvmStatic
    fun databasePath(): String = databaseFile.absolutePath
    @JvmStatic
    fun runtimeRootPath(): String = runtimeRoot.absolutePath
    @JvmStatic
    fun logRootPath(): String {
        val root = File(applicationContext.getExternalFilesDir(null), "torca/logs")
        check(root.exists() || root.mkdirs()) { "could not create Torca log directory" }
        return root.absolutePath
    }

    @JvmStatic
    fun reportNativeFailure(message: String) {
        Log.e("TorcaRuntime", message)
    }

    /** Starts the Android voice-communication capture path with platform DSP. */
    @JvmStatic
    @Synchronized
    fun startRadioCapture(): Boolean {
        if (radioCaptureThread?.isAlive == true) return true
        val sampleRate = 8_000
        val minimum = AudioRecord.getMinBufferSize(
            sampleRate,
            AudioFormat.CHANNEL_IN_MONO,
            AudioFormat.ENCODING_PCM_16BIT,
        )
        if (minimum <= 0) return false
        val bufferSize = max(minimum, sampleRate / 5 * 2)
        val record = try {
            AudioRecord.Builder()
                .setAudioSource(MediaRecorder.AudioSource.VOICE_COMMUNICATION)
                .setAudioFormat(
                    AudioFormat.Builder()
                        .setSampleRate(sampleRate)
                        .setEncoding(AudioFormat.ENCODING_PCM_16BIT)
                        .setChannelMask(AudioFormat.CHANNEL_IN_MONO)
                        .build(),
                )
                .setBufferSizeInBytes(bufferSize)
                .build()
        } catch (error: Throwable) {
            Log.w("TorcaAudio", "could not create voice communication capture", error)
            return false
        }
        if (record.state != AudioRecord.STATE_INITIALIZED) {
            record.release()
            return false
        }
        val session = record.audioSessionId
        val aec = if (AcousticEchoCanceler.isAvailable()) {
            AcousticEchoCanceler.create(session)?.also { effect ->
                runCatching { effect.enabled = true }
            }
        } else null
        val ns = if (NoiseSuppressor.isAvailable()) {
            NoiseSuppressor.create(session)?.also { effect ->
                runCatching { effect.enabled = true }
            }
        } else null
        val agc = if (AutomaticGainControl.isAvailable()) {
            AutomaticGainControl.create(session)?.also { effect ->
                runCatching { effect.enabled = true }
            }
        } else null
        try {
            record.startRecording()
            check(record.recordingState == AudioRecord.RECORDSTATE_RECORDING)
        } catch (error: Throwable) {
            Log.w("TorcaAudio", "could not start voice communication capture", error)
            aec?.release()
            ns?.release()
            agc?.release()
            record.release()
            return false
        }
        nativeSetRadioCaptureActive(true)
        val stop = AtomicBoolean(false)
        radioCaptureStop = stop
        radioCaptureThread = Thread {
            val buffer = ByteArray(bufferSize and -2)
            try {
                while (!stop.get()) {
                    val read = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
                        record.read(buffer, 0, buffer.size, AudioRecord.READ_BLOCKING)
                    } else {
                        @Suppress("DEPRECATION")
                        record.read(buffer, 0, buffer.size)
                    }
                    if (read > 0) nativePushRadioPcm(buffer.copyOf(read))
                }
            } catch (error: Throwable) {
                Log.w("TorcaAudio", "voice communication capture stopped", error)
            } finally {
                runCatching { record.stop() }
                aec?.release()
                ns?.release()
                agc?.release()
                record.release()
                nativeSetRadioCaptureActive(false)
            }
        }.apply {
            name = "torca-radio-capture"
            isDaemon = true
            start()
        }
        return true
    }

    @JvmStatic
    @Synchronized
    fun stopRadioCapture() {
        nativeSetRadioCaptureActive(false)
        radioCaptureStop?.set(true)
        radioCaptureThread?.interrupt()
        radioCaptureThread = null
        radioCaptureStop = null
    }

    private fun store(namespace: String): AndroidKeystoreSecretStore = when (namespace) {
        "database" -> databaseSecrets
        "identity" -> identitySecrets
        "peer" -> peerSecrets
        else -> error("unsupported secret namespace")
    }

    @JvmStatic external fun nativeBindRuntime(): Boolean
    @JvmStatic external fun nativeInitializeAudioContext(context: Context): Boolean
    @JvmStatic external fun nativePushRadioPcm(data: ByteArray)
    @JvmStatic external fun nativeSetRadioCaptureActive(active: Boolean)
}
