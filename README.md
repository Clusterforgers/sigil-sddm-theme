# imagespin

Turns a concentric sigil/mandala image into a seamless animated GIF where each layer
rotates independently.

Built for `images/1.png` — a *Sigillum Dei Aemeth*: an outer gold ring, a ring of
character cells, and a stack of nested heptagons around a pentagram core.

```
cargo build --release
./target/release/imagespin render          # -> spin.gif
```

## How it works

Rotation preserves radius, so the renderer never builds per-layer images. For each
output pixel it computes `(r, alpha)`, then for each layer un-rotates by that layer's
angle and tests whether the result falls inside the layer's region. First hit wins.
One pass, no compositing, no holes, nothing to seam.

Angles are measured **from North, counter-clockwise**: `dx = -r·sin α`, `dy = -r·cos α`.

### Layers

A layer is the region between two boundaries, each a `circle`, a `poly` (regular n-gon,
given as `[sides, circumradius, phase_deg]`) or a `star`. Because the artwork really is
nested polygons, polygon boundaries let each heptagon spin as a whole instead of being
sheared apart by a naive annulus cut.

Layers in `layers.json` are **chained** — each layer's inner boundary is literally the
next layer's outer boundary — so there are no gaps or overlaps and nothing can tear.

Two constraints worth knowing:

- Same-phase n-gons only nest when `r_inner <= cos(π/n) · r_outer` (0.901 for a
  heptagon). That rules out using every gap as a divider.
- `turns` is a **signed integer** count of revolutions per loop, and the angle is
  reduced modulo the frame count in integer arithmetic. Frame `N` is therefore
  bit-identical to frame 0 — the loop is seamless by construction, not by luck.

## Live viewer

```
direnv allow          # or: devenv shell
cargo run --release --bin live
```

The sigil turns at a **fixed** rate, read from layers.json so the window shows what the
GIF will render. Keys never steer the motion — they make parts of the diagram bloom.

```
  any key      bloom a layer
  1-9          bloom that specific layer
  Space        bloom every layer at once
  - / =        supersampling down / up (antialiasing vs framerate)
  F1           help                              Esc  quit
```

To change the speeds, edit `turns` in layers.json and restart — the same numbers drive
both the viewer and the GIF.

### The bloom

A keypress sets that layer's glow to 1.0; it decays exponentially (`BLOOM_DECAY`, ~0.55s)
so the fade is framerate-independent. The shader drives the stroke itself brighter and
gathers nearby highlights into a warm halo that spills into the dark ground.

Two details matter for it looking like light rather than fog:

- **The gather is thresholded** (`highlight()`). The artwork is dense fine lines, so
  averaging raw brightness over a disc lifts the whole band into a uniform grey veil.
  Dropping everything below the ground level means only the gold strokes spill.
- **Only the amount of light is sampled, not its colour**, then tinted with `GLOW`.
  Sampling colours directly drags neighbouring layers' artwork in as ghost detail.

Cost: 105 fps idle, 78 fps in the worst case where every layer blooms at once, at 2x2
supersampling on an Intel Arc iGPU at 2880x1800.

The fragment shader in `shaders/spin.wgsl` is otherwise the same inverse map as
`src/render.rs` — keep the two in step if you change `Boundary::radius_at`. The GIF path
does not bloom; it renders the artwork plain.

## Tuning a different image

```
imagespin fit --sides 7 --rmin 80 --rmax 400   # find polygons that match drawn lines
imagespin gaps --sides 7 --rmin 100 --rmax 160 # brightness vs radius: find empty gaps
imagespin preview                              # draw boundaries over the source
imagespin check                                # score each divider; low == in a gap
```

`fit` scores a hypothesis by mean brightness along its outline, so it locks onto real
drawn edges. `check` then confirms each chosen divider sits in empty space rather than
across artwork — that number is the thing to watch, not your eyes.

One expected exception: `circle 353` scores high because it lies on a drawn circle.
That is fine, and only for circles — a circle cut and rotated about its own centre is
visually identical.

## Output

`1920x1080`, 450 frames at 25 fps (18 s loop), 51 MB.

Layer periods: outer ring static; character cells and seven names 18 s per revolution;
heptagram, spirit names and core 9 s; cross band 6 s. The fastest layer advances
2.4° per frame.

`spin.mp4` is the same animation as h264 (crf 22, 11 MB) for anywhere a GIF is too heavy.

The GIF uses a **single global palette** built once from the source (rotation can only
resample colours the source already has), which avoids the frame-to-frame flicker of
per-frame quantisation. Frame 0 is full-size; later frames redraw only the disc's
bounding rect, with pixels that barely changed left transparent over `DisposalMethod::Keep`.

**GIF size scales linearly with frame count, and there is no way around it.** Rotating
by even a fraction of a degree repaints every antialiased edge in the artwork, so the
changed-pixel count stays near 26–29% no matter how small the per-frame step is. Frames
cost about 115 KB each at this resolution, full stop. Measured:

| frames (18 s loop) | size |
|---|---|
| 240 @ 20 fps (12 s) | 28 MB |
| 450 @ 25 fps (18 s) | 51 MB |

So the three quantities you actually trade are **duration, frame rate and resolution**:

- Loop length is `frames / fps`; a layer with `turns: t` takes `frames / (fps · t)`
  seconds per revolution. Scaling `frames` and `fps` together changes speed without
  touching the choreography.
- Slower at a fixed fps means more frames means a bigger file.
- Lower fps holds the size but makes the steps coarser.
- Halving `out_size` quarters the moving area — the only lever that buys a lot.

Palette knobs barely move the needle by comparison: `--colors 16` saves ~20% and bands
visibly, `--tol 600` saves ~8% and adds faint antialias ghosting. `--colors 32` (the
default) is indistinguishable from 64 at 3× zoom.

If the file needs to be small, encode video instead — h264 has interframe motion
compensation, which is exactly what this content wants and exactly what GIF lacks:

```
ffmpeg -i spin.gif -c:v libx264 -preset slow -crf 22 -pix_fmt yuv420p spin.mp4

# or straight from rendered frames, for best quality:
imagespin render --png-frames frames/
ffmpeg -framerate 25 -i frames/frame_%04d.png -c:v libx264 -crf 20 -pix_fmt yuv420p spin.mp4
```
