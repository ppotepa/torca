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
 * The wrapped blobs live under noBackupFilesDir and are bound to their canonical KeyId as AAD.
 * This class never returns or logs the Android Keystore master key.
 */
class AndroidKeystoreSecretStore(context: Context) {
    private val root = File(context.noBackupFilesDir, "torca/protected-secrets")
    private val keyStore = KeyStore.getInstance(KEYSTORE_PROVIDER).apply { load(null) }

    init {
        check(root.exists() || root.mkdirs()) { "Unable to create protected secret directory" }
    }

    @Synchronized
    fun insert(keyId: String, secret: ByteArray) {
        validateKeyId(keyId)
        val target = fileFor(keyId)
        check(!target.exists()) { "Protected key handle already exists" }

        val cipher = Cipher.getInstance(TRANSFORMATION)
        cipher.init(Cipher.ENCRYPT_MODE, masterKey())
        cipher.updateAAD(keyId.toByteArray(StandardCharsets.US_ASCII))
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
            cipher.updateAAD(keyId.toByteArray(StandardCharsets.US_ASCII))
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
    }
}
