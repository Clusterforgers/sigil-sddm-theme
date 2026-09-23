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
Any non-digit key picks a layer at random, so mashing the keyboard lights the sigil up
unpredictably; 1-9 name a specific one. Blood ripples out from the middle and blue
lightning arcs across the formation, both without any input.

```
  any key      bloom a random layer
  1-9          bloom that specific layer
  Space        bloom every layer at once
  Enter        a heavy drop into the middle
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
- **The gather is precomputed.** `gpu::glow_pyramid` evaluates `highlight()` once per
  source pixel at startup and box-filters it into a mip chain, so level L holds the mean
  highlight over a 2^L box. The shader picks a level from the bloom radius and reads that
  neighbourhood average in four taps, instead of gathering the source with 24 every frame.

Cost at 2880x1800 on an Intel Arc iGPU, measured with `bench` — offscreen, so neither
vsync nor the compositor's idea of the window size gets in the way. Median of 7 runs,
interleaved with a build of the old shader so both see the same machine load:

| supersampling | idle, before | idle | blooming, before | blooming |
| --- | --- | --- | --- | --- |
| 1x1 | 2.5 ms | 0.7 ms | 4.7 ms | 1.1 ms |
| 2x2 | 9.9 ms | 1.4 ms | 10.0 ms | 1.8 ms |
| 3x3 | 18.4 ms | 3.5 ms | 21.1 ms | 3.5 ms |
| 4x4 | 30.8 ms | 5.1 ms | 33.7 ms | 6.3 ms |

The iGPU is shared with the compositor, so the absolute numbers move with whatever else is
on screen — on a quiet machine 2x2 idle measures 1.3 ms rather than 1.4. The ratio is the
stable part.

### The energy

Two things run on their own, independently of the gold keypress bloom.

**Blood.** A drop lands in the middle of something a good deal thicker than water. There
is a splash where it hits, then a ring travelling away from it with the surface still
rippling behind.

It is a plain circle, and the work went into making that circle *read* rather than into
giving it a shape. A ring is only ever as clear as its edges, so `pulse_at` hands `shade`
four separate pieces instead of one strength it could only fade out:

- **`body`** — the bulk, which is nearly black. `BLOOD` *mixes* over the stroke rather
  than adding to it, keeping rather less of the light that was there. An additive term can
  only ever brighten, which is why an early attempt came out orange.
- **`edge`** — the light caught along the leading rim, the one bright part of a wave of
  something this dark. It goes on last, so the dark body cannot mix it away.
- **`trough`** — the dip that runs ahead of the crest, as it does on water. Darkening
  there is what gives the ring a hard outer boundary; on artwork this sparse it is the
  only way to draw an edge at all.
- **`warp`** — the wave is a lens. It displaces what lies under it, most on its faces and
  not at all at the crest where the surface is flat, so the strokes visibly bulge and
  settle as the ring crosses them. This is what stops the ring reading as a colour laid
  over the artwork rather than as something happening to it. `q` is the rotated frame and
  rotation preserves radius, so displacing radially there needs no correction for the
  layer's own angle.

The wake decays as `exp(-s * 1.8)`, which holds it to two or three ripples. Letting it run
on any longer reddens the whole disc, and that — not the shape — was what stopped earlier
versions reading as a ring at all. The crest is dragged as it spreads
(`speed *= exp(-1.35 dt)`) and its amplitude falls as `1/sqrt(r)`, because a ring spreads
fixed energy over a growing circumference.

Which way the wake points depends on which way the wave runs. Getting that backwards puts
the wake in front of the wave, and an implosion then washes the middle of the disc before
it has arrived.

About one disturbance in five **implodes** instead: a ring starts at the rim and closes on
the centre, running faster and brighter the further in it gets — the same `1/sqrt(r)` that
fades an outgoing wave concentrates a converging one. When it lands, three things happen
at once: it throws the gathered strength back out as a much heavier drop, the whole disc
flashes, and a fan of nine bolts of lightning rushes out of the centre. **Enter** fires a
heavy drop directly.

**Lightning.** Blue-white bolts strike every 0.1-0.4 s, with the occasional lull. Each one
leaves from wherever the last one landed, so the arcing walks the formation instead of
flashing at unrelated places. About one jump in five is a long throw right across the disc;
being rare is what makes those land. Bolts fork about half the time, and each strike leaves
a blue afterglow where it hit.

The fan an implosion throws out is the exception: fixed origin, given direction, and it
deliberately leaves `spark_at` alone so the wandering lightning is not dragged to the
centre just because the disc collapsed. The nine arms are released a bolt at a time over
about a third of a second rather than fired together, which reads as energy *rushing* out
instead of one flash — and caps how many are alight at once, which is what keeps the cost
spike in hand. Measured peak: 51 limbs across 7 bolts.

Everything else is additive and shares one path: pulses and flares reduce to a scalar
strength per pixel and feed the same `spill()` as the keypress bloom, with all the tints
blended by strength, so lighting up for several reasons at once still costs one set of four
taps. Lightning is the exception, because a bolt is the light source rather than something
lighting the artwork: its hot core is drawn straight over whatever the pixel turned out to
be. Its limbs are tested per pixel, so the wash around them is given **compact support**
purely so each limb can be box-culled exactly — an inverse-square skirt looked much the
same and had a tail too long to cull, and the box test is worth ~25% of the frame at 24
limbs.

Both also land a little directly on the ground rather than only on the strokes (`0.05` and
`0.12`). Without it a passing ring reads as a row of briefly brighter glyphs instead of a
wave, and a bolt looks painted on rather than lighting the air it crosses. Small on purpose:
more turns either into a flat stripe over the artwork.

Cost at 2880x1800, `ss=2`, best of seven runs (the iGPU is shared with the compositor, so
a single run varies by ~30%):

| | ms/frame |
| --- | --- |
| idle | 1.3 |
| a drop spreading | 1.8 |
| an implosion landing: rebound, flash, the fan at its peak | 5.6 |
| that, with Space held down as well | 7.0 |

The fan is the expensive part, and its cost is set by how many limbs are alight together
rather than by how many bolts are fired — which is why they are staggered. The peak was
measured at runtime, not estimated: 51 limbs. Two tests pin the things that would
otherwise break silently — that the uniform block still fits `downlevel_defaults`, and
that the array lengths the WGSL declares still match the Rust ones.

### Activation

Letters and symbols light up in **groups** and lift off the plate. A seed glyph is picked
and its nearest neighbours go with it, each held back a little longer than the last, so the
cluster ripples outward from the seed rather than snapping on together. Groups are spaced
out in time on purpose — back to back they overlap into a permanent wash, which costs more
and reads as less.

The glyph **leaves its slot**: the copy is what rises, and where it came from goes dark
until it is finished. An earlier version kept the original in place and went out of its way
to avoid that gap, because a hole punched in the plate with nothing leaving it looked like
a bug. Now something is leaving, so the gap is the whole point.

The copy is drawn **outside the layer loop**, in un-rotated space, at the very end of
`shade` beside the arc cores. It used to be worked out in `q`, the layer-rotated frame, and
added inside the loop — so the moment it drifted across a band boundary it was clipped by
that boundary and displaced by the neighbouring band's different rotation. It needs the
angle of its own layer all the same (`glyph_r`), or the letter would sit at whatever angle
that layer happened to have reached and could face the wrong way up entirely.

As it rises it **grows, cools from gold through white to violet, and comes apart**, read
from an ever coarser mip as it travels. Two things about that are worth keeping:

- The growth divides the offset into the artwork. Inflating the falloff alone — which is
  what it did at first — only puts a bigger halo around a letter of unchanged size.
- The colour ramp is compressed into the window where the copy is actually visible. Spread
  over the whole life it reached violet exactly when the copy had already faded to nothing,
  so the change was there and could never be caught.

Nothing in the pipeline knew where a glyph *was* — the source is a picture and the shader
only ever asks it for the colour at a point. `glyphs::find` walks the artwork once at
startup and flood-fills the lit pixels, keeping the blobs small enough to be a glyph rather
than part of the ruling: **279** of them in 35 ms. A letter touching a cell divider merges
into the line network and is dropped, which is the right failure — a glyph floating off with
a piece of ruling attached would look like a mistake. Several names come back as whole
words, which reads better than single letters.

Two bounding boxes are tested before either glyph pass runs, one per frame: the glyphs are
hidden where they sit in the artwork, the copies are drawn in un-rotated space, and the two
do not coincide once the layers have turned.

### The canvas

What gets uploaded is not the photograph. The linework is **drawn over it** at twice its
size, so the rings and rules are paths rather than pixels; the lettering is still the
photograph until it too is drawn.

This is worth doing because magnification cannot be filtered away. Rendered at 2.88x, our
output is indistinguishable from a Lanczos upscale of the same region — both show the same
scalloped edge, because the source's own edge ramp is only ~1.5 px wide. Sampling cannot
recover an edge that was never captured. Drawing the line does, because a line is geometry.
The drawn stroke has to be at least as wide as the one underneath, or the ragged original
shows along its edges.

The texture is therefore denser than the coordinate space `layers.json` is written in. Every
lookup is in normalised uv so nothing downstream cares, but two things do: `src_size` stays
the coordinate space, and `source_lod` shifts by a level per doubling of the canvas.

Note also, measured rather than assumed: supersampling **does** still earn its keep while
magnifying — `ss=1` is four times further from an `ss=8` reference than `ss=2` — so it
cannot be traded away for speed.

### Sampling the artwork

The artwork is a raster, so how it is *read* matters as much as what is in it.

It carries a **mip chain**, built in `source_pyramid`. Without one the viewer reads level 0
whatever the window is doing, and at the sizes people actually use the disc is *minified* —
at 714x427 the fit scale is 0.357, so even at `ss=2` that is 0.71 texels per sample. Thin
gold lines then get point sampled and crawl. The chain costs nothing when the disc is
magnified and is the whole difference when it is not.

The filtering happens in **linear light**. The texture is `Rgba8UnormSrgb`, so averaging the
stored bytes would average the wrong quantity and quietly shift the artwork's brightness as
it shrank.

The level is worked out on the CPU (`source_lod`), because it depends only on the window and
because the sampler cannot derive it for itself inside a loop that breaks. The `-0.35` bias
on it is not a fudge: trilinear blends toward a full 2x2 box, a wider filter than the
footprint calls for, so the straight `-log2` over-blurs. Fitted against a brute-force `ss=8`
render, `-0.35` beats both level 0 and an unbiased level on closeness to that reference
*and* on high-frequency energy, at 714x427 and 500x300 alike:

| | closeness to `ss=8` | high-frequency energy |
| --- | --- | --- |
| level 0 (before) | 0.0067 | 0.406 |
| unbiased | 0.0098 | — |
| **-0.35** | **0.0050** | **0.384** |

The supersample pattern stays a uniform grid, deliberately. A rotated grid is the usual
improvement, but it only pays for geometric edges, and this renderer has none worth the
name: the disc cut and every layer boundary sit in empty gaps by construction, so all the
visible detail is texture. Measured, the rotated grid was simply a wider filter — slightly
less high-frequency energy, but further from the reference, which is the wrong trade when
the complaint is blur.

### Resolving the fields once per pixel

`pulse_at`, `bolt_at` and `flare_at` all live in un-rotated space, so none of them depends
on which layer a sub-sample lands in. They are resolved once at the pixel centre and shared,
instead of four times over at `ss=2` — and the limb list runs to fifty-odd entries during an
implosion, which made it far and away the dominant cost. The layer search and the artwork
lookup are still supersampled, because that is where the detail is.

It is guarded by the same disc test `shade` uses. Without that guard, hoisting hands the
empty frame outside the plate a bill it never used to pay, and for a handful of limbs that
costs more than the sharing saves — measured going the wrong way by 0.8 ms before the guard
went in.

Interleaved against the same build without it, at 1434x829 `ss=2`:

| | before | after |
| --- | --- | --- |
| idle | 1.33 ms | 1.18 ms |
| two ripples | 2.03 ms | 1.83 ms |
| + 6 limbs | 3.66 ms | 3.46 ms |
| + 36 limbs | 10.01 ms | **2.67 ms** |

The glyphs stay out of it: they are positioned in the artwork and ride their layer as it
turns, so they have to be tested in rotated space, per sub-sample. They get the bounding box
instead.

The bolt cores were the risk — hoisting means one sample per pixel on a thin bright
filament — but at RMSE 0.00025 the two are indistinguishable. They are four to six pixels
wide on screen, which is plenty.

In the viewer this is the difference between dropping a frame in most two-second intervals
and holding 120 fps. What is left is an isolated drop every few seconds, which tracks the
compositor and an animated wallpaper rather than anything here.

### Keeping the pixel cost down

A full-screen window is much larger than the disc, and the disc is mostly flat ground, so
the speed comes from not doing the work rather than doing it faster:

- **Stop outside the disc.** `params.y` holds the outermost boundary's radius. Past it
  there is nothing to draw, and on a full-screen window that is most of the pixels.
- **Bracket every boundary.** `pack` stores the smallest and largest radius each boundary
  reaches over all angles. A pixel's radius usually settles containment against those two
  numbers on its own, so `boundary_radius` — a `rem_euclid` and a `cos`, twice per layer —
  only runs where the bracket leaves the answer open.
- **Rotate the offset instead of un-rotating the angle.** The layer that wins needs the
  source point, which is just the pixel's offset from the centre rotated by that layer's
  angle. The CPU passes `sin`/`cos` in `motion.zw` so no pixel recomputes them.

With the precomputed bloom that is 5-7x at the supersampling the viewer uses, enough that
4x4 now costs about half of what 2x2 used to.

`bench` measures a fixed size offscreen, which the vsync-locked viewer cannot:

```
cargo run --release --bin bench -- --size 2880x1800 --ss 2 --bloom 1
cargo run --release --bin bench -- --ss 2 --pulses 2 --bolts 24 --flares 4  # energy lit
cargo run --release --bin bench -- --ss 2 --bloom 1 --out frame.png  # and check the picture
```

The fragment shader in `shaders/spin.wgsl` is otherwise the same inverse map as
`src/render.rs` — keep the two in step if you change `Boundary::radius_at`. The GIF path
does not bloom; it renders the artwork plain.

## Drawing the figure

`images/1.png` is a raster, and the disc occupies only 820 of its 2000 px, so a
full-screen window magnifies it 1.44x and the small text goes soft. `src/sigil.rs` draws
the figure instead, at any scale:

```
imagespin draw --scale 3 --out sigil.png
imagespin draw --over-source            # drawing in red over the source in green
imagespin glyphs                        # ring every letter and symbol that was found
```

So far it draws only the line work — the gold band, the two circles, the 40-cell ring, the
names heptagon, the heptagram, the nested heptagons and the pentagram. The text and the
crosses are still to come, so the viewer deliberately does not use it yet.

The radii came from a radial brightness sweep (406.5, 393.5, 384, 352) and from
`imagespin fit`, and they are **not** the ones in `layers.json`: those are cut lines,
placed in the empty gaps *between* drawn rings, which is what stops the animation tearing
through artwork.

One trap is worth knowing about. `Boundary::Poly(n, r, phase)` puts the **apothem** at
`phase` — a flat edge faces that way, with the vertices half a step to either side —
although `geom.rs` describes it as a vertex. Drawing a polygon half a step out of step
with the boundary meant to cut around it is invisible in a still and obvious the moment it
rotates. `imagespin check` catches it: against a correct drawing it reports every divider
clean, exactly as it does for the photograph.

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
