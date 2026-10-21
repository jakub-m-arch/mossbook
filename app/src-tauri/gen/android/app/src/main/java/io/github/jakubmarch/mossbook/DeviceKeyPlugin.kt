package io.github.jakubmarch.mossbook

import android.app.Activity
import android.content.pm.PackageManager
import android.os.Build
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyPermanentlyInvalidatedException
import android.security.keystore.KeyProperties
import android.security.keystore.StrongBoxUnavailableException
import android.util.Base64
import androidx.biometric.BiometricManager
import androidx.biometric.BiometricManager.Authenticators.BIOMETRIC_STRONG
import androidx.biometric.BiometricPrompt
import androidx.core.content.ContextCompat
import androidx.fragment.app.FragmentActivity
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

@InvokeArg
class PromptText {
  lateinit var title: String
  var subtitle: String? = null
  lateinit var cancel: String
}

@InvokeArg
class SealArgs {
  lateinit var key: String
  lateinit var prompt: PromptText
}

@InvokeArg
class OpenArgs {
  lateinit var sealed: String
  lateinit var prompt: PromptText
}

/**
 * Fingerprint unlock (V-7, SPEC §6.5): encrypts and decrypts the vault's device
 * key with an AES-256-GCM key in the Android Keystore. That key can only be used
 * right after a strong (class 3) biometric check, never leaves the secure
 * hardware, and is invalidated when fingerprints are added or removed. Only Rust
 * calls this plugin; none of its commands is granted to the web view.
 *
 * Nothing here is logged: not the key, not the ciphertext, not error details.
 */
@TauriPlugin
class DeviceKeyPlugin(private val activity: Activity) : Plugin(activity) {
  private val alias = "mossbook-device-key"
  private val gcmTagBits = 128
  private val ivLength = 12

  @Command
  fun status(invoke: Invoke) {
    val result = JSObject()
    val status = BiometricManager.from(activity).canAuthenticate(BIOMETRIC_STRONG)
    result.put("available", status == BiometricManager.BIOMETRIC_SUCCESS)
    // A strong sensor without enrolled fingerprints: the UI can say where to add one.
    result.put("enrollable", status == BiometricManager.BIOMETRIC_ERROR_NONE_ENROLLED)
    invoke.resolve(result)
  }

  /** Encrypts a fresh device key after a biometric check; returns IV || ciphertext. */
  @Command
  fun seal(invoke: Invoke) {
    val args = invoke.parseArgs(SealArgs::class.java)
    val cipher = try {
      Cipher.getInstance(TRANSFORMATION).apply { init(Cipher.ENCRYPT_MODE, newKey()) }
    } catch (e: Exception) {
      invoke.reject("cannot create the device key", "failed")
      return
    }
    authenticate(invoke, cipher, args.prompt) { authenticated ->
      val plain = Base64.decode(args.key, Base64.NO_WRAP)
      try {
        val sealed = authenticated.iv + authenticated.doFinal(plain)
        val result = JSObject()
        result.put("sealed", Base64.encodeToString(sealed, Base64.NO_WRAP))
        result
      } finally {
        plain.fill(0)
      }
    }
  }

  /** Decrypts the device key after a biometric check. */
  @Command
  fun open(invoke: Invoke) {
    val args = invoke.parseArgs(OpenArgs::class.java)
    val sealed = try {
      Base64.decode(args.sealed, Base64.NO_WRAP)
    } catch (e: IllegalArgumentException) {
      invoke.reject("malformed device key", "invalid")
      return
    }
    if (sealed.size <= ivLength) {
      invoke.reject("malformed device key", "invalid")
      return
    }
    val key = existingKey()
    if (key == null) {
      invoke.reject("no device key", "invalidated")
      return
    }
    val cipher = try {
      Cipher.getInstance(TRANSFORMATION).apply {
        init(Cipher.DECRYPT_MODE, key, GCMParameterSpec(gcmTagBits, sealed, 0, ivLength))
      }
    } catch (e: KeyPermanentlyInvalidatedException) {
      // Fingerprints changed since the key was made: it can never be used again.
      deleteKey()
      invoke.reject("the device key was invalidated", "invalidated")
      return
    } catch (e: Exception) {
      invoke.reject("cannot use the device key", "failed")
      return
    }
    authenticate(invoke, cipher, args.prompt) { authenticated ->
      val plain = authenticated.doFinal(sealed, ivLength, sealed.size - ivLength)
      try {
        val result = JSObject()
        result.put("key", Base64.encodeToString(plain, Base64.NO_WRAP))
        result
      } finally {
        plain.fill(0)
      }
    }
  }

