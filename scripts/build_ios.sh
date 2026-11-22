#!/usr/bin/env bash
set -euo pipefail

# Build EXODROP for iOS Simulator (default) or iOS Device ("device").
# Usage: ./scripts/build_ios.sh [sim|device] [debug|release]

TARGET_KIND="${1:-sim}"
PROFILE="${2:-release}"

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

if [[ "$TARGET_KIND" == "device" ]]; then
    RUST_TARGET="aarch64-apple-ios"
    SDK="iphoneos"
else
    RUST_TARGET="aarch64-apple-ios-sim"
    SDK="iphonesimulator"
fi

CARGO_FLAGS=""
XCODE_CFG="Debug"
if [[ "$PROFILE" == "release" ]]; then
    CARGO_FLAGS="--release"
    XCODE_CFG="Release"
fi

echo "==> Building Rust staticlib ($RUST_TARGET, $PROFILE)..."
IPHONEOS_DEPLOYMENT_TARGET=15.0 cargo build --lib -p exodrop --target "$RUST_TARGET" $CARGO_FLAGS

LIB_OUT="$ROOT_DIR/mobile/ios/libs/$SDK"
mkdir -p "$LIB_OUT"
cp "$ROOT_DIR/target/$RUST_TARGET/$PROFILE/libexodrop.a" "$LIB_OUT/libexodrop.a"

echo "==> Building iOS app bundle via xcodebuild ($SDK, $XCODE_CFG, arm64)..."
xcodebuild \
    -project "$ROOT_DIR/mobile/ios/ExoDropiOS.xcodeproj" \
    -scheme ExoDropiOS \
    -sdk "$SDK" \
    -configuration "$XCODE_CFG" \
    -derivedDataPath "$ROOT_DIR/target/ios-derived" \
    ARCHS=arm64 \
    VALID_ARCHS=arm64 \
    ONLY_ACTIVE_ARCH=NO \
    CODE_SIGNING_ALLOWED=NO \
    build

APP_PATH="$ROOT_DIR/target/ios-derived/Build/Products/${XCODE_CFG}-${SDK}/ExoDropiOS.app"
echo "✅ iOS app built: $APP_PATH"
