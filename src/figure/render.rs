use super::pen::{Mark, Style};
use super::spec::Rgb;

use image::{RgbImage, RgbaImage};
use tiny_skia::{Color, FillRule, LineCap, Paint, Pixmap, Stroke, Transform};

/// Room left around the outermost ink in every layer texture, in canvas units: the blood
/// wave's warp pulls ink in from up to this far away, and the bloom spills about as far.
const FRAME_PAD: f32 = 16.0;

/// One distinct colour per layer, bottom first, for telling which ring turns with which.
/// `layer_tint` in `shaders/common.wgsl` is a hand-kept copy.
pub const LAYER_TINT: [[u8; 3]; 7] = [
    [255, 71, 71],
    [255, 158, 41],
    [242, 242, 56],
    [82, 242, 97],
    [66, 230, 245],
    [122, 148, 255],
    [245, 107, 245],
];

/// One letter or symbol in the drawing, in canvas units.
#[derive(Debug, Clone, Copy)]
pub struct Glyph {
    pub pos: [f32; 2],
    /// Enough to cover the glyph, used both to light it and to bound the search.
    pub radius: f32,
    /// Index of the layer that carries it.
    pub layer: usize,
}

/// The square around the centre that every layer texture covers, in canvas units.
#[derive(Debug, Clone, Copy)]
pub struct Frame {
    pub origin: [f32; 2],
    pub side: f32,
}

pub struct LayerImage {
    pub name: String,
    pub turns: i32,
    /// Nearest and furthest any ink comes to the centre, padded for stroke width.
    pub extent: [f32; 2],
    /// The layer on a transparent ground, covering `Rendered::frame`. Premultiplied RGBA,
    /// sRGB-encoded, as tiny-skia leaves it.
    pub image: RgbaImage,
}

/// A figure drawn and ready to show: one image per layer plus what the viewer needs to
/// know about them.
pub struct Rendered {
    pub canvas: [f32; 2],
    pub center: [f32; 2],
    pub background: Rgb,
    pub frame: Frame,
    /// Texture pixels per canvas unit.
    pub scale: f32,
    pub loop_secs: f32,
    /// How often each ambient effect happens.
    pub effects: crate::effects::Settings,
    pub layers: Vec<LayerImage>,
    pub glyphs: Vec<Glyph>,
    pub warnings: Vec<String>,
}

impl Rendered {
    /// Radius of the smallest circle holding every layer's ink.
    pub fn disc(&self) -> f32 {
        self.layers.iter().map(|l| l.extent[1]).fold(0.0, f32::max)
    }
}

/// Nearest and furthest `marks` come to `c`.
///
/// Measured against each path flattened into short straight pieces, padded by half the
/// stroke. Control points would do for the nearest distance but sit well outside a curve,
/// which would inflate every texture by the same margin.
pub(super) fn extent(marks: &[Mark], c: [f32; 2]) -> [f32; 2] {
    let (mut lo, mut hi) = (f32::MAX, 0.0f32);
    for m in marks {
        let pad = match m.style {
            Style::Stroke(w) => w * 0.5,
            Style::Fill => 0.0,
        };
        for (a, b) in pieces(&m.path) {
            let (a, b) = ((a.0 - c[0], a.1 - c[1]), (b.0 - c[0], b.1 - c[1]));
            hi = hi.max(a.0.hypot(a.1) + pad).max(b.0.hypot(b.1) + pad);
            lo = lo.min((dist_to_segment(a, b) - pad).max(0.0));
        }
    }
    if lo > hi { [0.0, 0.0] } else { [lo, hi] }
}

/// `path` as straight pieces, each curve cut into eight.
fn pieces(path: &tiny_skia::Path) -> Vec<((f32, f32), (f32, f32))> {
    use tiny_skia::PathSegment as S;
    const STEPS: usize = 8;
    let mut out = Vec::new();
    let (mut start, mut at) = ((0.0, 0.0), (0.0, 0.0));
    let to = |out: &mut Vec<_>, from: &mut (f32, f32), p: (f32, f32)| {
        out.push((*from, p));
        *from = p;
    };
    for seg in path.segments() {
        match seg {
            S::MoveTo(p) => {
                start = (p.x, p.y);
                at = start;
            }
            S::LineTo(p) => to(&mut out, &mut at, (p.x, p.y)),
            S::QuadTo(p1, p) => {
                let p0 = at;
                for i in 1..=STEPS {
                    let t = i as f32 / STEPS as f32;
                    let u = 1.0 - t;
                    let f = |a: f32, b: f32, c: f32| u * u * a + 2.0 * u * t * b + t * t * c;
                    to(&mut out, &mut at, (f(p0.0, p1.x, p.x), f(p0.1, p1.y, p.y)));
                }
            }
            S::CubicTo(p1, p2, p) => {
                let p0 = at;
                for i in 1..=STEPS {
                    let t = i as f32 / STEPS as f32;
                    let u = 1.0 - t;
                    let f = |a: f32, b: f32, c: f32, d: f32| {
                        u * u * u * a + 3.0 * u * u * t * b + 3.0 * u * t * t * c + t * t * t * d
                    };
                    to(&mut out, &mut at, (f(p0.0, p1.x, p2.x, p.x), f(p0.1, p1.y, p2.y, p.y)));
                }
            }
            S::Close => to(&mut out, &mut at, start),
        }
    }
    out
}

