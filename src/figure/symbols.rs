//! Shapes drawn from geometry rather than from the font, because they have to be sized and
//! turned to the place they sit in. To add one: a variant here, and its path in `path`.

use super::pen::Pen;

use serde::de::IntoDeserializer;
use serde::Deserialize;
use std::f32::consts::PI;
use tiny_skia::{Path, PathBuilder, Transform};

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Symbol {
    /// A cross pattée: four arms flaring from a narrow waist to a wide tip. Filled.
    Cross,
    /// A plain Latin cross with bare arms. Stroked.
    LatinCross,
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
        let path = self.path(size).and_then(|p| p.transform(xf));
        match self {
            Symbol::Cross => pen.glyph(path),
            Symbol::LatinCross => pen.line(path, Some(size * 0.085)),
        }
    }

    /// The outline, `size` tall and centred on the origin.
    fn path(self, size: f32) -> Option<Path> {
        let l = size * 0.5;
        let mut pb = PathBuilder::new();
        match self {
            Symbol::Cross => {
                let (w, f) = (size * 0.11, size * 0.35);
                // One continuous outline: waist corner, out to the two corners of a tip,
                // back to the next waist corner, four times round.
                let pts = [
                    (w, w), (f, l), (-f, l),
                    (-w, w), (-l, f), (-l, -f),
                    (-w, -w), (-f, -l), (f, -l),
                    (w, -w), (l, -f), (l, f),
                ];
                pb.move_to(pts[0].0, pts[0].1);
                for p in &pts[1..] {
                    pb.line_to(p.0, p.1);
                }
                pb.close();
            }
            Symbol::LatinCross => {
                pb.move_to(0.0, -l);
                pb.line_to(0.0, l);
                pb.move_to(-l * 0.62, -l * 0.25);
                pb.line_to(l * 0.62, -l * 0.25);
            }
        }
        pb.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_use_the_same_names_as_fields() {
        assert_eq!(Symbol::from_token("{cross}"), Some(Ok(Symbol::Cross)));
        assert_eq!(Symbol::from_token("{latin-cross}"), Some(Ok(Symbol::LatinCross)));
        assert_eq!(Symbol::from_token("cross"), None);
        assert!(Symbol::from_token("{crosss}").unwrap().unwrap_err().contains("latin-cross"));
    }
}
