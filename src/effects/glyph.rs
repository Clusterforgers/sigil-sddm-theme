use super::pool::Effect;
use super::smoothstep;
use crate::gpu::GpuGlyph;
use crate::figure::Glyph;

use rand::rngs::SmallRng;
use rand::RngExt;

/// A glyph that has been activated: lit, and drifting off the plate.
pub struct Lit {
    /// Where it sits in its layer, un-turned, and how far it reaches.
    pub pos: [f32; 2],
    pub half: [f32; 2],
    /// The layer carrying it. The copy has to travel with that layer as it turns, or it
    /// drifts away from the glyph it came out of.
    pub layer: usize,
    age: f32,
    /// How long it waits before it starts, so a group ripples.
    delay: f32,
    life: f32,
    /// How far out it drifts over its whole life, in canvas units.
    travel: f32,
    strength: f32,
}

impl Lit {
    /// How far through its own life it is, counting from the end of its wait.
    fn progress(&self) -> f32 {
        ((self.age - self.delay) / self.life.max(1e-3)).clamp(0.0, 1.0)
    }

    /// Drifts out under its own momentum and settles, rather than travelling at a constant
    /// rate — which would read as being dragged.
    fn travelled(&self) -> f32 {
        let u = self.progress();
        self.travel * (1.0 - (1.0 - u) * (1.0 - u))
    }

    /// How strongly the risen copy shows. It has to be gone by the time the glyph has
    /// finished travelling, or it just sits there as a second, brighter glyph.
    fn ghost(&self) -> f32 {
        let u = self.progress();
        self.strength * smoothstep(0.02, 0.22, u) * (1.0 - smoothstep(0.6, 1.0, u))
    }

    /// Where it has risen to, given the angle its layer has turned to and the disc centre.
    pub fn gpu(&self, angle: f32, center: [f32; 2]) -> GpuGlyph {
        // The glyph rides its layer, so the copy has to be carried round by that layer's
        // angle before the radial rise is added, or it would part company with the glyph
        // it came from.
        let (s, c) = angle.sin_cos();
        let (ax, ay) = (self.pos[0] - center[0], self.pos[1] - center[1]);
        // The inverse of the shader's artwork lookup, which rotates by +angle.
        let (mut px, mut py) = (ax * c + ay * s, ay * c - ax * s);
        let len = (px * px + py * py).sqrt().max(1e-3);
        let t = self.travelled();
        px += px * t / len;
        py += py * t / len;
        GpuGlyph {
            slot: [self.pos[0], self.pos[1], self.half[0], self.half[1]],
            risen: [center[0] + px, center[1] + py, self.ghost(), self.progress()],
            turn: [s, c, self.layer as f32, 0.0],
        }
    }
}

impl Effect for Lit {
    fn advance(&mut self, dt: f32) {
        self.age += dt;
    }

    fn done(&self) -> bool {
        self.age >= self.delay + self.life
    }
}

/// A cluster of glyphs to light: a seed picked from `catalogue` and its nearest
/// neighbours, each held back a little longer than the last, so the group ripples outward
/// from the seed rather than snapping on together. One glyph alone reads as a flicker; a
/// cluster reads as a passage being called.
pub fn cluster(
    rng: &mut SmallRng,
    catalogue: &[Glyph],
    most: usize,
) -> Vec<Lit> {
    if catalogue.is_empty() {
        return Vec::new();
    }
    let seed = catalogue[rng.random_range(0..catalogue.len())];
    let want = rng.random_range(3..9).min(most);

    // Nearest first. A few hundred glyphs, so sorting the lot costs nothing worth avoiding.
    let mut near: Vec<(f32, &Glyph)> = catalogue
        .iter()
        .map(|g| {
            let (dx, dy) = (g.pos[0] - seed.pos[0], g.pos[1] - seed.pos[1]);
            (dx * dx + dy * dy, g)
        })
        .collect();
    near.sort_by(|a, b| a.0.total_cmp(&b.0));

    near.iter()
        .take(want)
        .enumerate()
        .map(|(rank, &(_, g))| {
            // A little past the ink, so the glow is not cropped to the letter.
            let ext = g.radius + 3.0;
            Lit {
                pos: g.pos,
                half: [ext, ext],
                layer: g.layer,
                age: 0.0,
                // Each one waits on the one before it, with a little scatter so the ripple
                // does not look mechanical.
                delay: rank as f32 * rng.random_range(0.03..0.09),
                life: rng.random_range(1.3..2.4),
                travel: rng.random_range(10.0..22.0),
                strength: rng.random_range(0.75..1.25),
            }
        })
        .collect()
}
