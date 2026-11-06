#!/usr/bin/env bash
# Gera os ícones do LR Scanner (Discovery 4 preta) para todas as plataformas a
# partir dos SVGs de tool/icon_gen.py. Requer: python3, rsvg-convert e magick
# (ImageMagick 7) — no Mac: brew install librsvg imagemagick.
#
# Uso: bash tool/make_icons.sh
set -euo pipefail

HERE="$(cd "$(dirname "$0")/.." && pwd)"   # .../app/jlr_app
BRAND="$HERE/assets/brand"
RES="$HERE/android/app/src/main/res"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

for t in python3 rsvg-convert magick; do
  command -v "$t" >/dev/null || { echo "falta: $t" >&2; exit 1; }
done

echo "== SVGs =="
python3 "$HERE/tool/icon_gen.py" "$BRAND"

png() { rsvg-convert -w "$2" -h "$2" "$BRAND/$1.svg" -o "$3"; }

echo "== Android (legado + adaptativo + monocromático) =="
for pair in mdpi:48 hdpi:72 xhdpi:96 xxhdpi:144 xxxhdpi:192; do
  d="${pair%%:*}"; s="${pair#*:}"; a=$(( s * 108 / 48 ))   # 108dp na mesma densidade
  mkdir -p "$RES/mipmap-$d"
  png icon_rounded "$s" "$RES/mipmap-$d/ic_launcher.png"
  png icon_fg "$a" "$RES/mipmap-$d/ic_launcher_foreground.png"
  png icon_bg "$a" "$RES/mipmap-$d/ic_launcher_background.png"
  png icon_mono "$a" "$RES/mipmap-$d/ic_launcher_monochrome.png"
done
mkdir -p "$RES/mipmap-anydpi-v26"
cat > "$RES/mipmap-anydpi-v26/ic_launcher.xml" <<'XML'
<?xml version="1.0" encoding="utf-8"?>
<!-- Gerado por tool/make_icons.sh: Discovery 4 preta sobre grafite/verde-água. -->
<adaptive-icon xmlns:android="http://schemas.android.com/apk/res/android">
    <background android:drawable="@mipmap/ic_launcher_background" />
    <foreground android:drawable="@mipmap/ic_launcher_foreground" />
    <monochrome android:drawable="@mipmap/ic_launcher_monochrome" />
</adaptive-icon>
XML

echo "== macOS =="
MAC="$HERE/macos/Runner/Assets.xcassets/AppIcon.appiconset"
for s in 16 32 64 128 256 512 1024; do png icon_macos "$s" "$MAC/app_icon_$s.png"; done

echo "== Windows (.ico) =="
for s in 16 24 32 48 64 128 256; do png icon_rounded "$s" "$TMP/w$s.png"; done
magick "$TMP"/w16.png "$TMP"/w24.png "$TMP"/w32.png "$TMP"/w48.png \
  "$TMP"/w64.png "$TMP"/w128.png "$TMP"/w256.png \
  "$HERE/windows/runner/resources/app_icon.ico"

echo "== Web =="
png icon_rounded 32 "$HERE/web/favicon.png"
png icon_rounded 192 "$HERE/web/icons/Icon-192.png"
png icon_rounded 512 "$HERE/web/icons/Icon-512.png"
# Maskable: fundo de ponta a ponta + carro na zona segura (= camadas do adaptativo).
for s in 192 512; do
  png icon_bg "$s" "$TMP/bg.png"; png icon_fg "$s" "$TMP/fg.png"
  magick "$TMP/bg.png" "$TMP/fg.png" -composite "$HERE/web/icons/Icon-maskable-$s.png"
done

echo "== Logo da AppBar =="
png icon_rounded 128 "$BRAND/logo.png"

echo "OK"
