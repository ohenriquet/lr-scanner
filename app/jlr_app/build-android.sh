#!/usr/bin/env bash
# Build do app Android (LR Scanner) com a ponte nativa (.so) embutida. Ver ANDROID.md.
#
# Requisitos: Android SDK + NDK, cargo-ndk, alvos Rust do Android, Flutter, JDK.
# Roda no Windows (Git Bash, com o env.sh do projeto) e no macOS/Linux.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"   # .../app/jlr_app
ROOT="$(cd "$HERE/../.." && pwd)"       # raiz do repo
PACK="${1:-$ROOT/packs/L319_201200.sqlite}"

cd "$ROOT"
case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*)
    # shellcheck disable=SC1091
    source env.sh
    ;;
  *)
    # O rust-toolchain.toml fixa a toolchain Windows (windows-gnu), que não roda
    # aqui: usa a stable nativa (RUSTUP_TOOLCHAIN tem precedência sobre o arquivo).
    export RUSTUP_TOOLCHAIN="${RUSTUP_TOOLCHAIN:-stable}"
    for d in "$HOME/.cargo/bin" /opt/homebrew/opt/rustup/bin /usr/local/opt/rustup/bin; do
      [ -d "$d" ] && PATH="$d:$PATH"
    done
    export PATH
    export ANDROID_HOME="${ANDROID_HOME:-$HOME/Library/Android/sdk}"
    if [ -z "${ANDROID_NDK_HOME:-}" ]; then
      ANDROID_NDK_HOME="$(ls -d "$ANDROID_HOME"/ndk/* 2>/dev/null | sort -V | tail -1 || true)"
    fi
    export ANDROID_NDK_HOME
    if [ -z "${JAVA_HOME:-}" ]; then
      for j in /opt/homebrew/opt/openjdk@21 /usr/local/opt/openjdk@21; do
        [ -d "$j" ] && export JAVA_HOME="$j/libexec/openjdk.jdk/Contents/Home" && break
      done
    fi
    ;;
esac

command -v cargo-ndk >/dev/null 2>&1 || { echo "instale: cargo install cargo-ndk"; exit 1; }
[ -n "${ANDROID_NDK_HOME:-}" ] || { echo "NDK não encontrado (defina ANDROID_NDK_HOME)" >&2; exit 1; }

echo "== 1/3 compilando a ponte para as ABIs do Android =="
( cd crates/bridge && cargo ndk \
    -t arm64-v8a -t armeabi-v7a -t x86_64 \
    -o "$HERE/android/app/src/main/jniLibs" build --release )

echo "== 2/3 pacote do veículo e imagem do carro =="
if [ -f "$PACK" ]; then
  cargo run --release -p sdd-import -- pack-export --pack "$PACK" --out "$HERE/assets/pack.json"
elif [ -f "$HERE/assets/pack.json" ]; then
  echo "(sem $PACK — usando o assets/pack.json já existente)"
else
  echo "Pacote não encontrado: $PACK" >&2
  echo "Gere no PC do SDD (cargo run -p sdd-import -- pack-export ...) ou copie de lá" >&2
  echo "o app/jlr_app/assets/pack.json já exportado." >&2
  exit 1
fi
mkdir -p "$HERE/assets/car"
if [ ! -f "$HERE/assets/car/d4.jpg" ]; then
  [ -f "$ROOT/app/assets/car/lr4_santorini_black.jpg" ] || bash "$ROOT/app/assets/car/fetch.sh" || true
  cp -f "$ROOT/app/assets/car/lr4_santorini_black.jpg" "$HERE/assets/car/d4.jpg" 2>/dev/null || true
fi

echo "== 3/3 flutter build apk =="
cd "$HERE"
flutter build apk --release
echo "APK: $HERE/build/app/outputs/flutter-apk/app-release.apk"
echo "Instalar (o -i faz o Android Auto aceitar o app fora da Play Store; ver ANDROID.md):"
echo "  adb install -r -i com.android.vending \"$HERE/build/app/outputs/flutter-apk/app-release.apk\""
