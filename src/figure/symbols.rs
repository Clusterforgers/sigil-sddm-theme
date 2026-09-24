//! Shapes drawn from geometry rather than from the font, because they have to be sized and
//! turned to the place they sit in. To add one: a variant, and its shapes in `shapes`.
//!
//! Every symbol is a set of filled outlines, `size` tall and centred on the origin, with
//! North up. Anything stroked is turned into an outline first, so all of them draw the same
//! way and the viewer's lit-glyph effect can pick any of them up.

use super::pen::Pen;

use serde::de::IntoDeserializer;
use serde::Deserialize;
use std::f32::consts::PI;
use tiny_skia::{LineCap, Path, PathBuilder, Rect, Stroke, Transform};

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Symbol {
    /// A cross pattée: arms flaring from a narrow waist along concave curves.
    Cross,
    /// A cross potent (☩): straight arms, each ending in a crossbar. The plate's own cross.
    CrossPotent,
    /// A plain Latin cross with bare arms.
    LatinCross,
    /// An orb: a ringed circle with a cross potent standing on top.
    Orb,
    /// A tower: two posts joined by three rungs, crowned with a small cross.
    Tower,
}

impl Symbol {
    /// The symbol a text item like `"{cross}"` stands for, if it is one. Names are the same
    /// ones a `symbol:` field takes.
    pub fn from_token(item: &str) -> Option<Result<Symbol, String>> {
        let name = item.strip_prefix('{')?.strip_suffix('}')?;
        let de: serde::de::value::StrDeserializer<serde::de::value::Error> = name.into_deserializer();
        Some(Symbol::deserialize(de).map_err(|e| e.to_string()))
    }

    /// Draw it `size` tall, centred on `at` and turned `turn` radians clockwise.
    pub fn draw(self, pen: &mut Pen, at: (f32, f32), size: f32, turn: f32) {
        let xf = Transform::from_translate(at.0, at.1).pre_rotate(turn * 180.0 / PI);
        for shape in self.shapes(size) {
            pen.glyph(shape.transform(xf));
        }
    }

    /// Its outlines, `size` tall and centred on the origin.
    fn shapes(self, size: f32) -> Vec<Path> {
        match self {
            Symbol::Cross => pattee(size).into_iter().collect(),
            Symbol::CrossPotent => potent(size, 0.0).into_iter().collect(),
            Symbol::LatinCross => {
                let l = size * 0.5;
                let mut pb = PathBuilder::new();
                pb.move_to(0.0, -l);
                pb.line_to(0.0, l);
                pb.move_to(-l * 0.62, -l * 0.25);
                pb.line_to(l * 0.62, -l * 0.25);
                pb.finish().and_then(|p| stroked(&p, size * 0.085)).into_iter().collect()
            }
            Symbol::Orb => {
                // The orb fills the lower six tenths, the cross the rest, overlapping a
                // touch so the two read as one object.
                let r = size * 0.29;
                let cy = size * 0.5 - r - size * 0.02;
                let ring = |radius: f32, width: f32| {
                    PathBuilder::from_circle(0.0, cy, radius).and_then(|c| stroked(&c, width))
                };
                [ring(r, size * 0.075), ring(r * 0.5, size * 0.055), potent(size * 0.42, -size * 0.29)]
                    .into_iter()
                    .flatten()
                    .collect()
            }
            Symbol::Tower => {
                let (w, t) = (size * 0.3, size * 0.075);
                let post = |x: f32| rect(x - t * 0.5, -size * 0.3, t, size * 0.8);
                let rung = |y: f32| rect(-w, y - t * 0.5, 2.0 * w, t);
                [
                    post(-w * 0.62),
                    post(w * 0.62),
                    rung(-size * 0.22),
                    rung(size * 0.08),
                    rung(size * 0.38),
                    potent(size * 0.26, -size * 0.37),
                ]
                .into_iter()
                .flatten()
                .collect()
            }
        }
    }
}

