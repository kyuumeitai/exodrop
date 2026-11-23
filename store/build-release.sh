#!/usr/bin/env bash
# Build the signed Google Play bundle (AAB) for EXODROP.
# Usage: store/build-release.sh <versionName> <versionCode>     e.g. store/build-release.sh 0.1.0 1
# Signs with ~/.android/upload-keys/exodrop-upload.properties (never committed; back it up).
set -euo pipefail
cd "$(dirname "$0")/.."
NAME=${1:?versionName, e.g. 0.1.0}
CODE=${2:?versionCode, integer, must increase every upload}
export ORG_GRADLE_PROJECT_exodropVersionName="$NAME"
export ORG_GRADLE_PROJECT_exodropVersionCode="$CODE"
./scripts/build_android.sh release
mkdir -p build
AAB=mobile/android/app/build/outputs/bundle/release/app-release.aab
cp "$AAB" "build/exodrop-$NAME-$CODE.aab"
cp mobile/android/app/build/outputs/apk/release/app-release.apk "build/exodrop-$NAME-$CODE.apk"
keytool -printcert -jarfile "build/exodrop-$NAME-$CODE.aab" | grep -E "SHA256:" | head -1
echo "AAB: build/exodrop-$NAME-$CODE.aab  sha256 $(shasum -a 256 "build/exodrop-$NAME-$CODE.aab" | cut -d' ' -f1)"
