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

# O rust-toolchain.toml do repo fixa a toolchain Windows (windows-gnu), que não
# roda no macOS. Aqui usamos a stable nativa do Mac (RUSTUP_TOOLCHAIN tem
# precedência sobre o arquivo).
export RUSTUP_TOOLCHAIN="${RUSTUP_TOOLCHAIN:-stable}"
# rustup do Homebrew é keg-only (não entra no PATH sozinho).
for d in "$HOME/.cargo/bin" /opt/homebrew/opt/rustup/bin /usr/local/opt/rustup/bin; do
  [ -d "$d" ] && PATH="$d:$PATH"
done
export PATH

echo "== 1/5 ponte nativa (libjlr_bridge.dylib, arquitetura do host) =="
cargo build --release -p jlr-bridge
DYLIB="$ROOT/target/release/libjlr_bridge.dylib"
[ -f "$DYLIB" ] || { echo "dylib não gerada: $DYLIB" >&2; exit 1; }

echo "== 2/5 exportando o pacote do veículo =="
if [ -f "$PACK" ]; then
  cargo run --release -p sdd-import -- pack-export --pack "$PACK" --out "$HERE/assets/pack.json"
elif [ -f "$HERE/assets/pack.json" ]; then
  echo "(sem $PACK — usando o assets/pack.json já existente)"
else
  echo "Pacote não encontrado: $PACK" >&2
  echo "Gere no PC do SDD com: cargo run -p sdd-import -- vehicle --model L319 \\" >&2
  echo "  --year 201200 --engine V6DIESEL --engine-sub 3L --out packs/L319_201200.sqlite" >&2
  echo "ou copie de lá o app/jlr_app/assets/pack.json já exportado." >&2
  exit 1
fi

echo "== 3/5 imagem do carro =="
mkdir -p "$HERE/assets/car"
if [ ! -f "$HERE/assets/car/d4.jpg" ]; then
  [ -f "$ROOT/app/assets/car/lr4_santorini_black.jpg" ] || bash "$ROOT/app/assets/car/fetch.sh"
  cp -f "$ROOT/app/assets/car/lr4_santorini_black.jpg" "$HERE/assets/car/d4.jpg"
fi

echo "== 4/5 flutter build macos (release) =="
cd "$HERE"
flutter build macos --release

echo "== 5/5 embutindo a dylib no bundle e re-assinando (ad-hoc) =="
APP="$HERE/build/macos/Build/Products/Release/LR Scanner.app"
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
