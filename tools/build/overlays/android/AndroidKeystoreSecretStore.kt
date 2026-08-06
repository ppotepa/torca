package com.torca.host

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import java.io.File
import java.io.FileOutputStream
import java.nio.charset.StandardCharsets
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/**
 * Wraps Torca secret bytes with a non-exportable Android Keystore AES-GCM key.
 *
 * Wrapped blobs live under noBackupFilesDir and are bound to namespace + canonical KeyId as AAD.
 */
class AndroidKeystoreSecretStore(context: Context, private val namespace: String) {
    private val root: File
    private val keyStore = KeyStore.getInstance(KEYSTORE_PROVIDER).apply { load(null) }

    init {
        require(NAMESPACE.matches(namespace)) { "Invalid protected secret namespace" }
        root = File(context.noBackupFilesDir, "torca/protected-secrets/$namespace")
        check(root.exists() || root.mkdirs()) { "Unable to create protected secret directory" }
    }

    @Synchronized
    fun insert(keyId: String, secret: ByteArray) {
        validateKeyId(keyId)
        val target = fileFor(keyId)
        check(!target.exists()) { "Protected key handle already exists" }

        val cipher = Cipher.getInstance(TRANSFORMATION)
        cipher.init(Cipher.ENCRYPT_MODE, masterKey())
        cipher.updateAAD(aad(keyId))
        val ciphertext = cipher.doFinal(secret)
        val iv = cipher.iv
        require(iv.isNotEmpty() && iv.size <= UByte.MAX_VALUE.toInt()) {
            "Android Keystore returned an invalid GCM IV"
        }

        val encoded = ByteArray(2 + iv.size + ciphertext.size)
        encoded[0] = FORMAT_VERSION
        encoded[1] = iv.size.toByte()
        iv.copyInto(encoded, destinationOffset = 2)
        ciphertext.copyInto(encoded, destinationOffset = 2 + iv.size)

        val temporary = File(root, ".$keyId.${android.os.Process.myPid()}.tmp")
        try {
            FileOutputStream(temporary).use { output ->
                output.write(encoded)
                output.fd.sync()
            }
            check(temporary.renameTo(target)) { "Unable to commit protected secret" }
        } finally {
            if (temporary.exists()) temporary.delete()
            encoded.fill(0)
        }
    }

    @Synchronized
    fun load(keyId: String): ByteArray? {
        validateKeyId(keyId)
        val target = fileFor(keyId)
        if (!target.exists()) return null

        val encoded = target.readBytes()
        try {
            require(encoded.size >= 2 && encoded[0] == FORMAT_VERSION) {
                "Unsupported protected secret format"
            }
            val ivLength = encoded[1].toUByte().toInt()
            require(ivLength > 0 && encoded.size > 2 + ivLength) {
                "Malformed protected secret"
            }
            val iv = encoded.copyOfRange(2, 2 + ivLength)
            val ciphertext = encoded.copyOfRange(2 + ivLength, encoded.size)
            val cipher = Cipher.getInstance(TRANSFORMATION)
            cipher.init(
                Cipher.DECRYPT_MODE,
                masterKey(),
                GCMParameterSpec(GCM_TAG_BITS, iv),
            )
            cipher.updateAAD(aad(keyId))
            return cipher.doFinal(ciphertext)
        } finally {
            encoded.fill(0)
        }
    }

    @Synchronized
    fun delete(keyId: String): Boolean {
        validateKeyId(keyId)
        val target = fileFor(keyId)
        if (!target.exists()) return false

        runCatching {
            val length = target.length()
            FileOutputStream(target, false).use { output ->
                val zeroes = ByteArray(4096)
                var remaining = length
                while (remaining > 0) {
                    val count = minOf(remaining, zeroes.size.toLong()).toInt()
                    output.write(zeroes, 0, count)
                    remaining -= count
                }
                output.fd.sync()
            }
        }
        return target.delete()
    }

    private fun masterKey(): SecretKey {
        val existing = keyStore.getKey(MASTER_KEY_ALIAS, null)
        if (existing is SecretKey) return existing

        val generator = KeyGenerator.getInstance(
            KeyProperties.KEY_ALGORITHM_AES,
            KEYSTORE_PROVIDER,
        )
        generator.init(
            KeyGenParameterSpec.Builder(
                MASTER_KEY_ALIAS,
                KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT,
            )
                .setKeySize(256)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setRandomizedEncryptionRequired(true)
                .build(),
        )
        return generator.generateKey()
    }

    private fun aad(keyId: String): ByteArray =
        "$namespace:$keyId".toByteArray(StandardCharsets.US_ASCII)

    private fun fileFor(keyId: String): File = File(root, "$keyId.keystore")

    private fun validateKeyId(keyId: String) {
        require(KEY_ID.matches(keyId)) { "KeyId must be 32 lowercase hexadecimal characters" }
    }

    private companion object {
        const val KEYSTORE_PROVIDER = "AndroidKeyStore"
        const val MASTER_KEY_ALIAS = "torca.protected-secrets.v1"
        const val TRANSFORMATION = "AES/GCM/NoPadding"
        const val GCM_TAG_BITS = 128
        const val FORMAT_VERSION: Byte = 1
        val KEY_ID = Regex("[0-9a-f]{32}")
        val NAMESPACE = Regex("[a-z][a-z0-9-]{0,31}")
    }
}

/** Minimal JNI surface used by the Rust production composition. */
object AndroidKeystoreBridge {
    private lateinit var applicationContext: Context
    private val stores = mutableMapOf<String, AndroidKeystoreSecretStore>()

    @JvmStatic
    @Synchronized
    fun initialize(context: Context) {
        if (::applicationContext.isInitialized) return
        applicationContext = context.applicationContext
    }

    @JvmStatic
    @Synchronized
    fun load(namespace: String, keyId: String): ByteArray? = store(namespace).load(keyId)

    @JvmStatic
    @Synchronized
    fun insert(namespace: String, keyId: String, secret: ByteArray) {
        store(namespace).insert(keyId, secret)
    }

    @JvmStatic
    @Synchronized
    fun delete(namespace: String, keyId: String): Boolean = store(namespace).delete(keyId)

    @JvmStatic
    @Synchronized
    fun databasePath(): String {
        check(::applicationContext.isInitialized) { "Android Keystore bridge is not initialized" }
        val directory = File(applicationContext.noBackupFilesDir, "torca/data")
        check(directory.exists() || directory.mkdirs()) { "Unable to create Torca data directory" }
        return File(directory, "torca.db").absolutePath
    }

    private fun store(namespace: String): AndroidKeystoreSecretStore {
        check(::applicationContext.isInitialized) { "Android Keystore bridge is not initialized" }
        return stores.getOrPut(namespace) {
            AndroidKeystoreSecretStore(applicationContext, namespace)
        }
    }
}
