#!/usr/bin/env bash
set -euo pipefail

# Build EXODROP Android arm64-v8a native library + APK.
# Usage: ./scripts/build_android.sh [debug|release]

PROFILE="${1:-release}"
ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

ANDROID_SDK="${ANDROID_HOME:-$HOME/Library/Android/sdk}"
if [[ ! -d "$ANDROID_SDK" ]]; then
    echo "Error: Android SDK not found at $ANDROID_SDK (set ANDROID_HOME)." >&2
    exit 1
fi

NDK_DIR="${ANDROID_NDK_HOME:-$(ls -d "$ANDROID_SDK"/ndk/* 2>/dev/null | sort -V | tail -n 1)}"
if [[ -z "$NDK_DIR" || ! -d "$NDK_DIR" ]]; then
    echo "Error: Android NDK not found under $ANDROID_SDK/ndk." >&2
    exit 1
fi

HOST_TAG="$(uname -s | tr '[:upper:]' '[:lower:]')-x86_64"
TOOLCHAIN="$NDK_DIR/toolchains/llvm/prebuilt/$HOST_TAG"
if [[ ! -d "$TOOLCHAIN" ]]; then
    TOOLCHAIN="$(ls -d "$NDK_DIR"/toolchains/llvm/prebuilt/* | head -n 1)"
fi

API=26
export ANDROID_HOME="$ANDROID_SDK"
export ANDROID_NDK_HOME="$NDK_DIR"
export ANDROID_NDK_ROOT="$NDK_DIR"
export CC_aarch64_linux_android="$TOOLCHAIN/bin/aarch64-linux-android${API}-clang"
export CXX_aarch64_linux_android="$TOOLCHAIN/bin/aarch64-linux-android${API}-clang++"
export AR_aarch64_linux_android="$TOOLCHAIN/bin/llvm-ar"
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$TOOLCHAIN/bin/aarch64-linux-android${API}-clang"

CARGO_FLAGS=""
if [[ "$PROFILE" == "release" ]]; then
    CARGO_FLAGS="--release"
fi

echo "==> Building Rust cdylib (aarch64-linux-android, $PROFILE) with NDK $(basename "$NDK_DIR")..."
cargo build --lib -p exodrop --target aarch64-linux-android $CARGO_FLAGS

JNI_DIR="$ROOT_DIR/mobile/android/app/src/main/jniLibs/arm64-v8a"
mkdir -p "$JNI_DIR"
cp "$ROOT_DIR/target/aarch64-linux-android/$PROFILE/libexodrop.so" "$JNI_DIR/libexodrop.so"

# Bundle libc++_shared.so for Oboe (cpal Android audio)
LIBCPP="$TOOLCHAIN/sysroot/usr/lib/aarch64-linux-android/libc++_shared.so"
if [[ -f "$LIBCPP" ]]; then
    cp "$LIBCPP" "$JNI_DIR/libc++_shared.so"
fi

echo "==> Assembling Android APK with Gradle..."
cd "$ROOT_DIR/mobile/android"
printf "sdk.dir=%s\n" "$ANDROID_SDK" > local.properties
gradle assembleDebug

APK_PATH="$ROOT_DIR/mobile/android/app/build/outputs/apk/debug/app-debug.apk"
echo "✅ Android APK built: $APK_PATH"