/// Distance from the origin to the segment `a`-`b`.
fn dist_to_segment(a: (f32, f32), b: (f32, f32)) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = (dx * dx + dy * dy).max(1e-9);
    let t = (-(a.0 * dx + a.1 * dy) / len2).clamp(0.0, 1.0);
    let (x, y) = (a.0 + dx * t, a.1 + dy * t);
    (x * x + y * y).sqrt()
}

/// The square every layer texture covers: the disc of ink, padded.
pub(super) fn frame(center: [f32; 2], disc: f32) -> Frame {
    let half = disc + FRAME_PAD;
    Frame { origin: [center[0] - half, center[1] - half], side: 2.0 * half }
}

/// Every glyph mark, as the viewer's catalogue wants it.
pub(super) fn glyphs(marks: &[Mark], layer: usize) -> impl Iterator<Item = Glyph> + '_ {
    marks.iter().filter(|m| m.glyph).filter_map(move |m| {
        let b = m.path.bounds();
        let span = b.width().max(b.height());
        (span > 0.0).then(|| Glyph {
            pos: [(b.left() + b.right()) * 0.5, (b.top() + b.bottom()) * 0.5],
            radius: 0.5 * span + 2.0,
            layer,
        })
    })
}

/// Paint `marks` onto `pm` through `xf`, in their own colours or all in `tint`.
pub(super) fn paint(pm: &mut Pixmap, marks: &[Mark], xf: Transform, tint: Option<[u8; 3]>) {
    for m in marks {
        let [r, g, b] = tint.unwrap_or(m.color.0);
        let mut paint = Paint::default();
        paint.set_color(Color::from_rgba8(r, g, b, 255));
        paint.anti_alias = true;
        match m.style {
            Style::Fill => pm.fill_path(&m.path, &paint, FillRule::Winding, xf, None),
            Style::Stroke(width) => {
                let stroke = Stroke { width, line_cap: LineCap::Round, ..Stroke::default() };
                pm.stroke_path(&m.path, &paint, &stroke, xf, None)
            }
        }
    }
}

/// One layer on a transparent ground, covering `frame` at `scale` pixels per unit.
pub(super) fn layer_image(marks: &[Mark], frame: Frame, scale: f32) -> RgbaImage {
    let px = (frame.side * scale).ceil().max(1.0) as u32;
    let mut pm = Pixmap::new(px, px).expect("layer texture too large");
    let xf = Transform::from_scale(scale, scale).pre_translate(-frame.origin[0], -frame.origin[1]);
    paint(&mut pm, marks, xf, None);
    RgbaImage::from_raw(px, px, pm.take()).expect("pixmap is RGBA")
}

/// Every layer flattened onto the background, bottom first, at `scale` times the canvas.
pub(super) fn composite(
    layers: &[Vec<Mark>],
    canvas: [f32; 2],
    background: Rgb,
    scale: f32,
    tint: bool,
) -> RgbImage {
    let (w, h) = ((canvas[0] * scale).round() as u32, (canvas[1] * scale).round() as u32);
    let mut pm = Pixmap::new(w.max(1), h.max(1)).expect("canvas too large");
    let [r, g, b] = background.0;
    pm.fill(Color::from_rgba8(r, g, b, 255));
    let xf = Transform::from_scale(scale, scale);
    for (i, marks) in layers.iter().enumerate() {
        let tint = tint.then(|| LAYER_TINT[i % LAYER_TINT.len()]);
        paint(&mut pm, marks, xf, tint);
    }
    // Every pixel is opaque, so premultiplied and straight agree.
    let rgb = pm.pixels().iter().flat_map(|p| [p.red(), p.green(), p.blue()]).collect();
    RgbImage::from_raw(pm.width(), pm.height(), rgb).expect("pixmap size")
}
