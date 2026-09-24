use crate::config::Config;
use crate::geom::Boundary;

use image::{Rgb, RgbImage};
use std::f32::consts::TAU;
use tiny_skia::{Color, LineCap, Paint, PathBuilder, Pixmap, Stroke, Transform};

/// The drawn figure. Separate from `Config` because it describes ink, not regions.
pub struct Figure {
    pub gold: [u8; 3],
    /// The thick outer band, as (inner radius, outer radius).
    pub band: (f32, f32),
    /// Free-standing thin circles.
    pub circles: Vec<f32>,
    /// The ring of lettered cells: (inner, outer, how many dividers).
    pub cells: (f32, f32, usize),
    /// Heptagons, as (circumradius, phase in degrees).
    pub heptagons: Vec<(f32, f32)>,
    /// The heptagram, which the plate draws as a woven ribbon rather than a line:
    /// circumradius of the ribbon's outer edge, phase, how many vertices each chord steps
    /// over, and the ribbon's width.
    pub heptagram: Option<(f32, f32, usize, f32)>,
    /// The pentagram at the core: circumradius, phase, ribbon width. Like the
    /// heptagram it is a ribbon, so it is stroked as a pair of stars.
    pub pentagram: (f32, f32, f32),
    /// Width of a thin line, in source pixels.
    pub hairline: f32,
    /// The seven names of God, on the edges of a heptagon: the names in order from the
    /// edge whose midpoint faces North, the heptagon they sit on, and the type size.
    /// Transcribed from `images/1.png`.
    pub names: (Vec<&'static str>, f32, f32, f32),
    /// What is written in the forty outer cells, clockwise from the divider due North:
    /// the outer row then the inner row. Either may be empty — six cells hold a single
    /// character, centred. Transcribed from `images/1.png`; see the note in `default`.
    pub cell_text: Vec<(&'static str, &'static str)>,
    /// Where those two rows sit and how big they are: (outer baseline, inner baseline,
    /// type size), all in source pixels.
    pub cell_rows: (f32, f32, f32),
    /// The ring of single letters, seven to a heptagon edge: (inner heptagon, outer
    /// heptagon, shared phase, baseline clearance above the inner one, type size). Both
    /// heptagons are drawn by `heptagons`; this only says which pair holds the lettering.
    pub letter_ring: (f32, f32, f32, f32, f32),
    /// Its forty-nine cells, clockwise from the vertex due North. A `+` is a cross.
    pub letter_text: Vec<&'static str>,
    /// Crosses in the lens between a circle and the heptagon inscribed in it: degrees
    /// clockwise of each vertex, the circle that roofs them, and the largest size. Each is
    /// then cut to fit the lens, which is narrow at a vertex and deep at an apothem.
    pub lens_crosses: (Vec<f32>, f32, f32),
    /// Where the drawing takes over from the photograph in `over`: sides, circumradius and
    /// phase of the boundary outside which the scan is discarded.
    ///
    /// The two must not both print, so `over` is a hand-over, not an overlay: outside this
    /// line every mark is drawn, inside it every mark is photographed, and the drawing
    /// leaves out whatever it places inside.
    pub handover: (f32, f32, f32),
    /// The four rings of spirit names, outside in: (heptagon stood on, baseline clearance,
    /// type size, crosses off each end of a name, cross size).
    ///
    /// The clearance is perpendicular to the edge, not radial, so a row keeps its distance
    /// from the heptagon below it and swings outward towards each corner. On a circle it
    /// would wander off its edge and cross a cut.
    pub spirit_rows: Vec<(f32, f32, f32, usize, f32)>,
    /// The names themselves, row by row, clockwise from the wedge due North.
    pub spirit_names: Vec<[&'static str; 7]>,
    /// Loose crosses, each drawn seven times: (radius, degrees clockwise of the wedge due
    /// North, size). Positions came from flood-filling the scan, so they are the plate's.
    pub crosses: Vec<(f32, f32, f32)>,
    /// Single letters in the core: (letter, baseline radius, degrees from North
    /// counter-clockwise, type size). Seven ring the pentagram and four sit in its arms.
    /// The plate places them by eye, so each carries its own angle rather than a step.
    pub core_letters: Vec<(&'static str, f32, f32, f32)>,
    /// The five names inside the pentagram: the names clockwise from the first, the radius
    /// they sit on, the phase of the first, and the type size.
    pub core_names: (Vec<&'static str>, f32, f32, f32),
    /// The words around the cross at the centre, as (word, x, y) offsets in source pixels,
    /// and their type size. Set square to the page, not to a radius.
    pub core_centre: (Vec<(&'static str, f32, f32)>, f32),
    /// Arm length and stroke width of that cross.
    pub core_cross: (f32, f32),
    /// In `over`, the radius inside which the scan is cleared because the core is drawn.
    pub core_clear: f32,
    /// Nominal canvas, in the coordinates `layers.json` is written in. The texture is
    /// this times the render scale; the coordinates never change.
    pub canvas: [u32; 2],
}

impl Default for Figure {
    /// Measured from `images/1.png`. The circles came out of a radial brightness sweep;
    /// the polygons were fitted by eye against the source and are the ones most worth
    /// re-checking if the drawing ever drifts.
    fn default() -> Self {
        Figure {
            gold: [251, 185, 41],
            band: (393.5, 406.5),
            circles: vec![384.0, 352.0],
            // Starts just clear of the cut at 353, not on the circle at 352: a divider
            // reaching across that cut leaves a one-unit sliver turning the other way.
            cells: (353.6, 384.0, 40),
            // Only what is really there and sits wholly inside one layer. Radii came from
            // ray sweeps at seven angles fitted to `r = apothem / cos(theta - phase)`; a
            // line fits at every angle, a row of crosses reading like one does not.
            // Heptagons at 139 and 171 are rows of crosses and are not drawn.
            //
            // Phases are `Boundary::Poly`'s: the apothem points at the phase, so 25.71
            // (= 180/7) is a vertex due North and 0 a flat edge there.
            heptagons: vec![
                (153.5, 0.0),
                (121.5, 0.0),
                (352.5, 25.71),
                // 318 where the plate has 314: the heptagram's points land on this
                // heptagon's vertices, and touching rings cannot turn at different rates.
                // Widening it by four opens a gap for the cut, at the cost of a small
                // notch at each of the seven points.
                (318.0, 25.71),
                // The heptagram ribbon's own envelopes, hence exactly tangent to it: its
                // edges come no closer than 195.8 and 167.6, the apothems of these two.
                (217.5, 0.0),
                (186.0, 0.0),
            ],


            heptagram: Some((314.0, 25.71, 2, 28.2)),
            pentagram: (108.6, 36.0, 3.6),
            hairline: 1.6,
            // Reading clockwise from the edge facing North. The heptagon they sit on is
            // the one whose vertices point North — the same 314/25.71 the figure is
            // built around — and they are set just inside it.
            names: (
                vec![
                    "SAAIEMES",
                    "BTZKASE",
                    "HEIDENE",
                    "DEIMOA",
                    "IAMEOGBE",
                    "ILAOVN",
                    "IHRLAA",
                ],
                262.0,
                -25.71,
                15.0,
            ),
            // Read off a polar unroll of the source at six times its resolution. The hand
            // is 16th-century and several characters are ambiguous; README lists which.
            cell_text: vec![
                ("4", "T"),  ("9", "G"),  ("7", "n"),  ("t", "9"),  ("22", "h"),
                ("n", ""),   ("6", "m"),  ("22", "o"), ("20", "a"), ("4", "n"),
                ("6", "a"),  ("h", ""),   ("18", "o"), ("23", "f"), ("l", "p"),
                ("n", ""),   ("l", "8"),  ("7", "G"),  ("13", "r"), ("H", "D"),
                ("og", ""),  ("y", "15"), ("t", "n"),  ("o", "8"),  ("e", "21"),
                ("10", "6"), ("11", "A"), ("15", "r"), ("8", "a"),  ("r", "16"),
                ("n", ""),   ("6", "A"),  ("o", "10"), ("s", "G"),  ("h", "14"),
                ("o", "17"), ("s", ""),   ("4", "5"),  ("a", "24"), ("6", "w"),
            ],
            // Both rows clear the cut at 353 and the circle at 384, descenders included:
            // a `p` on the inner row bottoms out at 354.2, the tightest fit in the drawing.
            cell_rows: (370.5, 357.0, 12.0),
            letter_ring: (318.0, 352.5, 25.71, 8.0, 20.0),
            // Seven to an edge, clockwise from the vertex due North. They follow the inner
            // heptagon, so they rise towards each vertex rather than sitting on a circle.
            letter_text: vec![
                "Z", "l", "l", "R", "H", "i", "a",
                "a", "Z", "C", "a", "a", "c", "b",
                "p", "a", "u", "p", "n", "h", "r",
                "h", "d", "m", "h", "i", "a", "t",
                "k", "k", "a", "a", "e", "e", "e",
                "i", "i", "e", "e", "l", "l", "l",
                "e", "e", "l", "l", "M", "G", "+",
            ],
            lens_crosses: (vec![6.2, 25.71, 45.2], 352.0, 17.0),
            // The plate's inner heptagon, not the drawing's 318: the scan must be cleared
            // up to where it stops being used, leaving the four units between empty.
            handover: (7.0, 314.0, 25.71),
            spirit_rows: vec![
                (217.5, 9.5, 13.0, 0, 0.0),
                (186.0, 9.5, 13.0, 1, 11.0),
                // Eleven, where the others clear by nine and a half: the cut runs four and
                // a half above this heptagon, and at nine and a half the crosses span it.
                (153.5, 11.0, 13.0, 3, 10.0),
                (121.5, 10.5, 13.0, 4, 9.0),
            ],
            // One in each of the heptagram's tip triangles, on the vertex itself.
            crosses: vec![(236.9, 0.0, 17.0)],
            // Angles measured off the scan one at a time: the plate places these by eye and
            // they are as much as seventeen degrees from any regular step.
            core_letters: vec![
                ("Z", 74.0, 17.06, 26.0),
                ("A", 74.0, 305.78, 26.0),
                ("B", 74.0, 250.58, 26.0),
                ("A", 74.0, 199.90, 26.0),
                ("L", 74.0, 158.93, 26.0),
                ("H", 74.0, 106.83, 26.0),
                ("E", 74.0, 54.84, 26.0),
                // In the pentagram's arms. Only four; the fifth arm carries `MADIMIEL`.
                ("Z", 50.0, 359.40, 15.0),
                ("C", 51.0, 72.00, 15.0),
                ("Z", 53.0, 145.80, 15.0),
                ("S", 53.0, 217.80, 15.0),
            ],
            // Centred between the pentagram's points, reading clockwise.
            core_names: (
                vec!["ORABIEL", "ZEDEKEL", "MADIMIEL", "SEMELIEL", "OCCAZIEL"],
                57.0,
                36.0,
                9.0,
            ),
            core_centre: (
                vec![("VA", 0.0, -18.0), ("LE", -20.0, 2.0), ("NA", 20.0, 2.0), ("EL", 0.0, 22.0)],
                13.0,
            ),
            core_cross: (13.0, 2.2),
            // The inner heptagon's apothem, less a whisker: the scan's copy of that line
            // has to survive, since the drawing does not own it while `over` is in use.
            core_clear: 109.0,
            // Read off a polar unroll of the four rings. The wedge due North holds single
            // letters rather than names, which is not a gap in the reading — the plate has
            // `El`, `I`, `S`, `E` there.
            spirit_names: vec![
                ["El", "Me", "Ese", "Iana", "Akele", "Azdobn", "Stimcul"],
                ["I", "Heeon", "Ih", "Beigia", "Ir", "Stimcul", "Dmal"],
                ["S", "Ab", "Ath", "Ized", "Ekiei", "Madimi", "Esemeli"],
                ["E", "An", "Ave", "Liba", "Rocle", "Hagonel", "Ilemese"],
            ],
            canvas: [2000, 1125],
        }
    }
}

/// A point at `(r, alpha)`, alpha from North and counter-clockwise — the same mapping
/// `geom.rs` documents and `render.rs` inverts.
fn pt(c: [f32; 2], r: f32, alpha: f32) -> (f32, f32) {
    (c[0] - r * alpha.sin(), c[1] - r * alpha.cos())
}

/// The vertices of a regular `n`-gon, first vertex at `phase` degrees from North.
fn vertices(c: [f32; 2], n: usize, r: f32, phase_deg: f32) -> Vec<(f32, f32)> {
    let phase = phase_deg.to_radians();
    (0..n).map(|i| pt(c, r, phase + TAU * i as f32 / n as f32)).collect()
}

/// The vertices of a regular `n`-gon, with the phase given the way `Boundary::Poly` means
/// it rather than the way it reads.
///
/// `Poly(n, r, phase)` puts the *apothem* at `phase` — a flat edge faces that way, the
/// vertices sit half a step either side — although `geom.rs` calls it a vertex. So phase
/// 25.71 is a vertex due North and phase 0 a flat edge there. `Figure` keeps the same
/// convention as `fit` and `layers.json`; the half-step correction lives here.
fn poly_vertices(c: [f32; 2], n: usize, r: f32, poly_phase_deg: f32) -> Vec<(f32, f32)> {
    let half_step = 180.0 / n as f32;
    vertices(c, n, r, poly_phase_deg + half_step)
}

/// The vertices of an `{n/skip}` star polygon, in the order they are joined.
///
/// Stepping `skip` at a time closes only after all `n` are visited when the two are
/// coprime, which makes one continuous star rather than a ring of triangles.
fn star_points(c: [f32; 2], n: usize, r: f32, poly_phase_deg: f32, skip: usize) -> Vec<(f32, f32)> {
    let v = poly_vertices(c, n, r, poly_phase_deg);
    let mut order = Vec::with_capacity(n);
    let mut i = 0usize;
    for _ in 0..n {
        order.push(v[i]);
        i = (i + skip) % n;
    }
    order
}

/// The circumradius of the star sitting `width` inside an `{n/skip}` star of circumradius
/// `r`, perpendicular to the chords.
///
/// Offsetting every chord inwards scales the star, so the result is a star again and the
/// ribbon between the two has parallel sides.
fn star_inside(r: f32, skip: usize, n: usize, width: f32) -> f32 {
    r - width / (std::f32::consts::PI * skip as f32 / n as f32).cos()
}

/// Clear space between a name and the crosses running off it, in source pixels along the
/// edge. Small: what keeps a ring from filling up is that the run is short.
const MARGIN: f32 = 3.0;

/// The polygon `d` outside `r`, perpendicular to its edges.
///
/// Offsetting every edge outwards grows the apothem by the same amount, so the result is a
/// polygon again and anything laid along it keeps a constant clearance.
fn poly_outside(r: f32, n: usize, d: f32) -> f32 {
    r + d / (std::f32::consts::PI / n as f32).cos()
}

/// A slice of the band between two heptagons, from `a0` round to `a1`. Sampled rather than
/// built from the corners, since a slice rarely starts or stops on one.
fn band_slice(c: [f32; 2], lo: &Boundary, hi: &Boundary, a0: f32, a1: f32) -> Option<tiny_skia::Path> {
    let steps = 48;
    let mut pts = Vec::with_capacity(steps * 2 + 2);
    for i in 0..=steps {
        let a = a0 + (a1 - a0) * i as f32 / steps as f32;
        pts.push(pt(c, hi.radius_at(a), a));
    }
    for i in (0..=steps).rev() {
        let a = a0 + (a1 - a0) * i as f32 / steps as f32;
        pts.push(pt(c, lo.radius_at(a), a));
    }
    closed_path(&pts)
}

/// A cross pattée: four arms flaring from a narrow waist to a wide tip, `size` across and
/// turned `turn` radians from upright. Drawn rather than set from a font, because it has to
/// be sized and turned to the cell it sits in.
fn cross_path(x: f32, y: f32, size: f32, turn: f32) -> Option<tiny_skia::Path> {
    let l = size * 0.5;
    let w = size * 0.11;
    let f = size * 0.35;
    // One continuous outline: waist corner, out to the two corners of a tip, back to the
    // next waist corner, four times round.
    let pts = [
        (w, w), (f, l), (-f, l),
        (-w, w), (-l, f), (-l, -f),
        (-w, -w), (-f, -l), (f, -l),
        (w, -w), (l, -f), (l, f),
    ];
    let mut pb = PathBuilder::new();
    pb.move_to(pts[0].0, pts[0].1);
    for p in &pts[1..] {
        pb.line_to(p.0, p.1);
    }
    pb.close();
    let p = pb.finish()?;
    p.transform(Transform::from_translate(x, y).pre_rotate(turn * 180.0 / std::f32::consts::PI))
}

/// Where a cross goes in the lens between `roof` and the heptagon `below` it, and how big
/// it can be there: offset from the centre, then size.
///
/// The lens closes to nothing at each vertex and opens to its deepest at an apothem, so a
/// cross set at one size would either burst the narrow end or rattle around in the wide
/// one. It hangs from the roof either way, which is how the figure has them.
fn lens_cross(below: &Boundary, roof: f32, biggest: f32, alpha: f32) -> (f32, f32, f32) {
    // 1.6, not 1.0: the figure lets a cross spill well past the heptagon under it rather
    // than shrink to nothing, and only the very tips of the lens really pinch.
    let size = biggest.min((roof - below.radius_at(alpha)) * 1.6).max(1.0);
    let r = roof - size * 0.5 - 1.5;
    (-r * alpha.sin(), -r * alpha.cos(), size)
}

fn closed_path(pts: &[(f32, f32)]) -> Option<tiny_skia::Path> {
    let mut pb = PathBuilder::new();
    pb.move_to(pts[0].0, pts[0].1);
    for p in &pts[1..] {
        pb.line_to(p.0, p.1);
    }
    pb.close();
    pb.finish()
}

/// Draw the figure alone, at `scale` times the canvas `cfg` is written in.
pub fn draw(cfg: &Config, fig: &Figure, scale: f32) -> RgbImage {
    render(cfg, fig, scale, None)
}

/// Draw the linework over the photograph, at `scale` times its size.
///
/// Interim, until the whole figure is drawn. Magnifying a 2000px raster cannot invent an
/// edge that was never captured; drawing the same line as a path can, because it is
/// geometry. Drawn lines must be at least as wide as the ones underneath.
pub fn over(cfg: &Config, fig: &Figure, src: &RgbImage, scale: f32) -> RgbImage {
    render(cfg, fig, scale, Some(src))
}

/// The geometry is emitted at nominal size and scaled by the transform, so the constants
/// above stay readable against `layers.json` however large the texture gets.
fn render(cfg: &Config, fig: &Figure, scale: f32, base: Option<&RgbImage>) -> RgbImage {
    let w = (fig.canvas[0] as f32 * scale).round() as u32;
    let h = (fig.canvas[1] as f32 * scale).round() as u32;
    let mut pm = Pixmap::new(w, h).expect("canvas too large");

    // What the drawing is responsible for. When there is no photograph it is everything;
    // when there is one, only what lies outside the hand-over, or the two print on top of
    // each other. Elements are judged by how far in they reach, which is why every call
    // below passes its *minimum* radius and not its nominal one.
    let hand = Boundary::Poly(fig.handover.0, fig.handover.1, fig.handover.2);
    let floor = fig.handover.1 * (std::f32::consts::PI / fig.handover.0).cos() - 0.01;
    let owned = |min_r: f32| base.is_none() || min_r >= floor;

    // The heptagram reaches most of the way to the middle, so unlike the rings above it
    // it cannot be judged by a radius.
    let star_drawn = fig.heptagram.is_some();
    // The core is cleared wholesale rather than mark by mark, so everything inside that
    // radius is the drawing's.
    let core_owned = |max_r: f32| base.is_none() || max_r <= fig.core_clear;

    // Where the heptagram's lines land, so the scan is cleared along them and nowhere
    // else: the plate sets crosses inside the ribbon, and those are still the scan's to
    // draw. Five wide, to take the scan's own two-pixel line with it.
    let mut ribbon = Pixmap::new(w, h).expect("canvas too large");
    if let (Some((hr, hphase, skip, width)), true) = (fig.heptagram, base.is_some()) {
        let ink = Paint { anti_alias: true, ..Default::default() };
        let wipe = Stroke { width: 5.0, ..Stroke::default() };
        let xf0 = Transform::from_scale(scale, scale);
        for r in [hr, star_inside(hr, skip, 7, width)] {
            if let Some(p) = closed_path(&star_points(cfg.center, 7, r, hphase, skip)) {
                ribbon.stroke_path(&p, &ink, &wipe, xf0, None);
            }
        }
    }

    // ...and where the spirit rings land: each name's own slice, plus the gap after it
    // only where crosses fill that gap. The top ring leaves its gaps to the heptagram.
    if base.is_some() && !fig.spirit_rows.is_empty() {
        let ink = Paint { anti_alias: true, ..Default::default() };
        let xf0 = Transform::from_scale(scale, scale);
        let wind = tiny_skia::FillRule::Winding;
        let f2 = crate::text::Font::load();
        for (row, names) in fig.spirit_rows.iter().zip(&fig.spirit_names) {
            let &(hr, lift, size, per_gap, _) = row;
            let lo = Boundary::Poly(7.0, poly_outside(hr, 7, 2.5), 0.0);
            let hi = Boundary::Poly(7.0, poly_outside(hr, 7, lift + size + 3.0), 0.0);
            let line = Boundary::Poly(7.0, poly_outside(hr, 7, lift), 0.0);
            let half = |k: usize| {
                let a = crate::text::spoke(k, 7, 0.0);
                f2.width(names[k], size) * 0.5 / line.radius_at(a) + MARGIN / line.radius_at(a)
            };
            for k in 0..7 {
                let a = crate::text::spoke(k, 7, 0.0);
                if let Some(p) = band_slice(cfg.center, &lo, &hi, a - half(k), a + half(k)) {
                    ribbon.fill_path(&p, &ink, wind, xf0, None);
                }
                if per_gap > 0 {
                    let b = crate::text::spoke(k + 1, 7, 0.0);
                    if let Some(p) = band_slice(cfg.center, &lo, &hi, b + half((k + 1) % 7), a - half(k)) {
                        ribbon.fill_path(&p, &ink, wind, xf0, None);
                    }
                }
            }
        }
    }

    match base {
        // Resample the photograph up to the canvas first, so the drawn lines land on top
        // of it. Nothing is gained in the lettering by doing this — it is already as
        // detailed as it will ever be — but the lines drawn over it are not limited to
        // the raster grid.
        Some(src) => {
            let up = image::imageops::resize(src, w, h, image::imageops::FilterType::Lanczos3);
            let bg = crate::config::parse_hex(&cfg.background);
            let (cx, cy) = (cfg.center[0] * scale, cfg.center[1] * scale);
            for (i, px) in up.pixels().enumerate() {
                let (x, y) = ((i as u32 % w) as f32, (i as u32 / w) as f32);
                let (dx, dy) = (x - cx, y - cy);
                let r = (dx * dx + dy * dy).sqrt() / scale;
                let alpha = (-dx).atan2(-dy);
                // Outside the hand-over the drawing speaks for itself.
                let on_star = star_drawn
                    && ribbon.pixel(i as u32 % w, i as u32 / w).is_some_and(|q| q.alpha() > 100);
                let px = if r >= hand.radius_at(alpha) || on_star || r < fig.core_clear {
                    &bg
                } else {
                    &px.0
                };
                let o = i * 4;
                pm.data_mut()[o] = px[0];
                pm.data_mut()[o + 1] = px[1];
                pm.data_mut()[o + 2] = px[2];
                pm.data_mut()[o + 3] = 255;
            }
        }
        None => {
            let bg = crate::config::parse_hex(&cfg.background);
            pm.fill(Color::from_rgba8(bg[0], bg[1], bg[2], 255));
        }
    }

    let mut paint = Paint::default();
    paint.set_color(Color::from_rgba8(fig.gold[0], fig.gold[1], fig.gold[2], 255));
    paint.anti_alias = true;

    let xf = Transform::from_scale(scale, scale);
    let c = cfg.center;

    let stroke = Stroke { width: fig.hairline, line_cap: LineCap::Round, ..Stroke::default() };

    // The thick outer band, drawn as one stroked circle down its middle rather than as a
    // filled annulus — one path, and the width is then literally the measured thickness.
    let (bi, bo) = fig.band;
    if let Some(p) = PathBuilder::from_circle(c[0], c[1], (bi + bo) * 0.5).filter(|_| owned(bi)) {
        let band = Stroke { width: bo - bi, ..stroke.clone() };
        pm.stroke_path(&p, &paint, &band, xf, None);
    }

    for &r in &fig.circles {
        if let Some(p) = PathBuilder::from_circle(c[0], c[1], r).filter(|_| owned(r)) {
            pm.stroke_path(&p, &paint, &stroke, xf, None);
        }
    }

    // Radial dividers of the lettered ring.
    let (ci, co, n) = fig.cells;
    let mut pb = PathBuilder::new();
    let cells_owned = owned(ci);
    for i in 0..n {
        let a = TAU * i as f32 / n as f32;
        let (x0, y0) = pt(c, ci, a);
        let (x1, y1) = pt(c, co, a);
        pb.move_to(x0, y0);
        pb.line_to(x1, y1);
    }
    if let Some(p) = pb.finish().filter(|_| cells_owned) {
        pm.stroke_path(&p, &paint, &stroke, xf, None);
    }

    for &(r, phase) in &fig.heptagons {
        if let Some(p) = closed_path(&poly_vertices(c, 7, r, phase)).filter(|_| owned(r * (std::f32::consts::PI / 7.0).cos())) {
            pm.stroke_path(&p, &paint, &stroke, xf, None);
        }
    }

    // The heptagram. The plate draws it as a ribbon, so what is stroked is a pair of stars
    // rather than one: offsetting a chord inwards by the ribbon's width gives another star
    // of the same phase, smaller by the width divided by the cosine of the step — see
    // `star_inside`.
    if let Some((hr, hphase, skip, width)) = fig.heptagram {
        for r in [hr, star_inside(hr, skip, 7, width)] {
            if let Some(p) = closed_path(&star_points(c, 7, r, hphase, skip)) {
                pm.stroke_path(&p, &paint, &stroke, xf, None);
            }
        }
    }

    // The pentagram, the same way: a 5-gon stepped two at a time, and a ribbon again.
    let (pr, pphase, pwidth) = fig.pentagram;
    if core_owned(pr) {
        for r in [pr, star_inside(pr, 2, 5, pwidth)] {
            if let Some(p) = closed_path(&star_points(c, 5, r, pphase, 2)) {
                pm.stroke_path(&p, &paint, &stroke, xf, None);
            }
        }
    }


    // Everything lettered, from one parsed face.
    let font = crate::text::Font::load();
    let fill = tiny_skia::FillRule::Winding;
    let write = |pm: &mut Pixmap, a: f32, r: f32, size: f32, s: &str| {
        for p in font.place(c, a, r, size, s, crate::text::Along::Arc) {
            pm.fill_path(&p, &paint, fill, xf, None);
        }
    };

    // The seven names. Each sits on the edge whose midpoint it is centred on, which for
    // the vertices-North heptagon is half a step round from each vertex.
    let (ref names, nr, nphase, nsize) = fig.names;
    for (i, name) in names.iter().enumerate().filter(|_| owned(nr)) {
        write(&mut pm, crate::text::spoke(i, names.len(), nphase), nr, nsize, name);
    }

    // The lettered ring. Cell `i` is the one clockwise of the divider due North, so its
    // middle is half a step round — the same half-step the dividers above are drawn on.
    let (outer_row, inner_row, size) = fig.cell_rows;
    for (i, &(top, bottom)) in fig.cell_text.iter().enumerate().filter(|_| cells_owned) {
        let a = crate::text::spoke(i, n, -180.0 / n as f32);
        // A cell holding a single character centres it rather than leaving a gap where
        // the other row would be.
        if bottom.is_empty() {
            write(&mut pm, a, (outer_row + inner_row) * 0.5, size, top);
        } else {
            write(&mut pm, a, outer_row, size, top);
            write(&mut pm, a, inner_row, size, bottom);
        }
    }

    // The ring of single letters. Its cells are wedges between two heptagons that share a
    // phase, so the band is thin at the apothem and deep at each vertex; the lettering has
    // to follow that rather than sit on a circle, or the cells near a vertex would hold a
    // letter floating in the middle of nowhere while the ones at an apothem overflowed.
    let (lr_in, lr_out, lr_phase, lr_lift, lr_size) = fig.letter_ring;
    let inner = Boundary::Poly(7.0, lr_in, lr_phase);
    let outer = Boundary::Poly(7.0, lr_out, lr_phase);
    let cells = fig.letter_text.len();
    if cells > 0 && owned(lr_in * (std::f32::consts::PI / 7.0).cos()) {
        let mut pb = PathBuilder::new();
        for k in 0..cells {
            // A divider on every cell edge, including the one at each heptagon vertex:
            // the two heptagons do not meet there, so the band does not close itself.
            let a = crate::text::spoke(k, cells, 0.0);
            let (x0, y0) = pt(c, inner.radius_at(a), a);
            let (x1, y1) = pt(c, outer.radius_at(a), a);
            pb.move_to(x0, y0);
            pb.line_to(x1, y1);
        }
        if let Some(p) = pb.finish() {
            pm.stroke_path(&p, &paint, &stroke, xf, None);
        }

        for (k, ch) in fig.letter_text.iter().enumerate() {
            let a = crate::text::spoke(k, cells, -180.0 / cells as f32);
            let base = inner.radius_at(a) + lr_lift;
            if *ch == "+" {
                let mid = (base + outer.radius_at(a)) * 0.5;
                let (x, y) = pt(c, mid, a);
                if let Some(p) = cross_path(x, y, lr_size * 1.3, -a) {
                    pm.fill_path(&p, &paint, fill, xf, None);
                }
            } else {
                write(&mut pm, a, base, lr_size, ch);
            }
        }
    }

    // The four rings of spirit names. Each name is centred on the middle of one heptagon
    // edge, and the gap to the next is filled with crosses spaced along that edge.
    for (row, names) in fig.spirit_rows.iter().zip(&fig.spirit_names) {
        let &(hr, lift, size, per_side, cross) = row;
        let line = Boundary::Poly(7.0, poly_outside(hr, 7, lift), 0.0);
        // Where each name starts and stops, as an angle, so the crosses can be spaced
        // between them rather than through them.
        let half: Vec<f32> = names
            .iter()
            .enumerate()
            .map(|(k, n)| {
                let a = crate::text::spoke(k, 7, 0.0);
                font.width(n, size) * 0.5 / line.radius_at(a)
            })
            .collect();

        for (k, name) in names.iter().enumerate() {
            let a = crate::text::spoke(k, 7, 0.0);
            write(&mut pm, a, line.radius_at(a), size, name);

            // The crosses hug the name rather than filling the gap: the plate sets a short
            // run of them touching at the arms on either side and then leaves the middle of
            // each edge empty. Spread evenly across the gap instead and every ring turns
            // into a solid band of crosses, which is what this looked like at first.
            for side in [-1.0f32, 1.0] {
                for i in 0..per_side {
                    let along = MARGIN + cross * (1.02 * i as f32 + 0.5);
                    let b = a + side * (half[k] + along / line.radius_at(a));
                    let r = line.radius_at(b);
                    let (x, y) = pt(c, r, b);
                    if let Some(p) = cross_path(x, y, cross, -b) {
                        pm.fill_path(&p, &paint, fill, xf, None);
                    }
                }
            }
        }
    }

    // The core: the ring of letters, the five names inside the pentagram, and the words
    // around the cross at the centre.
    if core_owned(fig.core_clear) {
        for &(ch, r, deg, size) in &fig.core_letters {
            write(&mut pm, deg.to_radians(), r, size, ch);
        }
        let (ref cn, cr, cphase, csize) = fig.core_names;
        for (k, name) in cn.iter().enumerate() {
            write(&mut pm, crate::text::spoke(k, cn.len(), cphase), cr, csize, name);
        }
        let (ref words, wsize) = fig.core_centre;
        for &(w, dx, dy) in words {
            for p in font.place_at(c[0] + dx, c[1] + dy, wsize, w) {
                pm.fill_path(&p, &paint, fill, xf, None);
            }
        }
        // A plain Latin cross, not a pattÃ©e: the one at the centre has bare arms.
        let (arm, wide) = fig.core_cross;
        let mut pb = PathBuilder::new();
        pb.move_to(c[0], c[1] - arm);
        pb.line_to(c[0], c[1] + arm);
        pb.move_to(c[0] - arm * 0.62, c[1] - arm * 0.25);
        pb.line_to(c[0] + arm * 0.62, c[1] - arm * 0.25);
        if let Some(p) = pb.finish() {
            let bar = Stroke { width: wide, ..stroke.clone() };
            pm.stroke_path(&p, &paint, &bar, xf, None);
        }
    }

    // Loose crosses, seven of each.
    for &(r, off, size) in &fig.crosses {
        if owned(r - size * 0.5) {
            for k in 0..7 {
                let a = crate::text::spoke(k, 7, -off);
                let (x, y) = pt(c, r, a);
                if let Some(p) = cross_path(x, y, size, -a) {
                    pm.fill_path(&p, &paint, fill, xf, None);
                }
            }
        }
    }

    // Crosses in the lens above the letter ring. Nothing else is drawn between the circle
    // and the heptagon inscribed in it, and the space is a different shape at every angle,
    // so each cross is cut to fit rather than set at one size.
    let (ref offsets, roof, biggest) = fig.lens_crosses;
    for v in (0..7).filter(|_| owned(lr_out * (std::f32::consts::PI / 7.0).cos())) {
        for d in offsets {
            let a = crate::text::spoke(v, 7, -d);
            let (x, y, size) = lens_cross(&outer, roof, biggest, a);
            let (x, y) = (c[0] + x, c[1] + y);
            if let Some(p) = cross_path(x, y, size, -a) {
                pm.fill_path(&p, &paint, fill, xf, None);
            }
        }
    }

    // tiny-skia hands back premultiplied RGBA, but every pixel here is opaque, so the
    // channels are already straight.
    let mut out = RgbImage::new(w, h);
    for (i, px) in pm.pixels().iter().enumerate() {
        let (x, y) = (i as u32 % w, i as u32 / w);
        out.put_pixel(x, y, Rgb([px.red(), px.green(), px.blue()]));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `Boundary::Poly`'s phase is the *apothem* direction, not a vertex, whatever the
    /// comment on it says. Drawing a polygon half a step out of step with the boundary
    /// that is supposed to cut around it is invisible in a still and obvious in motion,
    /// so pin the convention here rather than rediscovering it.
    #[test]
    fn poly_phase_points_the_apothem_not_a_vertex() {
        let (n, r, phase) = (7.0f32, 200.0f32, 25.71f32);
        let b = Boundary::Poly(n, r, phase);
        let half_step = 180.0 / n;

        // At the phase itself the boundary is at its closest: the apothem.
        let apothem = r * (std::f32::consts::PI / n).cos();
        assert!((b.radius_at(phase.to_radians()) - apothem).abs() < 0.01);

        // Half a step round is a vertex, and there it reaches the full circumradius.
        assert!((b.radius_at((phase + half_step).to_radians()) - r).abs() < 0.01);
    }


    /// Nothing drawn may cross a layer cut.
    ///
    /// The two sides of a cut turn at different rates, so a line spanning one comes apart
    /// at the boundary as the plate rotates. It is invisible in a still, which is why it
    /// wants a test.
    #[test]
    fn nothing_drawn_crosses_a_layer_cut() {
        let cfg: Config = serde_json::from_str(include_str!("../layers.json")).unwrap();
        let layers = crate::gpu::ordered_layers(&cfg);
        let fig = Figure::default();
        let c = cfg.center;

        // Which layer owns a point, by the renderer's own first-hit-wins rule.
        let owner = |x: f32, y: f32| -> Option<usize> {
            let (dx, dy) = (x - c[0], y - c[1]);
            let r = (dx * dx + dy * dy).sqrt();
            let alpha = (-dx).atan2(-dy);
            layers.iter().position(|l| l.contains(r, alpha))
        };

        let check = |name: &str, pts: &[(f32, f32)]| {
            let mut seen: Option<usize> = None;
            for &(x, y) in pts {
                let Some(o) = owner(x, y) else { continue };
                match seen {
                    None => seen = Some(o),
                    Some(first) => assert_eq!(
                        first, o,
                        "{name} spans layers {first} and {o}; it will shear when they turn"
                    ),
                }
            }
        };

        let ring = |r: f32| -> Vec<(f32, f32)> {
            (0..720).map(|i| pt(c, r, TAU * i as f32 / 720.0)).collect()
        };
        let along = |a: (f32, f32), b: (f32, f32)| -> Vec<(f32, f32)> {
            (0..=200)
                .map(|i| {
                    let t = i as f32 / 200.0;
                    (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
                })
                .collect()
        };

        check("gold band inner edge", &ring(fig.band.0));
        check("gold band outer edge", &ring(fig.band.1));
        for &r in &fig.circles {
            check("circle", &ring(r));
        }

        let (ci, co, n) = fig.cells;
        for i in 0..n {
            let a = TAU * i as f32 / n as f32;
            check("cell divider", &along(pt(c, ci, a), pt(c, co, a)));
        }

        for &(r, phase) in &fig.heptagons {
            let v = poly_vertices(c, 7, r, phase);
            for i in 0..7 {
                check("heptagon edge", &along(v[i], v[(i + 1) % 7]));
            }
        }

        // Both edges of the ribbon, not just the line through the vertices: the inner one
        // reaches a good deal further in and is the one that will find a cut first.
        if let Some((hr, hphase, skip, width)) = fig.heptagram {
            for r in [hr, star_inside(hr, skip, 7, width)] {
                let v = star_points(c, 7, r, hphase, skip);
                for i in 0..7 {
                    check("heptagram ribbon", &along(v[i], v[(i + 1) % 7]));
                }
            }
        }

        // The lettering too, by its real outlines: ascenders and descenders are the parts
        // that reach across a cut, and a name at a constant radius inside a polygonal cut
        // straddles it wherever the cut dips to its apothem.
        let font = crate::text::Font::load();
        // The outline's own points, not its bounding box: the box is axis-aligned, so for
        // a glyph standing at 45 degrees it claims half again the ink that is there.
        let outline = |run: &[tiny_skia::Path]| -> Vec<(f32, f32)> {
            run.iter().flat_map(|p| p.points().iter().map(|q| (q.x, q.y))).collect()
        };

        let (ref names, nr, nphase, nsize) = fig.names;
        for (i, name) in names.iter().enumerate() {
            let a = crate::text::spoke(i, names.len(), nphase);
            let run = font.place(c, a, nr, nsize, name, crate::text::Along::Arc);
            check("name", &outline(&run));
        }

        let (outer_row, inner_row, size) = fig.cell_rows;
        for (i, &(top, bottom)) in fig.cell_text.iter().enumerate() {
            let a = crate::text::spoke(i, n, -180.0 / n as f32);
            let rows: Vec<(f32, &str)> = if bottom.is_empty() {
                vec![((outer_row + inner_row) * 0.5, top)]
            } else {
                vec![(outer_row, top), (inner_row, bottom)]
            };
            for (r, s) in rows {
                let run = font.place(c, a, r, size, s, crate::text::Along::Arc);
                check("cell text", &outline(&run));
            }
        }

        // The letter ring: its dividers, and the letters standing between them.
        let (lr_in, lr_out, lr_phase, lr_lift, lr_size) = fig.letter_ring;
        let lin = Boundary::Poly(7.0, lr_in, lr_phase);
        let lout = Boundary::Poly(7.0, lr_out, lr_phase);
        let cells = fig.letter_text.len();
        for (k, ch) in fig.letter_text.iter().enumerate() {
            let a = crate::text::spoke(k, cells, 0.0);
            check("letter-ring divider", &along(pt(c, lin.radius_at(a), a), pt(c, lout.radius_at(a), a)));

            let a = crate::text::spoke(k, cells, -180.0 / cells as f32);
            let base = lin.radius_at(a) + lr_lift;
            if *ch == "+" {
                let mid = (base + lout.radius_at(a)) * 0.5;
                let (x, y) = pt(c, mid, a);
                let p = cross_path(x, y, lr_size * 1.3, -a).unwrap();
                check("letter-ring cross", &outline(std::slice::from_ref(&p)));
            } else {
                let run = font.place(c, a, base, lr_size, ch, crate::text::Along::Arc);
                check("letter-ring letter", &outline(&run));
            }
        }

        // The core: its letters, the names inside the pentagram, and the words at the
        // centre. All of it belongs to one layer, and the cut above it is polygonal, so
        // the letters furthest out are the ones to watch.
        for &(ch, r, deg, size) in &fig.core_letters {
            let run = font.place(c, deg.to_radians(), r, size, ch, crate::text::Along::Arc);
            check("core letter", &outline(&run));
        }
        let (ref cn, cr, cphase, csize) = fig.core_names;
        for (k, name) in cn.iter().enumerate() {
            let a = crate::text::spoke(k, cn.len(), cphase);
            let run = font.place(c, a, cr, csize, name, crate::text::Along::Arc);
            check("core name", &outline(&run));
        }

        for &(r, off, size) in &fig.crosses {
            for k in 0..7 {
                let a = crate::text::spoke(k, 7, -off);
                let (x, y) = pt(c, r, a);
                let p = cross_path(x, y, size, -a).unwrap();
                check("loose cross", &outline(std::slice::from_ref(&p)));
            }
        }

        let (ref offsets, roof, biggest) = fig.lens_crosses;
        for v in 0..7 {
            for d in offsets {
                let a = crate::text::spoke(v, 7, -d);
                let (x, y, size) = lens_cross(&lout, roof, biggest, a);
                let p = cross_path(c[0] + x, c[1] + y, size, -a).unwrap();
                check("lens cross", &outline(std::slice::from_ref(&p)));
            }
        }

        // The four rings of spirit names, and the crosses that run off each end of them.
        // These sit between heptagons that the cuts also run between, so a row set on a
        // circle rather than along its own edge would cross one.
        for (row, names) in fig.spirit_rows.iter().zip(&fig.spirit_names) {
            let &(hr, lift, size, per_side, cross) = row;
            let line = Boundary::Poly(7.0, poly_outside(hr, 7, lift), 0.0);
            for (k, name) in names.iter().enumerate() {
                let a = crate::text::spoke(k, 7, 0.0);
                let run = font.place(c, a, line.radius_at(a), size, name, crate::text::Along::Arc);
                check("spirit name", &outline(&run));

                let half = font.width(name, size) * 0.5 / line.radius_at(a);
                for side in [-1.0f32, 1.0] {
                    for i in 0..per_side {
                        let along = MARGIN + cross * (1.02 * i as f32 + 0.5);
                        let b = a + side * (half + along / line.radius_at(a));
                        let (x, y) = pt(c, line.radius_at(b), b);
                        let p = cross_path(x, y, cross, -b).unwrap();
                        check("spirit cross", &outline(std::slice::from_ref(&p)));
                    }
                }
            }
        }

        let (pr, pphase, pwidth) = fig.pentagram;
        for r in [pr, star_inside(pr, 2, 5, pwidth)] {
            let pv = star_points(c, 5, r, pphase, 2);
            for i in 0..5 {
                check("pentagram ribbon", &along(pv[i], pv[(i + 1) % 5]));
            }
        }

    }

    /// ...and the drawing agrees with it, which is what actually keeps the two in step.
    #[test]
    fn drawn_vertices_sit_on_the_boundary() {
        let c = [0.0, 0.0];
        let (n, r, phase) = (7usize, 314.0f32, 25.71f32);
        let b = Boundary::Poly(n as f32, r, phase);
        for (x, y) in poly_vertices(c, n, r, phase) {
            let radius = (x * x + y * y).sqrt();
            let alpha = (-x).atan2(-y);
            // A drawn vertex must be exactly as far out as the boundary is there.
            assert!(
                (radius - b.radius_at(alpha)).abs() < 0.05,
                "vertex at r={radius} but boundary is at {}",
                b.radius_at(alpha)
            );
        }
    }
    /// Nothing drawn turns against something it is drawn on or tangent to.
    ///
    /// Which layer owns a ring is not obvious from `layers.json`: the cuts are polygons of
    /// two phases, and a ring falls wherever its *whole* outline lands. Three rules, all
    /// of them things that only show in motion:
    ///
    /// * A row of names and the heptagon it stands on are one ornament. In different
    ///   layers, the names walk along their own line.
    /// * The two nested heptagons and the row between them read as one piece, so they must
    ///   share a rate exactly, or the outer runs ahead of the inner.
    /// * Nothing may share a *speed* with the heptagram, even in the opposite direction:
    ///   at ±2 the inner heptagon read as joined to the star rather than to its own pair.
    ///
    /// Stated as relationships, not directions, since the absolute sense is easy to get
    /// wrong. Positive `turns` is counter-clockwise.
    #[test]
    fn nothing_turns_against_what_it_is_drawn_on() {
        let cfg: Config = serde_json::from_str(include_str!("../layers.json")).unwrap();
        let layers = crate::gpu::ordered_layers(&cfg);
        let fig = Figure::default();
        let c = cfg.center;
        let owner = |x: f32, y: f32| -> Option<usize> {
            let (dx, dy) = (x - c[0], y - c[1]);
            let r = (dx * dx + dy * dy).sqrt();
            let alpha = (-dx).atan2(-dy);
            layers.iter().position(|l| l.contains(r, alpha))
        };
        // Every point of a ring must land in one layer — `nothing_drawn_crosses_a_layer_cut`
        // already insists on that — so the turn rate is well defined.
        let turns_of = |what: &str, pts: &[(f32, f32)]| -> i32 {
            let mut seen: Vec<usize> = pts.iter().filter_map(|&(x, y)| owner(x, y)).collect();
            seen.sort();
            seen.dedup();
            assert_eq!(seen.len(), 1, "{what} is spread over {} layers", seen.len());
            layers[seen[0]].turns
        };
        let mut ring: Vec<(String, Vec<(f32, f32)>)> = Vec::new();
        for &(r, phase) in &fig.heptagons {
            let v = poly_vertices(c, 7, r, phase);
            let mut pts = Vec::new();
            for i in 0..7 {
                for j in 0..=20 {
                    let t = j as f32 / 20.0;
                    let (a, b) = (v[i], v[(i + 1) % 7]);
                    pts.push((a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t));
                }
            }
            ring.push((format!("heptagon {r}"), pts));
        }
        let font = crate::text::Font::load();
        for (row, names) in fig.spirit_rows.iter().zip(&fig.spirit_names) {
            let &(hr, lift, size, _, _) = row;
            let line = Boundary::Poly(7.0, poly_outside(hr, 7, lift), 0.0);
            let mut pts = Vec::new();
            for (k, n) in names.iter().enumerate() {
                let a = crate::text::spoke(k, 7, 0.0);
                for p in font.place(c, a, line.radius_at(a), size, n, crate::text::Along::Arc) {
                    pts.extend(p.points().iter().map(|q| (q.x, q.y)));
                }
            }
            ring.push((format!("spirit row on {hr}"), pts));
        }
        if let Some((hr, hphase, skip, width)) = fig.heptagram {
            let mut pts = Vec::new();
            for r in [hr, star_inside(hr, skip, 7, width)] {
                let v = star_points(c, 7, r, hphase, skip);
                for i in 0..7 {
                    for j in 0..=40 {
                        let t = j as f32 / 40.0;
                        let (a, b) = (v[i], v[(i + 1) % 7]);
                        pts.push((a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t));
                    }
                }
            }
            ring.push(("heptagram".to_string(), pts));
        }

        let find = |name: &str| -> i32 {
            let (what, pts) = ring.iter().find(|(n, _)| n == name).expect("ring not drawn");
            turns_of(what, pts)
        };

        // Printed as well as asserted: `cargo test -- --nocapture nothing_turns` shows
        // which layer each ring ended up in, which `layers.json` does not tell you.
        println!("\n  drawn element            layer            turns");
        for (n, pts) in &ring {
            let i = {
                let mut seen: Vec<usize> = pts.iter().filter_map(|&(x, y)| owner(x, y)).collect();
                seen.sort();
                seen.dedup();
                seen[0]
            };
            println!("  {:<22}  {:<15}  {:>3}", n, layers[i].name, layers[i].turns);
        }
        println!();

        // A row and the heptagon under it are one ornament, whichever layers they fall in.
        for (hr, _, _, _, _) in &fig.spirit_rows {
            let row = find(&format!("spirit row on {hr}"));
            let under = find(&format!("heptagon {hr}"));
            assert_eq!(
                row, under,
                "the row of names on the heptagon at {hr} turns {row} while the heptagon \
                 itself turns {under}; they are one ornament, and the names walk along \
                 their own line if the two come apart"
            );
        }

        let star = find("heptagram");
        let pair = find("heptagon 153.5");
        for name in ["heptagon 121.5", "spirit row on 121.5"] {
            let t = find(name);
            assert_eq!(
                t, pair,
                "{name} turns {t} where the heptagon at 153.5 turns {pair}; the two \
                 heptagons and the row between them read as one nested pair, so they have \
                 to turn at one rate or the outer runs ahead of the inner"
            );
        }
        assert_ne!(
            pair.abs(),
            star.abs(),
            "the pair turns {pair} and the heptagram {star}; matching speeds read as joined \
             even in opposite directions, and the pair is meant to read with itself"
        );
    }

}
