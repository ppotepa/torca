# Android host baseline

The Android host contract requires a singleTask Flutter activity, no cloud backup, explicit notification/foreground-service permissions and restoration from durable engine state after process recreation.

The committed manifest is a composition input. Gradle/Flutter-generated runner files and ABI-specific `libtorca_bridge.so` artifacts must be generated and tested by the owner toolchain before Android is validated.
