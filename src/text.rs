use std::borrow::Cow;
use std::f32::consts::PI;
use std::path::Path as FsPath;
use tiny_skia::{Path, PathBuilder, Transform};

/// The face used when a figure does not name one.
const EMBEDDED: &[u8] = include_bytes!("../fonts/AngelWish.ttf");

/// Which way a run of letters reads.
#[derive(Clone, Copy, PartialEq, Debug, Default, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Orient {
    /// Around an arc, upright to someone standing at the centre.
    #[default]
    Arc,
    /// Outward along a radius.
    Radial,
}

/// A font face. Owns its bytes; the face tables are parsed per text run, which costs
/// microseconds and avoids a self-referential struct.
pub struct Font {
    data: Cow<'static, [u8]>,
}

impl Font {
    pub fn embedded() -> Self {
        Font { data: Cow::Borrowed(EMBEDDED) }
    }

    pub fn from_file(path: &FsPath) -> Result<Self, String> {
        let data = std::fs::read(path).map_err(|e| format!("cannot read font {}: {e}", path.display()))?;
        ttf_parser::Face::parse(&data, 0)
            .map_err(|e| format!("{} is not a usable font: {e}", path.display()))?;
        Ok(Font { data: Cow::Owned(data) })
    }

    fn face(&self) -> ttf_parser::Face<'_> {
        // Checked when the font was loaded.
        ttf_parser::Face::parse(&self.data, 0).expect("font was validated on load")
    }

    pub fn has_char(&self, ch: char) -> bool {
        ch.is_whitespace() || self.face().glyph_index(ch).is_some()
    }

    /// Width of `s` if it were set at `size` canvas units, before any curving.
    pub fn width(&self, s: &str, size: f32) -> f32 {
        let face = self.face();
        let k = size / face.units_per_em() as f32;
        s.chars()
            .filter_map(|ch| face.glyph_index(ch))
            .filter_map(|g| face.glyph_hor_advance(g))
            .map(|a| a as f32 * k)
            .sum()
    }

    /// Lay `s` out around the centre `c`, centred on internal angle `alpha` with its
    /// baseline at `radius`, upright to a reader at the centre.
    pub fn place(&self, c: [f32; 2], alpha: f32, radius: f32, size: f32, s: &str, orient: Orient) -> Vec<Path> {
        let face = self.face();
        let k = size / face.units_per_em() as f32;
        let mut out = Vec::new();

        // Walk from one end of the run to the other, in the units the layout uses: arc
        // length for a curved run, distance for a straight one.
        let mut cursor = -self.width(s, size) * 0.5;

        for ch in s.chars() {
            let Some((path, adv)) = glyph(&face, ch) else { continue };
            let adv = adv * k;
            // Place by the glyph's middle so rounding at the ends does not drift.
            let at = cursor + adv * 0.5;

            let xf = match orient {
                Orient::Arc => {
                    let a = alpha - at / radius.max(1.0);
                    let (px, py) = (c[0] - radius * a.sin(), c[1] - radius * a.cos());
                    // Font y points up, which is -y on the canvas, hence the flipped scale.
                    Transform::from_translate(px, py).pre_rotate(-a * 180.0 / PI)
                }
                Orient::Radial => {
                    let r = radius + at;
                    let (px, py) = (c[0] - r * alpha.sin(), c[1] - r * alpha.cos());
                    Transform::from_translate(px, py).pre_rotate(-alpha * 180.0 / PI + 90.0)
                }
            };
            let xf = xf.pre_scale(k, -k).pre_translate(-adv * 0.5 / k, 0.0);

            out.extend(path.transform(xf));
            cursor += adv;
        }
        out
    }

    /// Lay `s` out upright and centred on `(x, y)`, with no curving.
    pub fn place_at(&self, x: f32, y: f32, size: f32, s: &str) -> Vec<Path> {
        let face = self.face();
        let k = size / face.units_per_em() as f32;
        let mut cursor = -self.width(s, size) * 0.5;
        let mut out = Vec::new();
        for ch in s.chars() {
            let Some((path, adv)) = glyph(&face, ch) else { continue };
            let xf = Transform::from_translate(x + cursor, y + size * 0.36).pre_scale(k, -k);
            out.extend(path.transform(xf));
            cursor += adv * k;
        }
        out
    }
}

/// One glyph's outline, in font units with the origin on the baseline, and its advance.
fn glyph(face: &ttf_parser::Face, ch: char) -> Option<(Path, f32)> {
    let id = face.glyph_index(ch)?;
    let adv = face.glyph_hor_advance(id)? as f32;
    let mut o = Outline { pb: PathBuilder::new(), open: false };
    face.outline_glyph(id, &mut o)?;
    if o.open {
        o.pb.close();
    }
    Some((o.pb.finish()?, adv))
}

/// A builder for a single glyph's outline, which is a series of subpaths.
struct Outline {
    pb: PathBuilder,
    /// Whether the current subpath is open.
    open: bool,
}

impl ttf_parser::OutlineBuilder for Outline {
    fn move_to(&mut self, x: f32, y: f32) {
        if self.open {
            self.pb.close();
        }
        self.pb.move_to(x, y);
        self.open = true;
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.pb.line_to(x, y);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.pb.quad_to(x1, y1, x, y);
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.pb.cubic_to(x1, y1, x2, y2, x, y);
    }
    fn close(&mut self) {
        self.pb.close();
        self.open = false;
    }
}
