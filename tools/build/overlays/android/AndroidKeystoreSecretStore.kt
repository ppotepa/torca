package com.torca.host

import android.content.Context
import android.util.Log
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import java.io.File
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

    @JvmStatic
    fun relayEndpoint(): String =
        applicationContext.assets.open("torca/relay_endpoint.txt")
            .bufferedReader(Charsets.US_ASCII)
            .use { it.readText().trim() }

    private fun store(namespace: String): AndroidKeystoreSecretStore = when (namespace) {
        "database" -> databaseSecrets
        "identity" -> identitySecrets
        "peer" -> peerSecrets
        else -> error("unsupported secret namespace")
    }

    @JvmStatic external fun nativeBindRuntime(): Boolean
}
