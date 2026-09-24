use std::f32::consts::{PI, TAU};
use tiny_skia::{Path, PathBuilder, Transform};

/// The face the figure is lettered in.
///
/// DejaVu Serif, because it was available offline and is unambiguously redistributable —
/// not because it is right. A Garamond would suit a 16th-century plate far better, and
/// swapping it means changing this one constant and the file beside it.
const FACE: &[u8] = include_bytes!("../fonts/DejaVuSerif.ttf");

/// Which way a run of letters reads.
#[derive(Clone, Copy, PartialEq)]
pub enum Along {
    /// Around an arc, upright to someone standing at the centre — the seven names.
    Arc,
    /// Outward along a radius, as the inner names are written.
    Radial,
}

/// A face, parsed once.
pub struct Font<'a> {
    face: ttf_parser::Face<'a>,
    /// Font units per em, so sizes can be given in source pixels.
    upem: f32,
}

impl Font<'static> {
    pub fn load() -> Self {
        let face = ttf_parser::Face::parse(FACE, 0).expect("the vendored face does not parse");
        let upem = face.units_per_em() as f32;
        Font { face, upem }
    }
}

/// Collects a glyph's outline into a `tiny-skia` path.
///
/// TrueType gives quadratics and OpenType cubics; `PathBuilder` takes both, so this is a
/// straight transcription rather than a conversion.
struct Outline {
    pb: PathBuilder,
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

impl Font<'_> {
    /// Width of `s` if it were set at `size` source pixels, before any curving.
    pub fn width(&self, s: &str, size: f32) -> f32 {
        let k = size / self.upem;
        s.chars()
            .filter_map(|ch| self.face.glyph_index(ch))
            .filter_map(|g| self.face.glyph_hor_advance(g))
            .map(|a| a as f32 * k)
            .sum()
    }

    /// One glyph's outline, in font units with the origin on the baseline.
    fn glyph(&self, ch: char) -> Option<(Path, f32)> {
        let id = self.face.glyph_index(ch)?;
        let adv = self.face.glyph_hor_advance(id)? as f32;
        let mut o = Outline { pb: PathBuilder::new(), open: false };
        self.face.outline_glyph(id, &mut o)?;
        if o.open {
            o.pb.close();
        }
        Some((o.pb.finish()?, adv))
    }

    /// Lay `s` out and return the filled paths, ready to draw.
    ///
    /// `centre` is the angle the run is centred on and `radius` where its baseline sits.
    /// Each glyph is placed on the arc and turned to its own tangent, so a long name bends
    /// with the ring instead of chording across it.
    pub fn place(
        &self,
        c: [f32; 2],
        centre_angle: f32,
        radius: f32,
        size: f32,
        s: &str,
        along: Along,
    ) -> Vec<Path> {
        let k = size / self.upem;
        let total = self.width(s, size);
        let mut out = Vec::new();

        // Walk from one end of the run to the other, in the units the layout uses: angle
        // for a curved run, distance for a straight one.
        let mut cursor = -total * 0.5;

        for ch in s.chars() {
            let Some((path, adv)) = self.glyph(ch) else {
                continue;
            };
            let adv = adv * k;
            // Place by the glyph's middle so rounding at the ends does not drift.
            let at = cursor + adv * 0.5;

            let xf = match along {
                Along::Arc => {
                    // Along the arc, the run's own x becomes an angle. Subtracted, not
                    // added: alpha runs counter-clockwise, which is leftward across the
                    // top of the disc, and letters added that way come out reversed.
                    let a = centre_angle - at / radius.max(1.0);
                    let (px, py) = (c[0] - radius * a.sin(), c[1] - radius * a.cos());
                    // Upright to a reader outside the disc, which is how the plate is
                    // lettered: the baseline follows the tangent and the letters stand
                    // away from the centre, so a name at the foot reads upside down.
                    // Font y points up, which is -y on the canvas, hence the flipped
                    // scale rather than a second rotation.
                    Transform::from_translate(px, py)
                        .pre_rotate(-a * 180.0 / PI)
                        .pre_scale(k, -k)
                        .pre_translate(-adv * 0.5 / k, 0.0)
                }
                Along::Radial => {
                    // Reading outward: the run lies along the radius at `centre_angle`.
                    let r = radius + at;
                    let (px, py) = (c[0] - r * centre_angle.sin(), c[1] - r * centre_angle.cos());
                    Transform::from_translate(px, py)
                        .pre_rotate((-centre_angle * 180.0 / PI) + 90.0)
                        .pre_scale(k, -k)
                        .pre_translate(-adv * 0.5 / k, 0.0)
                }
            };

            if let Some(p) = path.transform(xf) {
                out.push(p);
            }
            cursor += adv;
        }
        out
    }
}

impl Font<'_> {
    /// Lay `s` out upright and centred on `(x, y)`, with no curving.
    ///
    /// The four words around the cross at the very centre are set square to the page, not
    /// to a radius, so `place` cannot do them.
    pub fn place_at(&self, x: f32, y: f32, size: f32, s: &str) -> Vec<Path> {
        let k = size / self.upem;
        let mut cursor = -self.width(s, size) * 0.5;
        let mut out = Vec::new();
        for ch in s.chars() {
            let Some((path, adv)) = self.glyph(ch) else { continue };
            let adv = adv * k;
            let xf = Transform::from_translate(x + cursor, y + size * 0.36).pre_scale(k, -k);
            if let Some(p) = path.transform(xf) {
                out.push(p);
            }
            cursor += adv;
        }
        out
    }
}

/// The angle of the `i`th of `n` evenly spaced positions, clockwise from `phase`.
///
/// Clockwise because that is the way the plate is lettered and read, while alpha itself
/// runs the other way.
pub fn spoke(i: usize, n: usize, phase_deg: f32) -> f32 {
    phase_deg.to_radians() - TAU * i as f32 / n as f32
}