  /** Deletes the Keystore key, e.g. when fingerprint unlock is turned off. */
  @Command
  fun forget(invoke: Invoke) {
    deleteKey()
    invoke.resolve()
  }

  private fun canAuthenticate(): Boolean =
    BiometricManager.from(activity).canAuthenticate(BIOMETRIC_STRONG) ==
      BiometricManager.BIOMETRIC_SUCCESS

  private fun authenticate(
    invoke: Invoke,
    cipher: Cipher,
    text: PromptText,
    onSuccess: (Cipher) -> JSObject,
  ) {
    val fragmentActivity = activity as? FragmentActivity
    if (fragmentActivity == null || !canAuthenticate()) {
      invoke.reject("biometrics unavailable", "unavailable")
      return
    }
    activity.runOnUiThread {
      val callback = object : BiometricPrompt.AuthenticationCallback() {
        override fun onAuthenticationSucceeded(result: BiometricPrompt.AuthenticationResult) {
          val authenticated = result.cryptoObject?.cipher
          if (authenticated == null) {
            invoke.reject("no cipher after authentication", "failed")
            return
          }
          try {
            invoke.resolve(onSuccess(authenticated))
          } catch (e: Exception) {
            // A wrong or damaged ciphertext fails the GCM tag check here.
            invoke.reject("the device key did not open", "invalid")
          }
        }

        override fun onAuthenticationError(errorCode: Int, errString: CharSequence) {
          val code = when (errorCode) {
            BiometricPrompt.ERROR_NEGATIVE_BUTTON,
            BiometricPrompt.ERROR_USER_CANCELED,
            BiometricPrompt.ERROR_CANCELED -> "cancelled"
            BiometricPrompt.ERROR_LOCKOUT,
            BiometricPrompt.ERROR_LOCKOUT_PERMANENT -> "lockout"
            else -> "failed"
          }
          invoke.reject("biometric check ended", code)
        }
        // onAuthenticationFailed (one unrecognised finger): the prompt stays open.
      }
      val info = BiometricPrompt.PromptInfo.Builder()
        .setTitle(text.title)
        .apply { text.subtitle?.let { setSubtitle(it) } }
        .setNegativeButtonText(text.cancel)
        .setAllowedAuthenticators(BIOMETRIC_STRONG)
        .setConfirmationRequired(false)
        .build()
      BiometricPrompt(fragmentActivity, ContextCompat.getMainExecutor(activity), callback)
        .authenticate(info, BiometricPrompt.CryptoObject(cipher))
    }
  }

  private fun keyStore(): KeyStore = KeyStore.getInstance(KEYSTORE).apply { load(null) }

  private fun existingKey(): SecretKey? = keyStore().getKey(alias, null) as? SecretKey

  private fun deleteKey() {
    try {
      keyStore().deleteEntry(alias)
    } catch (e: Exception) {
      // Nothing to delete.
    }
  }

  /** A new Keystore key, replacing any older one. StrongBox when the phone has it. */
  private fun newKey(): SecretKey {
    deleteKey()
    val strongBox = Build.VERSION.SDK_INT >= Build.VERSION_CODES.P &&
      activity.packageManager.hasSystemFeature(PackageManager.FEATURE_STRONGBOX_KEYSTORE)
    if (strongBox) {
      try {
        return generateKey(true)
      } catch (e: StrongBoxUnavailableException) {
        // Fall back to the trusted execution environment.
      }
    }
    return generateKey(false)
  }

  private fun generateKey(strongBox: Boolean): SecretKey {
    val spec = KeyGenParameterSpec.Builder(
      alias,
      KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT,
    )
      .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
      .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
      .setKeySize(256)
      .setUserAuthenticationRequired(true)
      .setInvalidatedByBiometricEnrollment(true)
      .apply {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
          // Every use needs a fresh strong biometric check.
          setUserAuthenticationParameters(0, KeyProperties.AUTH_BIOMETRIC_STRONG)
        } else {
          @Suppress("DEPRECATION")
          setUserAuthenticationValidityDurationSeconds(-1)
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) setIsStrongBoxBacked(strongBox)
      }
      .build()
    return KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, KEYSTORE)
      .apply { init(spec) }
      .generateKey()
  }

  private companion object {
    const val KEYSTORE = "AndroidKeyStore"
    const val TRANSFORMATION = "AES/GCM/NoPadding"
  }
}
