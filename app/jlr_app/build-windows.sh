#!/usr/bin/env bash
# Build do app Windows (nativo) com a ponte de diagnóstico embutida.
#
# Requisitos no PC de build:
#   - Rust GNU + mingw (ver env.sh do repo)  -> constrói a jlr_bridge.dll
#   - Flutter + Visual Studio com workload "Desktop development with C++"
#     (MSVC v142/v143, C++ CMake tools, Windows 10/11 SDK) -> compila o runner
#
# Uso:  bash app/jlr_app/build-windows.sh [PACK.sqlite]
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"   # .../app/jlr_app
ROOT="$(cd "$HERE/../.." && pwd)"       # raiz do repo (port/)
PACK="${1:-$ROOT/packs/L319_201200.sqlite}"

cd "$ROOT"
# shellcheck disable=SC1091
source env.sh

echo "== 1/4 construindo a ponte nativa (jlr_bridge.dll) =="
cargo build --release -p jlr-bridge

echo "== 2/4 exportando o pacote do veículo para o app =="
if [ ! -f "$PACK" ]; then
  echo "Pacote não encontrado: $PACK" >&2
  echo "Gere com: cargo run -p sdd-import -- vehicle --model L319 --year 201200 \\" >&2
  echo "  --engine V6DIESEL --engine-sub 3L --out packs/L319_201200.sqlite" >&2
  exit 1
fi
cargo run --release -p sdd-import -- pack-export --pack "$PACK" \
  --out "$HERE/assets/pack.json"

echo "== 3/4 copiando a imagem do carro =="
mkdir -p "$HERE/assets/car"
cp -f "$ROOT/app/assets/car/lr4_santorini_black.jpg" "$HERE/assets/car/d4.jpg" \
  || echo "(imagem opcional ausente — rode app/assets/car/fetch.sh)"

echo "== 4/4 flutter build windows (release) =="
cd "$HERE"
flutter build windows --release

EXE="$HERE/build/windows/x64/runner/Release/jlr_sdd.exe"
echo
echo "Pronto: $EXE"
echo "A jlr_bridge.dll vai no mesmo diretório (bundle)."
echo "No app: Conectar -> informe a porta do Mongoose (ex.: COM5)."
