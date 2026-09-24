mod bloom;
mod bolt;
mod flare;
mod glyph;
mod pool;
mod pulse;
mod settings;

pub use bloom::Bloom;
pub use pool::{Cadence, Effect, Pool};
pub use pulse::Drop;
pub use settings::{Lulls, Schedule, Seconds, Settings};

use crate::gpu::{Uniforms, MAX_BOLT_SEGS, MAX_FLARES, MAX_GLYPHS, MAX_PULSES};
use crate::figure::{Glyph, Rendered};
use bolt::{Bolt, Burst, Sky, MAX_BOLTS};
use flare::Flare;
use glyph::Lit;
use pulse::Pulse;

use rand::rngs::SmallRng;
use rand::RngExt;

/// Smoothstep, for envelopes that must not pop at either end.
pub(crate) fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// How much of each effect is on screen, for the viewer's frame-time report.
#[derive(Default, Clone, Copy)]
pub struct Census {
    pub pulses: usize,
    pub limbs: usize,
    pub flares: usize,
    pub glyphs: usize,
    pub bloom: f32,
}

pub struct Effects {
    rng: SmallRng,
    sky: Sky,
    /// Every letter and symbol the figure draws.
    catalogue: Vec<Glyph>,

    pub bloom: Bloom,
    pulses: Pool<Pulse>,
    bolts: Pool<Bolt>,
    flares: Pool<Flare>,
    lit: Pool<Lit>,
    burst: Burst,
    /// How brightly the whole disc is still answering the last collapse.
    collapse: f32,
    /// Where the last bolt landed. The next one leaves from here, so the lightning walks
    /// the diagram instead of teleporting around it.
    spark_at: [f32; 2],

    /// How often each ambient effect happens, from the figure file.
    settings: Settings,
    next_drop: Cadence,
    next_implosion: Cadence,
    next_bolt: Cadence,
    next_lit: Cadence,
}

impl Effects {
    /// Effects for `fig`, timed by the `effects` section of its file.
    pub fn new(fig: &Rendered) -> Self {
        let mut fx = Effects {
            rng: rand::make_rng(),
            sky: Sky { center: fig.center, disc: fig.disc() },
            bloom: Bloom::new(fig.layers.len()),
            catalogue: fig.glyphs.clone(),
            pulses: Pool::new(MAX_PULSES),
            bolts: Pool::new(MAX_BOLTS),
            flares: Pool::new(MAX_FLARES),
            lit: Pool::new(MAX_GLYPHS),
            burst: Burst::default(),
            collapse: 0.0,
            // The first bolt has nowhere to come from, so start it at the centre.
            spark_at: fig.center,
            settings: fig.effects,
            next_drop: Cadence::after(0.0),
            next_implosion: Cadence::after(0.0),
            next_bolt: Cadence::after(0.0),
            next_lit: Cadence::after(0.0),
        };
        fx.rewind();
        fx
    }

    /// Carry on over a figure that has just been edited. Whatever is in flight keeps going;
    /// lit glyphs are dropped, since the letters they lifted may no longer be there, and
    /// every schedule starts afresh so a shortened wait takes effect at once.
    pub fn reload(&mut self, fig: &Rendered) {
        self.sky = Sky { center: fig.center, disc: fig.disc() };
        self.bloom = Bloom::new(fig.layers.len());
        self.catalogue = fig.glyphs.clone();
        self.lit = Pool::new(MAX_GLYPHS);
        self.settings = fig.effects;
        self.rewind();
    }

    /// Start every schedule on a fresh wait, so nothing happens the moment the figure
    /// appears; let it turn for a moment first.
    fn rewind(&mut self) {
        let s = &self.settings;
        self.next_drop.wait(s.drops.every.draw(&mut self.rng));
        self.next_implosion.wait(s.implosions.every.draw(&mut self.rng));
        self.next_bolt.wait(s.lightning.every.draw(&mut self.rng));
        self.next_lit.wait(s.glyphs.every.draw(&mut self.rng));
    }

    /// Start a disturbance.
    pub fn drop(&mut self, kind: Drop) {
        self.pulses.push(Pulse::new(kind, self.sky.disc));
    }

    /// A flare where a bolt came down.
    fn flare(&mut self, pos: [f32; 2], radius: std::ops::Range<f32>, life: std::ops::Range<f32>, power: f32) {
        let f = Flare {
            pos,
            radius: self.rng.random_range(radius),
            age: 0.0,
            life: self.rng.random_range(life),
            strength: power * self.rng.random_range(0.5..0.9),
        };
        self.flares.push(f);
    }

