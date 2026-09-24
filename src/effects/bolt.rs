use super::pool::Effect;
use crate::gpu::GpuLimb;

use rand::rngs::SmallRng;
use rand::RngExt;
use std::f32::consts::{PI, TAU};
use std::ops::Range;

/// Bolts an implosion throws out of the centre when it lands.
pub const BURST_BOLTS: u32 = 9;

/// Bolts that can be in the air together. Has to clear a whole burst, or the fan would
/// evict its own first arms before the last ones were out.
pub const MAX_BOLTS: usize = 14;

/// One straight limb of a bolt, in canvas units.
pub struct Limb {
    pub a: [f32; 2],
    pub b: [f32; 2],
}

/// A bolt of lightning arcing across the diagram.
pub struct Bolt {
    /// The jagged path, plus any fork, flattened into one list.
    pub limbs: Vec<Limb>,
    age: f32,
    life: f32,
    strength: f32,
    /// Half-width of the hot core, in canvas units.
    width: f32,
    /// Flicker phase, so two bolts alight at once do not pulse in step.
    phase: f32,
}

impl Bolt {
    /// Lightning is a flash that decays fast, with a flicker on top so it reads as
    /// electricity rather than as a line quietly fading out.
    fn amount(&self) -> f32 {
        let u = (self.age / self.life.max(1e-3)).min(1.0);
        let decay = (1.0 - u) * (1.0 - u);
        self.strength * decay * (0.68 + 0.32 * (self.age * 57.0 + self.phase).sin())
    }

    /// Its limbs as the shader wants them.
    pub fn gpu(&self) -> impl Iterator<Item = GpuLimb> + '_ {
        let style = [self.amount(), self.width, 0.0, 0.0];
        self.limbs
            .iter()
            .map(move |l| GpuLimb { seg: [l.a[0], l.a[1], l.b[0], l.b[1]], style })
    }
}

impl Effect for Bolt {
    fn advance(&mut self, dt: f32) {
        self.age += dt;
    }

    fn done(&self) -> bool {
        self.age >= self.life
    }
}

/// How a bolt is drawn: kinks along the main run, the chance of a fork and what it looks
/// like, and how it flashes.
struct Shape {
    steps: usize,
    /// Sideways kink, as a fraction of the run's length.
    spread: f32,
    fork_chance: f32,
    /// Fork length as a fraction of the main reach.
    fork_len: Range<f32>,
    fork_spread: f32,
    /// How far a fork may turn from the run's direction; `None` for anywhere.
    fork_turn: Option<f32>,
    life: Range<f32>,
    strength: Range<f32>,
    width: Range<f32>,
}

/// Where lightning happens: the disc it must stay inside.
#[derive(Clone, Copy)]
pub struct Sky {
    pub center: [f32; 2],
    pub disc: f32,
}

impl Sky {
    /// Pull a point back inside the disc, so a bolt and its halo never cross the rim.
    fn inside(&self, p: [f32; 2], frac: f32) -> [f32; 2] {
        let (dx, dy) = (p[0] - self.center[0], p[1] - self.center[1]);
        let r = (dx * dx + dy * dy).sqrt();
        let max = self.disc * frac;
        if r <= max || r < 1e-3 {
            p
        } else {
            [self.center[0] + dx * max / r, self.center[1] + dy * max / r]
        }
    }

    /// Strike from `from` to somewhere new. Returns the bolt and where it landed.
    pub fn strike(&self, rng: &mut SmallRng, from: [f32; 2]) -> (Bolt, [f32; 2]) {
        let d = self.disc;
        let long = rng.random_range(0.0..1.0) < 0.22;
        let reach = if long { d * rng.random_range(0.75..1.55) } else { d * rng.random_range(0.22..0.62) };
        let dir: f32 = rng.random_range(0.0..TAU);
        let shape = Shape {
            steps: if long { 9 } else { 6 },
            spread: 0.085,
            // A fork costs three limbs and does more for the look than anything else here.
            fork_chance: 0.45,
            fork_len: 0.20..0.45,
            fork_spread: 0.11,
            fork_turn: None,
            life: 0.18..0.38,
            strength: if long { 1.10..1.60 } else { 0.75..1.15 },
            width: 2.6..4.0,
        };
        self.build(rng, from, dir, reach, 0.90, &shape, 1.0)
    }

