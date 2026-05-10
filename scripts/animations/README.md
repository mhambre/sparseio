# Animations

This directory holds short `manim` scenes used to generate README-friendly animations for SparseIO.

## Layout

- `general-read/`: shows the core sparse read path for buffered and streamed viewer reads.

## Setup

```bash
python3 -m venv .venv
source .venv/bin/activate
python3 -m pip install --upgrade pip
python3 -m pip install manim==0.19.0
```

`ffmpeg` is also required for GIF output.

## Rendering

Render the general read animation:

```bash
./scripts/animations/general-read/render.sh
```

On first run, the shared animation bootstrap will create `scripts/animations/.venv`
and install `scripts/animations/requirements.txt` automatically.

That script renders a low-resolution scene for documentation and writes:

```text
docs/static/readme-general-read.gif
```