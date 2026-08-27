# Animations

This directory holds short [Manim](https://www.manim.community/) scenes used to
generate README-friendly animations for SparseIO.

## Layout

- `general-read/`: shows the core sparse read path for buffered and streamed viewer reads.

## Setup

```bash
python3 -m venv .venv
source .venv/bin/activate
python3 -m pip install --upgrade pip
python3 -m pip install -r requirements.txt
```

[FFmpeg](https://ffmpeg.org/) is also required for GIF output.

## Rendering

Render animation:

```bash
./scripts/animations/{animation}/render.sh
```

```text
docs/static/{animation}.gif
```