    /// One arm of the fan an implosion throws out of the centre when it lands.
    pub fn burst_arm(&self, rng: &mut SmallRng, angle: f32, power: f32) -> (Bolt, [f32; 2]) {
        let reach = self.disc * rng.random_range(0.72..0.95);
        // Taut and nearly straight: this is energy thrown outward, not something picking
        // its way across the diagram. Shorter-lived than a wandering strike, which also
        // keeps the number alight at once — and so the cost — down.
        let shape = Shape {
            steps: 6,
            spread: 0.055,
            fork_chance: 0.18,
            fork_len: 0.18..0.38,
            fork_spread: 0.09,
            fork_turn: Some(1.1),
            life: 0.12..0.24,
            strength: 1.2..1.8,
            width: 2.8..4.2,
        };
        self.build(rng, self.center, angle, reach, 0.92, &shape, power)
    }

    #[allow(clippy::too_many_arguments)]
    fn build(
        &self,
        rng: &mut SmallRng,
        from: [f32; 2],
        dir: f32,
        reach: f32,
        bound: f32,
        s: &Shape,
        power: f32,
    ) -> (Bolt, [f32; 2]) {
        let to = self.inside([from[0] + reach * dir.cos(), from[1] + reach * dir.sin()], bound);
        let mut limbs = jag(rng, from, to, s.steps, s.spread);

        if limbs.len() > 2 && rng.random_range(0.0..1.0) < s.fork_chance {
            let root = limbs[rng.random_range(1..limbs.len() - 1)].a;
            let ang = match s.fork_turn {
                Some(t) => dir + rng.random_range(-t..t),
                None => rng.random_range(0.0..TAU),
            };
            let flen = reach * rng.random_range(s.fork_len.clone());
            let tip = self.inside([root[0] + flen * ang.cos(), root[1] + flen * ang.sin()], bound);
            limbs.extend(jag(rng, root, tip, 3, s.fork_spread));
        }

        let bolt = Bolt {
            limbs,
            age: 0.0,
            life: rng.random_range(s.life.clone()),
            strength: power * rng.random_range(s.strength.clone()),
            width: rng.random_range(s.width.clone()),
            phase: rng.random_range(0.0..TAU),
        };
        (bolt, to)
    }
}

/// A jagged path from `a` to `b`.
fn jag(rng: &mut SmallRng, a: [f32; 2], b: [f32; 2], steps: usize, spread: f32) -> Vec<Limb> {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len = (dx * dx + dy * dy).sqrt().max(1.0);
    // Unit normal to the run: the direction the kinks push in.
    let (nx, ny) = (-dy / len, dx / len);
    let pts: Vec<[f32; 2]> = (0..=steps)
        .map(|i| {
            let t = i as f32 / steps as f32;
            // sin is zero at both ends and one in the middle — taut, not frayed.
            let off = rng.random_range(-1.0..1.0) * spread * len * (t * PI).sin();
            [a[0] + dx * t + nx * off, a[1] + dy * t + ny * off]
        })
        .collect();
    pts.windows(2).map(|w| Limb { a: w[0], b: w[1] }).collect()
}

/// The fan an implosion left to release, a bolt at a time.
///
/// Spread over a fifth of a second rather than fired at once, which reads as energy
/// rushing out instead of a single flash — and caps how many bolts are alight together,
/// which is what keeps the cost spike in hand.
#[derive(Default)]
pub struct Burst {
    left: u32,
    timer: f32,
    pub power: f32,
    base: f32,
}

impl Burst {
    pub fn start(&mut self, rng: &mut SmallRng, power: f32) {
        *self = Burst { left: BURST_BOLTS, timer: 0.0, power, base: rng.random_range(0.0..TAU) };
    }

    pub fn tick(&mut self, dt: f32) {
        if self.left > 0 {
            self.timer -= dt;
        }
    }

    /// The angle of the next arm, if one is due.
    pub fn next_arm(&mut self, rng: &mut SmallRng) -> Option<f32> {
        if self.left == 0 || self.timer > 0.0 {
            return None;
        }
        let i = (BURST_BOLTS - self.left) as f32;
        // Evenly spaced so it is a fan, jittered so it is not a diagram.
        let spread = TAU / BURST_BOLTS as f32;
        let jitter: f32 = rng.random_range(-0.35..0.35);
        self.left -= 1;
        // Spread wider than a bolt lives, so the fan is never all alight at once. That is
        // as much a cost decision as a look one.
        self.timer += rng.random_range(0.022..0.042);
        Some(self.base + spread * (i + jitter))
    }
}
