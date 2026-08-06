use core::ffi::c_void;
use std::path::PathBuf;
use std::sync::OnceLock;

use jni::objects::{JByteArray, JObject, JString, JValue};
use jni::sys::{JNI_ERR, JNI_VERSION_1_6, jint};
use jni::{JNIEnv, JavaVM};
use torca_crypto::{ProtectedSecretStore, ProtectedSecretStoreError};
use torca_identity::KeyId;

const BRIDGE_CLASS: &str = "com/torca/host/AndroidKeystoreBridge";
static JAVA_VM: OnceLock<JavaVM> = OnceLock::new();

/// Captures the process Java VM when Android loads `libtorca_bridge.so`.
///
/// # Safety
///
/// Called by the Android runtime with a valid JavaVM pointer during `System.loadLibrary`.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn JNI_OnLoad(
    vm: *mut jni::sys::JavaVM,
    _reserved: *mut c_void,
) -> jint {
    let java_vm = match unsafe { JavaVM::from_raw(vm) } {
        Ok(value) => value,
        Err(_) => return JNI_ERR,
    };
    let _ = JAVA_VM.set(java_vm);
    JNI_VERSION_1_6
}

/// Android Keystore-backed implementation of the common protected-secret port.
pub(crate) struct AndroidProtectedSecretStore {
    namespace: &'static str,
}

impl AndroidProtectedSecretStore {
    pub(crate) const fn new(namespace: &'static str) -> Self {
        Self { namespace }
    }
}

impl ProtectedSecretStore for AndroidProtectedSecretStore {
    fn insert(&mut self, key_id: KeyId, secret: &[u8]) -> Result<(), ProtectedSecretStoreError> {
        let namespace = self.namespace;
        with_env(|env| {
            let namespace = env.new_string(namespace)?;
            let key = env.new_string(key_id.to_string())?;
            let secret_array = env.byte_array_from_slice(secret)?;
            let namespace_object = JObject::from(namespace);
            let key_object = JObject::from(key);
            let secret_object = JObject::from(secret_array);
            env.call_static_method(
                BRIDGE_CLASS,
                "insert",
                "(Ljava/lang/String;Ljava/lang/String;[B)V",
                &[
                    JValue::Object(&namespace_object),
                    JValue::Object(&key_object),
                    JValue::Object(&secret_object),
                ],
            )?;
            Ok(())
        })
    }

    fn load(&self, key_id: KeyId) -> Result<Option<Vec<u8>>, ProtectedSecretStoreError> {
        let namespace = self.namespace;
        with_env(|env| {
            let namespace = env.new_string(namespace)?;
            let key = env.new_string(key_id.to_string())?;
            let namespace_object = JObject::from(namespace);
            let key_object = JObject::from(key);
            let value = env.call_static_method(
                BRIDGE_CLASS,
                "load",
                "(Ljava/lang/String;Ljava/lang/String;)[B",
                &[JValue::Object(&namespace_object), JValue::Object(&key_object)],
            )?;
            let object = value.l()?;
            if object.is_null() {
                return Ok(None);
            }
            let array = JByteArray::from(object);
            env.convert_byte_array(&array).map(Some)
        })
    }

    fn delete(&mut self, key_id: KeyId) -> Result<bool, ProtectedSecretStoreError> {
        let namespace = self.namespace;
        with_env(|env| {
            let namespace = env.new_string(namespace)?;
            let key = env.new_string(key_id.to_string())?;
            let namespace_object = JObject::from(namespace);
            let key_object = JObject::from(key);
            env.call_static_method(
                BRIDGE_CLASS,
                "delete",
                "(Ljava/lang/String;Ljava/lang/String;)Z",
                &[JValue::Object(&namespace_object), JValue::Object(&key_object)],
            )?
            .z()
        })
    }
}

pub(crate) fn database_path() -> Result<PathBuf, ProtectedSecretStoreError> {
    with_env(|env| {
        let value = env.call_static_method(BRIDGE_CLASS, "databasePath", "()Ljava/lang/String;", &[])?;
        let object = value.l()?;
        if object.is_null() {
            return Err(jni::errors::Error::NullPtr("Android databasePath"));
        }
        let path = JString::from(object);
        let value: String = env.get_string(&path)?.into();
        Ok(PathBuf::from(value))
    })
}

fn with_env<T>(
    operation: impl FnOnce(&mut JNIEnv<'_>) -> jni::errors::Result<T>,
) -> Result<T, ProtectedSecretStoreError> {
    let vm = JAVA_VM
        .get()
        .ok_or_else(|| ProtectedSecretStoreError("Android Java VM is unavailable".into()))?;
    let mut env = vm
        .attach_current_thread()
        .map_err(|_| ProtectedSecretStoreError("Android JNI thread attach failed".into()))?;
    match operation(&mut env) {
        Ok(value) => Ok(value),
        Err(_) => {
            if env.exception_check().unwrap_or(false) {
                let _ = env.exception_clear();
            }
            Err(ProtectedSecretStoreError(
                "Android Keystore JNI operation failed".into(),
            ))
        }
    }
}
