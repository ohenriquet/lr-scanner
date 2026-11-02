#!/usr/bin/env bash
# Build do app macOS (nativo) com a ponte FFI embutida.  >>> RODE NUM MAC <<<
#
# Requisitos no Mac:
#   - Rust (rustup) — compila a libjlr_bridge.dylib para a arquitetura do Mac
#   - Flutter + Xcode (command line tools) — compila o runner macOS
#   - O Mongoose aparece como /dev/cu.usbmodem* (CDC-ACM)
#
# Uso:  bash app/jlr_app/build-macos.sh [PACK.sqlite]
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"   # .../app/jlr_app
ROOT="$(cd "$HERE/../.." && pwd)"       # raiz do repo (port/)
PACK="${1:-$ROOT/packs/L319_201200.sqlite}"

cd "$ROOT"

echo "== 1/4 ponte nativa (libjlr_bridge.dylib, arquitetura do host) =="
cargo build --release -p jlr-bridge
DYLIB="$ROOT/target/release/libjlr_bridge.dylib"
[ -f "$DYLIB" ] || { echo "dylib não gerada: $DYLIB" >&2; exit 1; }

echo "== 2/4 exportando o pacote do veículo =="
if [ ! -f "$PACK" ]; then
  echo "Pacote não encontrado: $PACK" >&2
  echo "Gere no PC do SDD com: cargo run -p sdd-import -- vehicle --model L319 \\" >&2
  echo "  --year 201200 --engine V6DIESEL --engine-sub 3L --out packs/L319_201200.sqlite" >&2
  exit 1
fi
cargo run --release -p sdd-import -- pack-export --pack "$PACK" --out "$HERE/assets/pack.json"

echo "== 3/4 flutter build macos (release) =="
cd "$HERE"
flutter build macos --release

echo "== 4/4 embutindo a dylib no bundle e re-assinando (ad-hoc) =="
APP="$HERE/build/macos/Build/Products/Release/jlr_sdd.app"
FW="$APP/Contents/Frameworks"
mkdir -p "$FW"
cp -f "$DYLIB" "$FW/libjlr_bridge.dylib"
install_name_tool -id "@rpath/libjlr_bridge.dylib" "$FW/libjlr_bridge.dylib" 2>/dev/null || true
# A dylib é embutida após o build; re-assina ad-hoc pra o Gatekeeper não barrar.
codesign --force --deep --sign - "$APP" 2>/dev/null || true

echo
echo "Pronto: $APP"
echo "A ponte procura a dylib em Contents/Frameworks/ (ver lib/bridge.dart)."
echo "No app: Conectar -> informe a porta (ex.: /dev/cu.usbmodemXXXX)."
