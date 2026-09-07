#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../common.sh"

QUALITY="${QUALITY:-m}"
FPS="${FPS:-30}"
MEDIA_DIR="$SCRIPT_DIR/.media"
OUTPUT_DIR="$ROOT_DIR/docs/static"
mkdir -p "$MEDIA_DIR" "$OUTPUT_DIR"

"$PYTHON_BIN" -m manim -q "$QUALITY" --fps "$FPS" \
  --media_dir "$MEDIA_DIR" --progress_bar none \
  "$SCRIPT_DIR/scene.py" CasScene

VIDEO_FILE="$MEDIA_DIR/videos/scene"
case "$QUALITY" in
  l) VIDEO_FILE="$VIDEO_FILE/480p${FPS}" ;;
  m) VIDEO_FILE="$VIDEO_FILE/720p${FPS}" ;;
  h) VIDEO_FILE="$VIDEO_FILE/1080p${FPS}" ;;
  p) VIDEO_FILE="$VIDEO_FILE/1440p${FPS}" ;;
  k) VIDEO_FILE="$VIDEO_FILE/2160p${FPS}" ;;
  *) echo "unsupported quality: $QUALITY" >&2; exit 1 ;;
esac

ffmpeg -v error -y -i "$VIDEO_FILE/CasScene.mp4" \
  -c copy -movflags +faststart "$OUTPUT_DIR/cas.mp4"

ffmpeg -v error -y -i "$OUTPUT_DIR/cas.mp4" \
  -vf "fps=12,scale=960:-1:flags=lanczos,split[s0][s1];[s0]palettegen=max_colors=128[p];[s1][p]paletteuse=dither=bayer:bayer_scale=3" \
  "$OUTPUT_DIR/cas.gif"

echo "wrote $OUTPUT_DIR/cas.gif and $OUTPUT_DIR/cas.mp4"
