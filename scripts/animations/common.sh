#!/usr/bin/env bash

set -euo pipefail

ANIMATIONS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$ANIMATIONS_DIR/../.." && pwd)"
VENV_DIR="$ANIMATIONS_DIR/.venv"
PYTHON_BIN="$VENV_DIR/bin/python3"
REQUIREMENTS_FILE="$ANIMATIONS_DIR/requirements.txt"

ensure_animation_venv() {
  if [[ ! -x "$PYTHON_BIN" ]]; then
    echo "creating animation virtualenv at $VENV_DIR" >&2
    python3 -m venv "$VENV_DIR"
    "$PYTHON_BIN" -m pip install --upgrade pip
    "$PYTHON_BIN" -m pip install -r "$REQUIREMENTS_FILE"
  fi
}

ensure_animation_venv
