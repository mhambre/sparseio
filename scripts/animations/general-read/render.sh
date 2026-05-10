#!/usr/bin/env bash

set -euo pipefail

ANIMATION_NAME="general-read"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../common.sh"

SCENE_FILE="$ROOT_DIR/scripts/animations/$ANIMATION_NAME/scene.py"
MEDIA_DIR="$ROOT_DIR/scripts/animations/$ANIMATION_NAME/.media"
OUTPUT_DIR="$ROOT_DIR/docs/static"
OUTPUT_GIF="$OUTPUT_DIR/$ANIMATION_NAME.gif"

mkdir -p "$MEDIA_DIR" "$OUTPUT_DIR"

"$PYTHON_BIN" -m manim -q l --fps 12 --media_dir "$MEDIA_DIR" "$SCENE_FILE" GeneralReadScene

VIDEO_FILE="$(find "$MEDIA_DIR/videos" -type f -name 'GeneralReadScene.mp4' | head -n 1)"
if [[ -z "$VIDEO_FILE" ]]; then
  echo "rendered video not found under $MEDIA_DIR/videos" >&2
  exit 1
fi

ffmpeg -y \
  -i "$VIDEO_FILE" \
  -vf "fps=12,scale=960:-1:flags=lanczos,split[s0][s1];[s0]palettegen=max_colors=128[p];[s1][p]paletteuse=dither=bayer:bayer_scale=3" \
  "$OUTPUT_GIF"

echo "wrote $OUTPUT_GIF"
