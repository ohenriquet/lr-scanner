#!/usr/bin/env bash
# Build do app Android com a ponte nativa (.so) embutida. Ver ANDROID.md.
#
# Requisitos: Android SDK + NDK, cargo-ndk, Flutter. E o transporte USB-OTG
# (jlr_connect_fd) implementado — ver ANDROID.md (ainda pendente).
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"   # .../app/jlr_app
ROOT="$(cd "$HERE/../.." && pwd)"       # raiz do repo
PACK="${1:-$ROOT/packs/L319_201200.sqlite}"

cd "$ROOT"
# shellcheck disable=SC1091
source env.sh

command -v cargo-ndk >/dev/null 2>&1 || { echo "instale: cargo install cargo-ndk"; exit 1; }

echo "== 1/3 compilando a ponte para as ABIs do Android =="
( cd crates/bridge && cargo ndk \
    -t arm64-v8a -t armeabi-v7a -t x86_64 \
    -o "$HERE/android/app/src/main/jniLibs" build --release )

echo "== 2/3 exportando o pacote do veículo =="
cargo run --release -p sdd-import -- pack-export --pack "$PACK" \
  --out "$HERE/assets/pack.json"
mkdir -p "$HERE/assets/car"
cp -f "$ROOT/app/assets/car/lr4_santorini_black.jpg" "$HERE/assets/car/d4.jpg" 2>/dev/null || true

echo "== 3/3 flutter build apk =="
cd "$HERE"
flutter build apk --release
echo "APK: $HERE/build/app/outputs/flutter-apk/app-release.apk"
