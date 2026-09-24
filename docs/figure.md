# The figure file

The sigil is defined in `figure.json5`. It lists **layers**, and each layer lists the
**elements** drawn on it. Every layer is its own transparent image and turns as one piece,
so to move something to another layer, move its line.

```sh
cargo run -- check              # validate, and list the layers
cargo run -- draw --tint        # sigil.png, each layer in its own colour
cargo run -- layers             # layers/NN-name.png, one per layer
cargo run --bin live            # the animated viewer; reloads when you save
```

`--figure other.json5` works on another file (`live other.json5` for the viewer).

## Conventions

- **Units** are canvas units: the `canvas` is 2000×1125 and `center` is where every radius
  is measured from.
- **Angles** are degrees **clockwise from North** (12 o'clock).
- `rotate: 0` puts a vertex of a polygon or star at North. For a heptagon, `rotate: 25.71`
  (180/7) puts the middle of an edge there instead.
- **Items run clockwise**, starting at `rotate`.
- `turns` are whole revolutions per `loop`, **positive clockwise**. Whole numbers make the
  loop seamless.
- **Stacking**: a layer with a higher `z` draws on top (like CSS `z-index`). `z` is
  optional and defaults to 0; layers with the same `z` stack in the order they are written,
  later on top. `imagespin check` lists the layers in their final stacking order.
- **Every element starts with its `type`**, then its fields in any order.
- **Every element** accepts `color: "#RRGGBB"` (overriding the figure's `ink`) and
  `note: "…"` (ignored; use it to explain yourself).
- **Misspelled fields are errors**, not silently ignored.

## The file

```json5
{
  canvas: [2000, 1125],
  center: [1000, 562.5],
  background: "#0E0D0C",
  ink: "#FBB929",                 // default colour
  line_width: 1.6,                // default stroke width (optional)
  font: "fonts/DejaVuSerif.ttf",  // optional, relative to this file
  loop: { frames: 450, fps: 25 },
  layers: [
    { name: "outer ring", turns: 0, elements: [ /* … */ ] },
    { name: "star", turns: 2, z: 1, elements: [ /* … */ ] },  // z: drawn over the others
  ],
}
```

## Effects

The optional `effects` section sets how often each ambient effect happens. Times are
seconds between one occurrence and the next. Write a single number for an exact wait, or
`[min, max]` for a fresh random wait each time. Anything you leave out keeps its default.

```json5
effects: {
  implosions: { every: [12, 22] },     // a ring closing in from the rim, then a burst of lightning
  lightning:  { every: [0.08, 0.35], lulls: { chance: 0.22, last: [0.7, 1.6] } },
  glyphs:     { every: [1.2, 2.8] },   // a cluster of letters lighting up and lifting off
  surge:      { charge: 1.8, break: 1.4, hold: 1.2, reform: 1.6, stay_broken: false },
  shimmer:    { strength: 0.6, speed: 0.05 },   // always on: light gliding round the gilt
  haze:       { strength: 0.8 },                // always on: the ink wavering like heat haze
  colors:     { every: [6, 11], direction: "both", palette: ["#FF4A1C", "#9B5CFF", "#3FD0FF", "#FFF4E0"] },
  dissolve:   { every: [8, 14], duration: 3.2 },
  scramble:   { every: [3, 6], letters: 5, duration: 1.2 },
  breath:     { strength: 0.22, period: 6 },
  build:      { duration: 6, on_start: true, after_surge: true },
  echoes:     { strength: 1 },
  constellations: { every: [5, 9], stars: 5 },
  hover:      { strength: 0.8 },
  typing:     { enabled: true },
}
```

- **Turning an effect off:** `enabled: false` stops it, e.g. `glyphs: { enabled: false }`.
- **Lightning lulls:** after each strike there is a `chance` that the next wait is a lull
  lasting `last` instead of the usual `every`. Set `chance: 0` for a steady rattle.
- **The surge (Enter):** blood drops and the figure spins up and gathers light for `charge`
  seconds. Then it explodes in a white flash and a burst of lightning, and the layers fly
  apart and fade over `break`. It stays gone for `hold`, then comes back together over
  `reform`. With `stay_broken: true` it never comes back, which is what a login screen
  wants. Pressing Enter again while it runs does nothing.
- **Shimmer and haze** are always on. `shimmer` is a band of light that sweeps slowly
  round the figure and stays put while the layers turn under it, so the gilt catches it
  like real metal leaf. `speed` is in turns per second. `haze` makes the ink waver by up
  to `strength` canvas units, five times as much while the surge charges. A `strength`
  of 0 turns either one off.
- **Colour waves** roll out from the centre every `every` seconds. Each takes a colour
  from `palette` and recolours the gold ring by ring as it passes. The gold keeps its
  light and dark, so the lettering stays legible.
- **Colour direction:** `colors.direction` is `"out"` (from the centre), `"in"` (from the
  rim, slowing as it gathers at the centre) or `"both"` (picked afresh for each wave).
- **Dissolve** burns one layer away along a ragged, glowing edge and grows it back over
  `duration` seconds. The burn pattern turns with the layer.
- **Scramble** sets a run of up to `letters` neighbouring letters flickering through
  other letters of the figure for `duration` seconds, then gives the real ones back.
  Borrowed letters are about the same size as the ones they replace and stand the right
  way up. The words round the central cross are left alone.
- **Breath** is always on: the gold swells and dims by up to `strength` (0 to 1) once
  every `period` seconds, each ring a beat behind the one inside it, so the breath travels
  outward. `strength: 0` turns it off.
- **Build** draws the figure in over `duration` seconds. The lines are traced by a glowing
  pen, from the core outward, and the letters and symbols flash into place in a sweep
  round the figure. It runs at start (`on_start`), and redraws the figure as it reforms
  after the surge (`after_surge`). Other effects hold off until the figure is whole.
- **Echoes** are fading copies trailing a layer that spins fast, so they show during the
  surge's spin-up and not at the layers' own speeds. `strength: 0` turns them off.
- **Constellations** link `stars` letters (2 to 8) across different rings with glowing
  threads and a spark running along each, following the letters as the rings turn.
- **Hover:** ripples spread from the pointer as it moves, and letters near it glow.
  `strength: 0` turns it off.
- **Typing:** each character typed lights another letter of the ring with the most
  letters, stepping about a twelfth of the way round each time. Backspace puts the last
  one out, and the surge's explosion clears them all.
- **Keys in the viewer:** every printable key is typing, as on a login screen. Enter sets
  off the surge, and the rest of the keys are:

  | key | does |
  |---|---|
  | F2 | colour wave |
  | F3 | dissolve |
  | F4 | scramble |
  | F5 | constellation |
  | F6 | colour each layer |
  | F7 / F8 | supersampling down / up |
  | F9 | bloom every layer |
- **Previewing without a window:** `cargo run --release --bin bench -- --run 1.8
  --trigger color --out frame.png` renders 1.8 s after setting off a colour wave.
  `--trigger` sets off `surge`, `color`, `dissolve`, `scramble` or `constellation` at the
  start, or keeps up `typing` or `hover` through the run, starting from the whole figure.
  Leave it out to watch the figure draw itself in. `--surge 1.9` is short for `--run 1.9 --trigger surge`.
- **Live editing:** in the viewer, saving the file restarts every schedule, so a shorter
  wait takes effect at once.

## Tracks

A **track** is a closed curve around the centre that text and spokes follow:

```json5
{ circle: 262 }
{ polygon: { sides: 7, r: 318, rotate: 0 } }
```

## Elements

| `type` | draws | fields |
|---|---|---|
| `circle` | a ring, or a band if `width` is set | `r`, `width?` |
| `polygon` | a regular polygon | `sides`, `r` (to a vertex), `rotate?`, `width?` |
| `star` | a star polygon, each vertex joined to the one `skip` further round | `points`, `skip`, `r`, `rotate?`, `ribbon?`, `width?` |
| `spokes` | radial lines between two tracks | `count`, `from`, `to`, `rotate?`, `width?` |
| `text` | a ring of words along a track | `items`, `along`, `size`, `rotate?`, `offset?`, `line_gap?`, `flank?`, `orient?` |
| `label` | one piece of text, placed by hand | `text`, `size`, `at` |
| `symbols` | a ring of symbols | `symbol`, `count`, `size`, `r` or `along` or `fit`, `rotate?`, `offset?`, `run?` |
| `symbol` | one symbol | `symbol`, `size`, `at?`, `rotate?` |

```json5
{ type: "circle", r: 400, width: 13 }
{ type: "polygon", sides: 7, r: 217.5, rotate: 25.71 }
{ type: "star", points: 7, skip: 2, r: 314, ribbon: 28.2 }   // ribbon: a second star inset by 28.2
{ type: "spokes", count: 40, from: { circle: 353.6 }, to: { circle: 384 } }
```

### `text`

Each item gets one evenly spaced slot:

```json5
{
  type: "text", along: { circle: 363.75 }, size: 12, line_gap: 13.5, rotate: 4.5,
  items: [
    "n",          // one line, baseline on the track
    ["4", "T"],   // stacked lines, centred on the track, first one outermost
    "",           // an empty slot
    "{cross}",    // a symbol instead of text
  ],
}
```

- `offset` lifts the baselines off the track, measured square to it. On a polygon track the
  words then keep an even clearance from its edges.
- `flank: { count: 3, size: 10, symbol: "cross" }` sets a short run of symbols either side
  of every word.
- `orient: "radial"` reads outward along the radius instead of around the arc.

### `label`

```json5
{ type: "label", text: "Z", size: 26, at: { r: 74, angle: 342.9 } }  // on an arc
{ type: "label", text: "VA", size: 13, at: [0, -18] }                // square to the page, offset from the centre
```

### `symbols` and `symbol`

The symbols:

| name | looks like |
|---|---|
| `cross-potent` | ☩, straight arms each ending in a crossbar (the plate's own cross) |
| `cross` | a cross pattée, with arms flaring along concave curves |
| `latin-cross` | a plain Latin cross |
| `orb` | a ringed orb with a cross on top |
| `tower` | two posts joined by three rungs, crowned with a small cross |

Each symbol stands upright to the centre. The same names work inside text as
`"{cross-potent}"`, `"{orb}"` and so on.

A `symbols` ring sits in one of three ways:

```json5
// on a circle
{ type: "symbols", symbol: "cross-potent", count: 7, r: 236.9, size: 24 }
// along a track, lifted `offset` off it
{ type: "symbols", symbol: "cross-potent", count: 7, size: 10,
  along: { polygon: { sides: 7, r: 153.5, rotate: 25.71 } }, offset: 6.5 }
// hung from a circle and shrunk to fit the lens above a track
{ type: "symbols", symbol: "cross-potent", count: 7, size: 17, rotate: 6.2,
  fit: { roof: 388, floor: { polygon: { sides: 7, r: 388.5 } } } }
```

`run: { count: 6, step: 5.45 }` turns each of the `count` places into a short run of
symbols `step` degrees apart, centred on the place. The rows of crosses round each corner
of a heptagon use it.

```json5
{ type: "symbol", symbol: "latin-cross", size: 26 }   // one symbol, `at` offset from the centre
```

## When something is wrong

Loading stops at the first problem and says where it is:

```text
figure.json5:56:27: layers[2].elements[0].radius: unknown field `radius`, expected one of `r`, `width`, `color`, `note`
 56 |         { type: "circle", radius: 352 },
    |                           ^
```

```text
figure.json5: layers[2] "letter ring" › elements[1] (polygon): a polygon needs at least 3 `sides`, got 2
```

In the live viewer a broken save prints the error and keeps showing the last good figure.

## Adding an element type

1. Create `src/figure/elements/my_thing.rs`. It needs a struct deriving `Deserialize` with
   `#[serde(deny_unknown_fields)]`, holding its fields plus `color` and `note` like the
   others, and an `impl Draw` with `draw` (issue paths to the `Pen`), `check` (reject values
   that make no sense) and `color`.
2. In `src/figure/elements/mod.rs`, add `mod my_thing;` and one line to `registry!`:
   `MyThing(my_thing::MyThing) = "my-thing",`.
3. Use it: `{ type: "my-thing", … }`.

Geometry helpers are in `src/geometric/`: `shapes::{pt, polygon, star, closed_path, …}` and
`Track`. On the `Pen`:

- `line` strokes, and `fill` fills.
- `glyph` fills something the viewer's lit-letter effect may pick up.
- `text` and `text_at` write words.

A new symbol is a variant plus its outline in `src/figure/symbols.rs`.
