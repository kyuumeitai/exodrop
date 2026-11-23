#!/usr/bin/env bash
set -euo pipefail

# Build EXODROP for Android.
#
#   ./scripts/build_android.sh debug            # debug APK (installable with adb), release-optimised Rust
#   ./scripts/build_android.sh release          # signed Play bundle (.aab) + signed release APK
#
# ABIS="arm64-v8a armeabi-v7a" by default (Play needs both to reach 32-bit-only phones such as Android Go devices).
# Release signing reads ~/.android/upload-keys/exodrop-upload.properties (storeFile, storePassword, keyAlias,
# keyPassword) or $EXODROP_SIGNING_PROPERTIES.
# Outputs:
#   debug   -> mobile/android/app/build/outputs/apk/debug/app-debug.apk
#   release -> mobile/android/app/build/outputs/bundle/release/app-release.aab
#              mobile/android/app/build/outputs/apk/release/app-release.apk

MODE="${1:-debug}"
ABIS="${ABIS:-arm64-v8a armeabi-v7a}"
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
TOOLCHAIN="$(ls -d "$NDK_DIR"/toolchains/llvm/prebuilt/* | head -n 1)"

API=26
export ANDROID_HOME="$ANDROID_SDK"
export ANDROID_NDK_HOME="$NDK_DIR"
export ANDROID_NDK_ROOT="$NDK_DIR"

JNI_ROOT="$ROOT_DIR/mobile/android/app/src/main/jniLibs"
rm -rf "$JNI_ROOT"

for ABI in $ABIS; do
    case "$ABI" in
        arm64-v8a)   TARGET=aarch64-linux-android;   CLANG=aarch64-linux-android;    SYSLIB=aarch64-linux-android ;;
        armeabi-v7a) TARGET=armv7-linux-androideabi; CLANG=armv7a-linux-androideabi; SYSLIB=arm-linux-androideabi ;;
        x86_64)      TARGET=x86_64-linux-android;    CLANG=x86_64-linux-android;     SYSLIB=x86_64-linux-android ;;
        *) echo "Unsupported ABI $ABI" >&2; exit 1 ;;
    esac
    TU="$(echo "$TARGET" | tr 'a-z-' 'A-Z_')"
    TL="$(echo "$TARGET" | tr '-' '_')"
    export "CC_${TL}=$TOOLCHAIN/bin/${CLANG}${API}-clang"
    export "CXX_${TL}=$TOOLCHAIN/bin/${CLANG}${API}-clang++"
    export "AR_${TL}=$TOOLCHAIN/bin/llvm-ar"
    export "CARGO_TARGET_${TU}_LINKER=$TOOLCHAIN/bin/${CLANG}${API}-clang"

    echo "==> Building Rust cdylib for $ABI ($TARGET, release) with NDK $(basename "$NDK_DIR")..."
    cargo build --lib -p exodrop --release --target "$TARGET"

    mkdir -p "$JNI_ROOT/$ABI"
    # Strip debug info: the unstripped arm64 library is ~136 MB, stripped it is a few MB.
    "$TOOLCHAIN/bin/llvm-strip" --strip-unneeded \
        -o "$JNI_ROOT/$ABI/libexodrop.so" "$ROOT_DIR/target/$TARGET/release/libexodrop.so"
    # libc++_shared.so for Oboe (cpal Android audio)
    cp "$TOOLCHAIN/sysroot/usr/lib/$SYSLIB/libc++_shared.so" "$JNI_ROOT/$ABI/libc++_shared.so"
    ls -lh "$JNI_ROOT/$ABI"
done

cd "$ROOT_DIR/mobile/android"
printf "sdk.dir=%s\n" "$ANDROID_SDK" > local.properties
export EXODROP_ABIS="$(echo $ABIS | tr ' ' ',')"

if [[ "$MODE" == "release" ]]; then
    PROPS="${EXODROP_SIGNING_PROPERTIES:-$HOME/.android/upload-keys/exodrop-upload.properties}"
    if [[ ! -f "$PROPS" ]]; then
        echo "Error: release signing properties not found at $PROPS" >&2
        exit 1
    fi
    export EXODROP_SIGNING_PROPERTIES="$PROPS"
    echo "==> Gradle bundleRelease + assembleRelease..."
    gradle --quiet bundleRelease assembleRelease
    ls -lh app/build/outputs/bundle/release/app-release.aab app/build/outputs/apk/release/app-release.apk
else
    echo "==> Gradle assembleDebug..."
    gradle --quiet assembleDebug
    ls -lh app/build/outputs/apk/debug/app-debug.apk
fi
