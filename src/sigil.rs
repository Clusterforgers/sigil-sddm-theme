//! Draws the *Sigillum Dei Aemeth* instead of sampling a scan of one.
//!
//! Everything here lives in the same coordinate system as `layers.json`: source pixels
//! measured from `Config::center`, angles from North and counter-clockwise, exactly as
//! `geom.rs` defines them.
//!
//! One thing to watch. The radii in `layers.json` are **not** the figure's own — they are
//! cut lines, deliberately placed in the empty gaps *between* drawn rings, which is what
//! stops the animation tearing through artwork. The constants here are the drawn figure's,
//! measured off `images/1.png` by sweeping rays from the centre. The two sets are close
//! but must not be confused: the gold band is drawn at 393.5-406.5, while the layer that
//! owns it is cut at 389-410.

use crate::config::Config;

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
    /// The lettered cells between a circle and the big heptagon: (circle radius, cells
    /// per edge). Their dividers are spaced evenly *along each edge*, not evenly in
    /// angle — which is why the angles they land on look irregular when measured.
    pub edge_cells: (f32, usize),
    /// Heptagons, as (circumradius, phase in degrees).
    pub heptagons: Vec<(f32, f32)>,
    /// The heptagram: circumradius, phase, and how many vertices each chord steps
    /// over. `None` while its chords would cross a layer cut — see the note in `default`.
    pub heptagram: Option<(f32, f32, usize)>,
    /// The pentagram at the core: circumradius, phase.
    pub pentagram: (f32, f32),
    /// Width of a thin line, in source pixels.
    pub hairline: f32,
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
            edge_cells: (352.0, 0),
            // Only what has been checked against the source *and* sits wholly inside one
            // layer. Both conditions matter, and the second is the easy one to miss: a
            // drawn line that crosses a cut has its two halves turned at different rates,
            // so it visibly comes apart at the boundary as the plate rotates.
            //
            // Dropped for crossing a cut: the heptagram, whose chords run from r=314 down
            // to 196 and so cross the cut at 307 near both ends of every chord, and the
            // cell dividers that ran from the big heptagon out to the circle. Dropped for
            // not being there at all: heptagons at 139, 171, 186 and 217, which `fit` and
            // `gaps` both reported confidently and which are rows of crosses and lettering
            // rather than lines. Those two tools score by mean brightness along an outline,
            // so a radius that merely runs *along* a band of text scores as well as a rule
            // does. Comparing the drawing against the source pixel by pixel is what
            // settled it.
            //
            // Phases are in `Boundary::Poly`'s convention: the apothem points at the
            // phase, so 25.71 (= 180/7) is a vertex due North and 0 is a flat edge there.
            heptagons: vec![(153.5, 0.0), (121.5, 0.0)],
            heptagram: None,
            pentagram: (108.6, 36.0),
            hairline: 1.6,
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