    /// Advance everything by `dt`, drop whatever has finished, follow up on what it left
    /// behind, and spawn whatever is due. None of this is driven by the keyboard.
    pub fn advance(&mut self, dt: f32) {
        self.bloom.fade(dt);
        self.collapse *= (-6.0 * dt).exp();
        self.bolts.advance(dt);
        self.flares.advance(dt);
        self.lit.advance(dt);

        // An implosion that reaches the middle lands, and throws back out everything it
        // gathered on the way in.
        for p in self.pulses.advance(dt).into_iter().filter(Pulse::landed) {
            let gathered = p.gathered();
            self.drop(Drop::Rebound(gathered));
            // It throws off energy as well as blood. The fan is queued rather than fired
            // here so it comes out as a rush over the next fifth of a second.
            self.burst.start(&mut self.rng, gathered.clamp(0.5, 1.5));
            // ...and the whole formation answers.
            self.collapse = 1.0;
            let power = self.burst.power;
            self.flares.push(Flare {
                pos: self.sky.center,
                radius: self.sky.disc * 0.3,
                age: 0.0,
                life: 0.5,
                strength: power * 1.2,
            });
        }

        self.burst.tick(dt);
        while let Some(angle) = self.burst.next_arm(&mut self.rng) {
            let (bolt, to) = self.sky.burst_arm(&mut self.rng, angle, self.burst.power);
            self.bolts.push(bolt);
            self.flare(to, 40.0..80.0, 0.3..0.6, self.burst.power);
        }

        // Each ambient effect on its own schedule. A disabled one keeps its clock running
        // but does nothing, so re-enabling it in the file picks up straight away.
        let s = self.settings;

        if self.next_drop.tick(dt) {
            if s.drops.enabled {
                self.drop(Drop::Droplet);
            }
            self.next_drop.wait(s.drops.every.draw(&mut self.rng));
        }

        if self.next_implosion.tick(dt) {
            if s.implosions.enabled {
                self.drop(Drop::Implosion);
            }
            self.next_implosion.wait(s.implosions.every.draw(&mut self.rng));
        }

        if self.next_bolt.tick(dt) {
            if s.lightning.enabled {
                let (bolt, to) = self.sky.strike(&mut self.rng, self.spark_at);
                self.bolts.push(bolt);
                // Something has to light up where it landed, or the strike has no consequence.
                self.flare(to, 40.0..85.0, 0.35..0.70, 1.0);
                self.spark_at = to;
            }
            let lull = self.rng.random_range(0.0..1.0) < s.lulls.chance;
            let wait = if lull { s.lulls.last } else { s.lightning.every };
            self.next_bolt.wait(wait.draw(&mut self.rng));
        }

        if self.next_lit.tick(dt) {
            if s.glyphs.enabled {
                for l in glyph::cluster(&mut self.rng, &self.catalogue, MAX_GLYPHS) {
                    self.lit.push(l);
                }
            }
            self.next_lit.wait(s.glyphs.every.draw(&mut self.rng));
        }
    }

    /// Write every effect into the uniform block. `angles` is each layer's current turn,
    /// in the same order as the layers `Effects` was built with.
    pub fn write(&self, uni: &mut Uniforms, angles: &[f32]) {
        for (i, l) in uni.layers.iter_mut().enumerate().take(angles.len()) {
            l.motion[1] = self.bloom.level(i);
        }

        for (slot, p) in uni.pulses.iter_mut().zip(self.pulses.iter()) {
            *slot = p.gpu();
        }
        for (slot, f) in uni.flares.iter_mut().zip(self.flares.iter()) {
            *slot = f.gpu();
        }
        // Every live bolt's limbs go into one flat list, which is all the shader wants.
        let mut limbs = 0;
        for (slot, l) in uni.limbs.iter_mut().zip(self.bolts.iter().flat_map(Bolt::gpu)) {
            *slot = l;
            limbs += 1;
        }

        // Two boxes, because the two passes happen in different frames: the glyphs are
        // hidden where they sit in the artwork, the copies are drawn in un-rotated space.
        // Groups are clusters, so both stay tight and almost every pixel skips both.
        let mut art = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
        let mut scr = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
        for (slot, l) in uni.glyphs.iter_mut().zip(self.lit.iter()) {
            let angle = angles.get(l.layer).copied().unwrap_or(0.0);
            *slot = l.gpu(angle, self.sky.center);
            let grow = 1.0 + 1.6 * slot.risen[3];
            for k in 0..2 {
                art[k] = art[k].min(l.pos[k] - l.half[k]);
                art[k + 2] = art[k + 2].max(l.pos[k] + l.half[k]);
                scr[k] = scr[k].min(slot.risen[k] - l.half[k] * grow);
                scr[k + 2] = scr[k + 2].max(slot.risen[k] + l.half[k] * grow);
            }
        }
        let none = self.lit.is_empty();
        uni.gbox = if none { [0.0; 4] } else { art };
        uni.gbox_p = if none { [0.0; 4] } else { scr };

        uni.live = [
            self.pulses.len() as f32,
            self.flares.len() as f32,
            limbs.min(MAX_BOLT_SEGS) as f32,
            self.lit.len() as f32,
        ];
        uni.look[0] = self.collapse;
    }

