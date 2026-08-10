# AndroidKeystoreBridge is a stable ABI consumed directly by Rust through JNI.
# Do not rename or remove its static methods in a release APK.
-keep class com.torca.host.AndroidKeystoreBridge {
    public static *;
}

# NativeRuntimeBridge methods are resolved by the Android VM from libtorca_native.
-keepclassmembers class com.torca.host.NativeRuntimeBridge {
    public static native <methods>;
}