/// The vertices of a regular `n`-gon whose phase is given the way `Boundary::Poly` means
/// it — and that is not the way it reads.
///
/// `Poly(n, r, phase)` puts the *apothem* at `phase`, i.e. a flat edge faces that way and
/// the vertices sit half a step to either side. (`geom.rs` describes it as a vertex, which
/// is worth not believing: `radius_at(phase)` returns `r * cos(PI/n)`, the apothem.) So a
/// heptagon that `fit` reports at phase 25.71 has a vertex pointing due North, and one it
/// reports at phase 0 has a flat edge there.
///
/// Keeping the constants in `Figure` in the same convention as `fit` and `layers.json`
/// means they can be compared by eye; the half-step correction lives here instead.
fn poly_vertices(c: [f32; 2], n: usize, r: f32, poly_phase_deg: f32) -> Vec<(f32, f32)> {
    let half_step = 180.0 / n as f32;
    vertices(c, n, r, poly_phase_deg + half_step)
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
/// An interim, and worth being honest about why it exists. The rings and rules in the
/// source are only as smooth as a 2000px raster can make them, and magnifying that cannot
/// invent the edge that was never captured — our sampling already matches a Lanczos
/// upscale. Drawing those same lines as paths does invent it, because they are geometry.
/// The lettering stays photographic until it too is drawn, so this buys the half of the
/// figure that is cheap to buy.
///
/// The drawn lines have to be at least as wide as the ones underneath or the ragged
/// original shows along their edges.
pub fn over(cfg: &Config, fig: &Figure, src: &RgbImage, scale: f32) -> RgbImage {
    render(cfg, fig, scale, Some(src))
}

/// The geometry is emitted at nominal size and scaled by the transform, so the constants
/// above stay readable against `layers.json` however large the texture gets.
fn render(cfg: &Config, fig: &Figure, scale: f32, base: Option<&RgbImage>) -> RgbImage {
    let w = (fig.canvas[0] as f32 * scale).round() as u32;
    let h = (fig.canvas[1] as f32 * scale).round() as u32;
    let mut pm = Pixmap::new(w, h).expect("canvas too large");

    match base {
        // Resample the photograph up to the canvas first, so the drawn lines land on top
        // of it. Nothing is gained in the lettering by doing this — it is already as
        // detailed as it will ever be — but the lines drawn over it are not limited to
        // the raster grid.
        Some(src) => {
            let up = image::imageops::resize(src, w, h, image::imageops::FilterType::Lanczos3);
            for (i, px) in up.pixels().enumerate() {
                let o = i * 4;
                pm.data_mut()[o] = px.0[0];
                pm.data_mut()[o + 1] = px.0[1];
                pm.data_mut()[o + 2] = px.0[2];
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
    if let Some(p) = PathBuilder::from_circle(c[0], c[1], (bi + bo) * 0.5) {
        let band = Stroke { width: bo - bi, ..stroke.clone() };
        pm.stroke_path(&p, &paint, &band, xf, None);
    }

    for &r in &fig.circles {
        if let Some(p) = PathBuilder::from_circle(c[0], c[1], r) {
            pm.stroke_path(&p, &paint, &stroke, xf, None);
        }
    }

    // Radial dividers of the lettered ring.
    let (ci, co, n) = fig.cells;
    let mut pb = PathBuilder::new();
    for i in 0..n {
        let a = TAU * i as f32 / n as f32;
        let (x0, y0) = pt(c, ci, a);
        let (x1, y1) = pt(c, co, a);
        pb.move_to(x0, y0);
        pb.line_to(x1, y1);
    }
    if let Some(p) = pb.finish() {
        pm.stroke_path(&p, &paint, &stroke, xf, None);
    }

    // The inner ring of cells. Its dividers run from points spaced evenly along each
    // heptagon edge out to the circle, so they fan slightly rather than sitting at equal
    // angles — measuring the source shows exactly that irregularity.
    let (ec_r, per_edge) = fig.edge_cells;
    if per_edge > 0 && !fig.heptagons.is_empty() {
        let (hr0, hp0) = fig.heptagons[0];
        let _ = (hr0, hp0);
        let hv = poly_vertices(c, 7, hr0, hp0);
        let mut pb = PathBuilder::new();
        for i in 0..7 {
            let a = hv[i];
            let b = hv[(i + 1) % 7];
            for k in 0..=per_edge {
                let t = k as f32 / per_edge as f32;
                let (px, py) = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
                // Push the point straight out from the centre to meet the circle.
                let (dx, dy) = (px - c[0], py - c[1]);
                let len = (dx * dx + dy * dy).sqrt().max(1e-3);
                pb.move_to(px, py);
                pb.line_to(c[0] + dx * ec_r / len, c[1] + dy * ec_r / len);
            }
        }
        if let Some(p) = pb.finish() {
            pm.stroke_path(&p, &paint, &stroke, xf, None);
        }
    }

    for &(r, phase) in &fig.heptagons {
        if let Some(p) = closed_path(&poly_vertices(c, 7, r, phase)) {
            pm.stroke_path(&p, &paint, &stroke, xf, None);
        }
    }

    // The heptagram: one continuous path that steps `skip` vertices at a time, which for a
    // 7-gon closes only after visiting all seven — that is what makes it a single star
    // rather than a ring of triangles.
    if let Some((hr, hphase, skip)) = fig.heptagram {
        let v = poly_vertices(c, 7, hr, hphase);
        let mut order = Vec::with_capacity(7);
        let mut i = 0usize;
        for _ in 0..7 {
            order.push(v[i]);
            i = (i + skip) % 7;
        }
        if let Some(p) = closed_path(&order) {
            pm.stroke_path(&p, &paint, &stroke, xf, None);
        }
    }

    // The pentagram, the same way: a 5-gon stepped two at a time.
    let (pr, pphase) = fig.pentagram;
    let pv = poly_vertices(c, 5, pr, pphase);
    let star: Vec<_> = (0..5).map(|k| pv[(k * 2) % 5]).collect();
    if let Some(p) = closed_path(&star) {
        pm.stroke_path(&p, &paint, &stroke, xf, None);
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
    use crate::geom::Boundary;

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
}
