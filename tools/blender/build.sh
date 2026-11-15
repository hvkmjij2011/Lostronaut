#!/bin/bash
# Rebuilds the whole asset set from code: tools/blender/build.sh [--only Name,...] [--no-icons] ...
# Needs the Blender 5.2 flatpak (org.blender.Blender) and ImageMagick for the contact sheets.
set -e
here="$(cd "$(dirname "$0")" && pwd)"
flatpak run org.blender.Blender -b --factory-startup -P "$here/build_assets.py" -- "$@" 2>&1 \
    | grep -E '^\[|Error|error|Traceback|File "|line [0-9]' || true
shots="$here/build/shots"
if ls "$shots"/*.png >/dev/null 2>&1; then
    montage -label '%t' -background '#2b2f36' -fill white -pointsize 14 -geometry 320x320+4+4 \
        "$shots"/*.png "$here/build/models_sheet.png" 2>/dev/null || true
fi
if ls "$here"/../../assets/icons/*.png >/dev/null 2>&1; then
    montage -label '%t' -background '#3a3f48' -fill white -pointsize 10 -geometry 128x128+6+6 -tile 9x \
        "$here"/../../assets/icons/*.png "$here/build/icons_sheet.png" 2>/dev/null || true
fi
