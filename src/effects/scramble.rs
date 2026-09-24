use super::pool::Effect;
use super::smoothstep;
use crate::figure::Glyph;
use crate::gpu::GpuSwap;

use rand::rngs::SmallRng;
use rand::RngExt;

/// Letters closer to the centre than this are left alone: the words round the cross there
/// are set square to the page, not upright to the centre, so turning another letter into
/// their place would stand it at the wrong angle.
const INNERMOST: f32 = 40.0;

/// A letter flickering through other letters of the figure, then settling back.
///
/// The whole flicker is decided when it starts — which letters it borrows and when each
/// takes over — so it ages like any other effect and needs no randomness while it runs.
pub struct Swap {
    target: Glyph,
    /// (seconds from the start, letter shown from then on).
    flicks: Vec<(f32, Glyph)>,
    center: [f32; 2],
    age: f32,
    delay: f32,
    life: f32,
}

/// The clockwise angle, in radians, of `p` round `center`. Each letter stands upright to
/// the centre, so this is also how far it is turned from upright.
fn bearing(p: [f32; 2], center: [f32; 2]) -> f32 {
    (p[0] - center[0]).atan2(center[1] - p[1])
}

fn distance(p: [f32; 2], center: [f32; 2]) -> f32 {
    (p[0] - center[0]).hypot(p[1] - center[1])
}

impl Swap {
    fn new(rng: &mut SmallRng, target: Glyph, catalogue: &[Glyph], center: [f32; 2], delay: f32, life: f32) -> Option<Self> {
        // Letters of about the same size, so each fills the slot rather than rattling in it
        // or bursting out of it.
        let donors: Vec<Glyph> = catalogue
            .iter()
            .filter(|g| distance(g.pos, center) > INNERMOST)
            .filter(|g| (g.radius - target.radius).abs() <= target.radius * 0.25)
            .filter(|g| g.pos != target.pos)
            .copied()
            .collect();
        if donors.is_empty() {
            return None;
        }
        // Flicking fast, a new letter every few frames, over the first four fifths; the
        // last fifth is the real letter coming back.
        let mut flicks = Vec::new();
        let mut t = 0.0;
        while t < life * 0.8 {
            flicks.push((t, donors[rng.random_range(0..donors.len())]));
            t += rng.random_range(0.06..0.11);
        }
        Some(Swap { target, flicks, center, age: 0.0, delay, life })
    }

    /// How strongly the borrowed letter replaces the real one.
    fn strength(&self) -> f32 {
        let u = (self.age - self.delay) / self.life;
        if !(0.0..1.0).contains(&u) {
            return 0.0;
        }
        smoothstep(0.0, 0.03, u) * (1.0 - smoothstep(0.72, 0.8, u))
    }

    pub fn gpu(&self) -> GpuSwap {
        let t = self.age - self.delay;
        let source = self.flicks.iter().rev().find(|(at, _)| *at <= t).map_or(self.target, |f| f.1);
        let c = self.center;
        let turn = bearing(source.pos, c) - bearing(self.target.pos, c);
        let t = self.target;
        GpuSwap {
            slot: [t.pos[0], t.pos[1], t.radius, t.layer as f32],
            source: [source.pos[0], source.pos[1], source.layer as f32, self.strength()],
            map: [turn.cos(), turn.sin(), source.radius / t.radius, 0.0],
        }
    }

    /// Where the letter sits and how far it reaches, for the shader's bounding box.
    pub fn extent(&self) -> ([f32; 2], f32) {
        (self.target.pos, self.target.radius)
    }
}

impl Effect for Swap {
    fn advance(&mut self, dt: f32) {
        self.age += dt;
    }

    fn done(&self) -> bool {
        self.age >= self.delay + self.life
    }
}

/// A run of up to `letters` neighbouring letters that start scrambling one after another,
/// so it spreads along the word like a ripple.
pub fn scramble(rng: &mut SmallRng, catalogue: &[Glyph], center: [f32; 2], letters: u32, life: f32) -> Vec<Swap> {
    let candidates: Vec<Glyph> = catalogue.iter().filter(|g| distance(g.pos, center) > INNERMOST).copied().collect();
    if candidates.is_empty() {
        return Vec::new();
    }
    let seed = candidates[rng.random_range(0..candidates.len())];
    let mut near = candidates.clone();
    near.sort_by(|a, b| {
        let d = |g: &Glyph| (g.pos[0] - seed.pos[0]).hypot(g.pos[1] - seed.pos[1]);
        d(a).total_cmp(&d(b))
    });
    let want = rng.random_range(1..=letters.max(1)) as usize;
    near.into_iter()
        .take(want)
        .enumerate()
        .filter_map(|(rank, g)| Swap::new(rng, g, catalogue, center, rank as f32 * 0.07, life))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn glyph(x: f32, y: f32) -> Glyph {
        Glyph { pos: [x, y], radius: 6.0, layer: 0 }
    }

    /// Flickers through other letters, then gives the real one back.
    #[test]
    fn it_flickers_and_settles_back() {
        let mut rng = rand::make_rng();
        let center = [0.0, 0.0];
        let catalogue: Vec<Glyph> = (0..20).map(|i| glyph(100.0 + i as f32 * 10.0, 0.0)).collect();
        let mut s = Swap::new(&mut rng, catalogue[0], &catalogue, center, 0.0, 1.0).unwrap();
        s.advance(0.3);
        let during = s.gpu();
        assert_eq!(during.source[3], 1.0);
        assert_ne!([during.source[0], during.source[1]], catalogue[0].pos, "never borrows itself");
        s.advance(0.6);
        assert_eq!(s.gpu().source[3], 0.0, "the real letter is back");
    }

    /// Turning maps a letter's slot onto the donor's: a point just outward of one lands
    /// just outward of the other, whatever their angles round the centre.
    #[test]
    fn the_map_carries_outward_to_outward() {
        let mut rng = rand::make_rng();
        let (north, east) = (glyph(0.0, -100.0), glyph(100.0, 0.0));
        let s = Swap::new(&mut rng, north, &[north, east], [0.0, 0.0], 0.0, 1.0).unwrap();
        let m = s.gpu().map;
        // "Outward" at the north slot is (0, -1); at the east donor it is (1, 0).
        let (x, y) = (0.0f32, -1.0f32);
        let mapped = (x * m[0] - y * m[1], x * m[1] + y * m[0]);
        assert!((mapped.0 - 1.0).abs() < 1e-5 && mapped.1.abs() < 1e-5, "{mapped:?}");
    }
}
