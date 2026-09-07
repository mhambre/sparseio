# CAS animation

A short, looping companion to `general-read` using the same Verdana font, navy
background, gold viewers, cyan source labels, green cache, and outlined chunks.
Labels are short; the animation carries the explanation. Each upstream chunk
group sits inside a folded-corner file outline labeled with its own filename.

Three viewers request equal chunks from S3, HTTPS, and Hugging Face. Each unseen chunk
travels into the cache and merges into the same stored A. The stored count stays
at one. A second round returns that cached chunk to all three viewers along
green branches labeled "Deduplicated fanout", without upstream transfers.
Distinct B and C chunks then arrive from HTTPS and Hugging Face,
occupy separate cache entries, and reach their respective viewers. The final
visual is `5 -> 3`: three copies of A plus unique B and C occupy three entries.

Only the highlighted chunks move. Other source chunks remain untouched.
Identical chunks must first be fetched before they can be recognized; CAS saves
stored payload bytes, while subsequent mapped reads avoid fetching again.
The three object-bound viewers share the same cache.

```bash
bash scripts/animations/cas/render.sh
```

Outputs:

- [cas.gif](../../../docs/static/cas.gif): looping 960 px GIF at 12 fps, matching
  the existing GIF's export settings.
- [cas.mp4](../../../docs/static/cas.mp4): 720p video at 30 fps by default.

Use `QUALITY=l FPS=15` for faster previews or `QUALITY=h FPS=60` for a 1080p video.
