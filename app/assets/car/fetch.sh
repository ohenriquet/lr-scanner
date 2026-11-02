#!/usr/bin/env bash
# Baixa os renders de estúdio do Land Rover Discovery 4 / LR4 por cor, usados
# na imagem do veículo do painel. Fonte: renders "color-matched" da Chrome Data
# via CDN da autobytel (os mesmos usados por sites de catálogo). Uso pessoal no
# app de diagnóstico; não são versionados no git (ver .gitignore).
#
# Uso: bash fetch.sh   (gera lr4_*.jpg = pré-facelift 2010-13 e fl_*.jpg = facelift 2014-16)
set -euo pipefail
cd "$(dirname "$0")"
UA="Mozilla/5.0 (Windows NT 10.0; Win64; x64)"

# Pré-facelift (Discovery 4 2010-13; estilo cc_2010lan005a). slug:código
PRE_BASE="https://img.autobytel.com/chrome/colormatched_01/white/640/cc_2010lan005a_01_640/cc_2010lan005a_01_640"
PRE="santorini_black:820 stornaway_grey:907 zermatt_silver:798 alaska_white:909 \
ipanema_sand:824 nara_bronze:825 bournville:822 buckingham_blue:796 izmir_blue:920 \
bali_blue:823 lugano_teal:963 galway_green:821 keswick_green:799 rimini_red:889"

# Facelift (LR4 2014-16; estilo cc_2016lrs090001). slug:código
FL_BASE="https://img.autobytel.com/chrome/colormatched_01/white/640/cc_2016lrs090001_01_640/cc_2016lrs090001_01_640"
FL="santorini_black:1ag barolo_black:1ae mariana_black:1ad corris_grey:1ab scotia_grey:1bc \
indus_silver:1ac yulong_white:1aq fuji_white:1aa aruba:1aj zanzibar:1bh loire_blue:1am \
aintree_green:1al firenze_red:1af montalcino_red:1ah"

for pair in $PRE; do
  curl -s -A "$UA" "${PRE_BASE}_${pair#*:}.jpg" -o "lr4_${pair%%:*}.jpg"
done
for pair in $FL; do
  curl -s -A "$UA" "${FL_BASE}_${pair#*:}.jpg" -o "fl_${pair%%:*}.jpg"
done
echo "OK: $(ls lr4_*.jpg fl_*.jpg | wc -l) imagens."