    pub fn census(&self) -> Census {
        Census {
            pulses: self.pulses.len(),
            limbs: self.bolts.iter().map(|b| b.limbs.len()).sum::<usize>().min(MAX_BOLT_SEGS),
            flares: self.flares.len(),
            glyphs: self.lit.len(),
            bloom: self.bloom.peak(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figure::Figure;

    fn effects() -> (Effects, Vec<f32>) {
        let fig = Figure::load(concat!(env!("CARGO_MANIFEST_DIR"), "/figure.json5")).unwrap();
        let r = fig.render(0.5);
        (Effects::new(&r), vec![0.3; r.layers.len()])
    }

    /// A long run at a ragged framerate never overfills a shader array, and whatever the
    /// counts say is live is actually written.
    #[test]
    fn a_long_run_stays_inside_the_uniform_arrays() {
        let (mut fx, angles) = effects();
        let mut uni: Uniforms = bytemuck::Zeroable::zeroed();
        let mut seen = Census::default();
        for i in 0..6000 {
            if i % 400 == 0 {
                fx.drop(Drop::Implosion);
                fx.drop(Drop::Heavy);
            }
            fx.advance(if i % 7 == 0 { 0.05 } else { 1.0 / 60.0 });
            fx.write(&mut uni, &angles);
            let c = fx.census();
            assert!(c.pulses <= MAX_PULSES && c.flares <= MAX_FLARES);
            assert!(c.limbs <= MAX_BOLT_SEGS && c.glyphs <= MAX_GLYPHS);
            assert_eq!(uni.live[2] as usize, c.limbs);
            seen.pulses = seen.pulses.max(c.pulses);
            seen.limbs = seen.limbs.max(c.limbs);
            seen.glyphs = seen.glyphs.max(c.glyphs);
        }
        assert!(seen.pulses > 0 && seen.limbs > 0 && seen.glyphs > 0);
    }

    /// Count what the schedules start in `secs` of running.
    fn started(settings: Settings, secs: f32) -> (usize, usize, usize) {
        let fig = Figure::load(concat!(env!("CARGO_MANIFEST_DIR"), "/figure.json5")).unwrap();
        let mut r = fig.render(0.25);
        r.effects = settings;
        let mut fx = Effects::new(&r);
        let (mut drops, mut bolts, mut glyphs) = (0, 0, 0);
        let (mut p, mut b, mut g) = (0, 0, 0);
        for _ in 0..(secs * 60.0) as usize {
            fx.advance(1.0 / 60.0);
            let c = fx.census();
            // Count arrivals: a pool that grew started something.
            drops += c.pulses.saturating_sub(p);
            bolts += fx.bolts.len().saturating_sub(b);
            glyphs += c.glyphs.saturating_sub(g);
            (p, b, g) = (c.pulses, fx.bolts.len(), c.glyphs);
        }
        (drops, bolts, glyphs)
    }

    /// The figure file's timings are what actually drive the effects.
    #[test]
    fn settings_decide_what_happens_and_how_often() {
        let mut off = Settings::default();
        for s in [&mut off.drops, &mut off.implosions, &mut off.lightning, &mut off.glyphs] {
            s.enabled = false;
        }
        assert_eq!(started(off, 30.0), (0, 0, 0), "disabled effects still happened");

        let mut slow = Settings::default();
        slow.drops.every = Seconds { min: 5.0, max: 5.0 };
        slow.implosions.enabled = false;
        let mut fast = slow;
        fast.drops.every = Seconds { min: 1.0, max: 1.0 };
        // Ten seconds: short of a ring's twelve-second life and of the pool's sixteen slots,
        // so every drop shows up as the pool growing.
        let (s, f) = (started(slow, 10.0).0, started(fast, 10.0).0);
        assert!((1..=2).contains(&s) && (9..=10).contains(&f), "slow {s}, fast {f}");
    }

    /// An implosion runs all the way in, lands, and throws its rebound and fan out.
    #[test]
    fn an_implosion_lands_and_rebounds() {
        let (mut fx, _) = effects();
        fx.drop(Drop::Implosion);
        let mut landed = false;
        for _ in 0..600 {
            fx.advance(1.0 / 60.0);
            if fx.collapse > 0.9 {
                landed = true;
                break;
            }
        }
        assert!(landed, "the implosion never reached the centre");
    }
}