/// A cross pattée `size` tall, centred on the origin.
fn pattee(size: f32) -> Option<Path> {
    let l = size * 0.5;
    // Half-widths at the waist and at the tips.
    let (w, f) = (size * 0.085, size * 0.33);
    // One arm, pointing up, from its right waist corner round to its left: (control, end)
    // pairs. The sides leave the waist almost straight and sweep out only near the tip —
    // the concave flare that makes a pattée — and the end dips slightly inward rather than
    // being cut square.
    let arm = [((w, -l * 0.62), (f, -l)), ((0.0, -l * 0.86), (-f, -l)), ((-w, -l * 0.62), (-w, -w))];
    // A quarter turn, which carries each arm's left waist corner onto the next arm's right
    // one, so the four join into one outline.
    let turn = |(x, y): (f32, f32), k: usize| (0..k).fold((x, y), |(x, y), _| (y, -x));
    let mut pb = PathBuilder::new();
    pb.move_to(w, -w);
    for k in 0..4 {
        for (c, p) in arm {
            let (c, p) = (turn(c, k), turn(p, k));
            pb.quad_to(c.0, c.1, p.0, p.1);
        }
    }
    pb.close();
    pb.finish()
}

/// A cross potent `size` tall, centred `dy` below the origin.
///
/// Built from overlapping rectangles, all wound the same way, so the fill is their union.
fn potent(size: f32, dy: f32) -> Option<Path> {
    let l = size * 0.5;
    // Arm thickness, and how far each end bar reaches either side of its arm: slim, with
    // short bars, or the four bars close up into a lattice at small sizes.
    let (t, bar) = (size * 0.12, size * 0.2);
    let mut pb = PathBuilder::new();
    let mut push = |x: f32, y: f32, w: f32, h: f32| {
        if let Some(r) = Rect::from_xywh(x, y + dy, w, h) {
            pb.push_rect(r);
        }
    };
    push(-t * 0.5, -l, t, size);
    push(-l, -t * 0.5, size, t);
    push(-bar, -l, 2.0 * bar, t);
    push(-bar, l - t, 2.0 * bar, t);
    push(-l, -bar, t, 2.0 * bar);
    push(l - t, -bar, t, 2.0 * bar);
    pb.finish()
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> Option<Path> {
    Rect::from_xywh(x, y, w, h).map(PathBuilder::from_rect)
}

/// The outline of `path` stroked `width` wide with round ends, as a shape to fill.
fn stroked(path: &Path, width: f32) -> Option<Path> {
    let stroke = Stroke { width, line_cap: LineCap::Round, ..Stroke::default() };
    // Resolution: how finely curves are flattened. The canvas is drawn at up to a few
    // times its own units, so allow for that.
    path.stroke(&stroke, 4.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_use_the_same_names_as_fields() {
        assert_eq!(Symbol::from_token("{cross}"), Some(Ok(Symbol::Cross)));
        assert_eq!(Symbol::from_token("{cross-potent}"), Some(Ok(Symbol::CrossPotent)));
        assert_eq!(Symbol::from_token("{orb}"), Some(Ok(Symbol::Orb)));
        assert_eq!(Symbol::from_token("cross"), None);
        assert!(Symbol::from_token("{crosss}").unwrap().unwrap_err().contains("latin-cross"));
    }

    /// Every symbol fits the size it was asked for, so placing one never spills into its
    /// neighbours.
    #[test]
    fn every_symbol_fits_its_size() {
        for s in [Symbol::Cross, Symbol::CrossPotent, Symbol::LatinCross, Symbol::Orb, Symbol::Tower] {
            let shapes = s.shapes(20.0);
            assert!(!shapes.is_empty(), "{s:?} draws nothing");
            for p in shapes {
                let b = p.bounds();
                let reach = b.left().abs().max(b.right().abs()).max(b.top().abs()).max(b.bottom().abs());
                assert!(reach <= 10.0 + 1.0, "{s:?} reaches {reach} from its centre at size 20");
            }
        }
    }
}
