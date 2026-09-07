# Animations

This directory holds [Manim](https://www.manim.community/) scenes used to
generate animations for SparseIO.

## Layout

- `general-read/`: shows the core sparse read path for buffered and streamed viewer reads.
- `cas/`: a short loop showing identical chunks from three upstreams sharing
  one cached copy. See its [storyboard](cas/README.md).

## Setup

```bash
python3 -m venv .venv
source .venv/bin/activate
python3 -m pip install --upgrade pip
python3 -m pip install -r requirements.txt
```

[FFmpeg](https://ffmpeg.org/) is also required for GIF output.

## Rendering

From the repository root:

```bash
just animations
just animations general-read
just animations cas
```

The default renders both animations. Each render script manages its virtual
environment and Python dependencies; FFmpeg must already be installed.

Render directly without Just:

```bash
./scripts/animations/{animation}/render.sh
```

```text
docs/static/{animation}.gif
```

The CAS scene exports both a GIF and a 720p, 30 fps MP4:

```bash
bash scripts/animations/cas/render.sh
QUALITY=l FPS=15 bash scripts/animations/cas/render.sh
```

The outputs are `docs/static/cas.gif` and `docs/static/cas.mp4`.
Use `QUALITY=h FPS=60` for a 1080p render. Rendering scripts manage their own
virtual environment under `scripts/animations/.venv`.
