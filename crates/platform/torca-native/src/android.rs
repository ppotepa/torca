use core::ffi::c_void;
use std::path::PathBuf;
use std::sync::OnceLock;

use jni::objects::{GlobalRef, JByteArray, JClass, JObject, JString, JValue};
use jni::sys::{JNI_ERR, JNI_VERSION_1_6, jint};
use jni::{JNIEnv, JavaVM};
use torca_crypto::{ProtectedSecretStore, ProtectedSecretStoreError};
use torca_identity::KeyId;

static JAVA_VM: OnceLock<JavaVM> = OnceLock::new();
static BRIDGE_CLASS_REF: OnceLock<GlobalRef> = OnceLock::new();
static ANDROID_CONTEXT_REF: OnceLock<GlobalRef> = OnceLock::new();

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

/// Binds the application class loader to the native runtime.  Rust startup
/// work runs on threads which do not have Android's application class loader,
/// so resolving this class by name there is unreliable.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_com_torca_host_AndroidKeystoreBridge_nativeBindRuntime(
    env: *mut jni::sys::JNIEnv,
    class: jni::sys::jclass,
) -> jni::sys::jboolean {
    let Ok(env) = (unsafe { JNIEnv::from_raw(env) }) else {
        return 0;
    };
    let class = unsafe { JClass::from_raw(class) };
    let Ok(global) = env.new_global_ref(class) else {
        return 0;
    };
    if BRIDGE_CLASS_REF.set(global).is_ok() || BRIDGE_CLASS_REF.get().is_some() { 1 } else { 0 }
}

/// Initializes the Android context used by CPAL's AAudio backend.  Flutter
/// does not install ndk-glue, so this must be done explicitly before any
/// radio audio device is queried. The global reference keeps the application
/// context alive for native worker threads.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_com_torca_host_AndroidKeystoreBridge_nativeInitializeAudioContext(
    env: *mut jni::sys::JNIEnv,
    _class: jni::sys::jclass,
    context: jni::sys::jobject,
) -> jni::sys::jboolean {
    let Ok(env) = (unsafe { JNIEnv::from_raw(env) }) else {
        return 0;
    };
    let context = unsafe { JObject::from_raw(context) };
    let Ok(global) = env.new_global_ref(&context) else {
        return 0;
    };
    if ANDROID_CONTEXT_REF.set(global).is_err() {
        return 1;
    }
    let Some(vm) = JAVA_VM.get() else { return 0 };
    // ndk-context requires this call exactly once. The OnceLock above makes
    // repeated service/activity initialization harmless.
    unsafe {
        ndk_context::initialize_android_context(
            vm.get_java_vm_pointer().cast(),
            ANDROID_CONTEXT_REF
                .get()
                .expect("Android context was just initialized")
                .as_obj()
                .as_raw()
                .cast(),
        );
    }
    1
}

/// Receives bounded PCM chunks from the Android AudioRecord voice
/// communication engine. The adapter converts them to Î¼-law frames and drops
/// them unless the Rust radio coordinator currently owns the floor.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_com_torca_host_AndroidKeystoreBridge_nativePushRadioPcm(
    env: JNIEnv,
    _class: JClass,
    data: JByteArray,
) {
    let Ok(bytes) = env.convert_byte_array(&data) else { return };
    torca_radio_adapters::push_android_pcm(&bytes);
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_com_torca_host_AndroidKeystoreBridge_nativeSetRadioCaptureActive(
    _env: *mut jni::sys::JNIEnv,
    _class: jni::sys::jclass,
    active: jni::sys::jboolean,
) {
    torca_radio_adapters::set_android_native_capture_active(active != 0);
}

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
            let class = bridge_class(env)?;
            env.call_static_method(
                class,
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
            let class = bridge_class(env)?;
            let value = env.call_static_method(
                class,
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
            let class = bridge_class(env)?;
            env.call_static_method(
                class,
                "delete",
                "(Ljava/lang/String;Ljava/lang/String;)Z",
                &[JValue::Object(&namespace_object), JValue::Object(&key_object)],
            )?
            .z()
        })
    }
}

pub(crate) fn database_path() -> Result<PathBuf, ProtectedSecretStoreError> {
    string_method("databasePath").map(PathBuf::from)
}
pub(crate) fn log_root_path() -> Result<PathBuf, ProtectedSecretStoreError> {
    string_method("logRootPath").map(PathBuf::from)
}
fn string_method(method: &str) -> Result<String, ProtectedSecretStoreError> {
    with_env(|env| {
        let class = bridge_class(env)?;
        let value = env.call_static_method(class, method, "()Ljava/lang/String;", &[])?;
        let object = value.l()?;
        if object.is_null() {
            return Err(jni::errors::Error::NullPtr("Android bridge string result"));
        }
        let value = JString::from(object);
        Ok(env.get_string(&value)?.into())
    })
}

fn bridge_class<'local>(env: &mut JNIEnv<'local>) -> jni::errors::Result<JClass<'local>> {
    let class = BRIDGE_CLASS_REF
        .get()
        .ok_or(jni::errors::Error::NullPtr("Android bridge class is not bound"))?;
    // A GlobalRef is an object reference.  Re-wrap its raw `jclass` for this
    // environment so `call_static_method` performs a static lookup, rather
    // than treating the class object as an instance receiver.
    let _ = env;
    unsafe { Ok(JClass::from_raw(class.as_obj().as_raw() as jni::sys::jclass)) }
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
        Err(error) => {
            if env.exception_check().unwrap_or(false) {
                let _ = env.exception_describe();
                let _ = env.exception_clear();
            }
            Err(ProtectedSecretStoreError(format!("Android JNI operation failed: {error}")))
        }
    }
}
