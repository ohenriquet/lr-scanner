#!/usr/bin/env bash
# Sobe o servidor local (car-scan serve) e abre o app no navegador.
# Uso:  bash run-app.sh [PORTA_COM]        (padrao: COM5)
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
cd "$HERE"
# shellcheck disable=SC1091
source env.sh

PORT="${1:-COM5}"
EXE="target/debug/car-scan.exe"
WEB="app/jlr_app/build/web"

if [ ! -x "$EXE" ]; then
  echo "[erro] $EXE ausente — rode: cargo build -p car-scan" >&2
  exit 1
fi
if [ ! -f "$WEB/index.html" ]; then
  echo "[erro] build web ausente em $WEB — rode: (em app/jlr_app) flutter build web" >&2
  exit 1
fi

# abre o navegador padrao ~2s depois, sem bloquear o servidor
( sleep 2; powershell -NoProfile -Command "Start-Process 'http://127.0.0.1:8080/'" >/dev/null 2>&1 ) &

echo "Mongoose em $PORT . Servidor em http://127.0.0.1:8080  (Ctrl-C para parar)"
exec "$EXE" --port "$PORT" serve --web "$WEB"
