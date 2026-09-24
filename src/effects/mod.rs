mod bloom;
mod bolt;
mod constellation;
mod dissolve;
mod flare;
mod glyph;
mod hover;
mod pool;
mod pulse;
mod scramble;
mod settings;
mod surge;
mod tint;
mod typing;

pub use bloom::Bloom;
pub use pool::{Cadence, Effect, Pool};
pub use pulse::Drop;
pub use settings::{Breath, Build, Direction, Lulls, Palette, Schedule, Seconds, Settings, Shimmer, SurgeTiming};

use crate::gpu::{Uniforms, MAX_BOLT_SEGS, MAX_FLARES, MAX_GLYPHS, MAX_PULSES, MAX_SWAPS, MAX_THREADS, MAX_WAVES};
use crate::figure::{Glyph, Rendered};
use bolt::{Bolt, Burst, Sky, MAX_BOLTS};
use constellation::Constellation;
use dissolve::Dissolve;
use flare::Flare;
use glyph::Lit;
use hover::Hover;
use pulse::Pulse;
use scramble::Swap;
use surge::Surge;
use tint::ColorWave;
use typing::Typing;

use rand::rngs::SmallRng;
use rand::RngExt;

/// How far behind a layer its echoes trail, in seconds of its own spin.
const ECHO_SECONDS: f32 = 0.045;

/// Smoothstep, for envelopes that must not pop at either end.
pub(crate) fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Where a point of a layer, `pos` in the layer's own frame, is on screen once the layer
/// has turned to `angle` about `center`: the inverse of the shader's artwork lookup.
pub(crate) fn on_screen(pos: [f32; 2], center: [f32; 2], angle: f32) -> [f32; 2] {
    let (s, c) = angle.sin_cos();
    let (x, y) = (pos[0] - center[0], pos[1] - center[1]);
    [center[0] + x * c + y * s, center[1] + y * c - x * s]
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
    /// Each layer's name, its own speed in revolutions per second (positive clockwise),
    /// and how far it has turned: radians counter-clockwise, as the shader wants it.
    names: Vec<String>,
    speeds: Vec<f32>,
    angles: Vec<f32>,
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
    /// What Enter sets off, and this frame's screen shake from it, in canvas units.
    surge: Surge,
    shake: [f32; 2],
    waves: Pool<ColorWave>,
    dissolves: Pool<Dissolve>,
    swaps: Pool<Swap>,
    constellations: Pool<Constellation>,
    hover: Hover,
    typing: Typing,
    /// Seconds since start, for everything the shader animates on its own.
    time: f32,
    /// How far the figure has drawn itself in: 0 is nothing yet, 1 is all of it.
    build: f32,

    /// How often each ambient effect happens, from the figure file.
    settings: Settings,
    next_implosion: Cadence,
    next_bolt: Cadence,
    next_lit: Cadence,
    next_color: Cadence,
    next_dissolve: Cadence,
    next_scramble: Cadence,
    next_constellation: Cadence,
}

impl Effects {
    /// Effects for `fig`, timed by the `effects` section of its file.
    pub fn new(fig: &Rendered) -> Self {
        let mut fx = Effects {
            rng: rand::make_rng(),
            sky: Sky { center: fig.center, disc: fig.disc() },
            names: Vec::new(),
            speeds: Vec::new(),
            angles: Vec::new(),
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
            surge: Surge::new(),
            shake: [0.0; 2],
            waves: Pool::new(MAX_WAVES),
            dissolves: Pool::new(4),
            swaps: Pool::new(MAX_SWAPS),
            constellations: Pool::new(3),
            hover: Hover::new(),
            typing: Typing::new(&fig.glyphs, fig.center),
            time: 0.0,
            build: if fig.effects.build.on_start { 0.0 } else { 1.0 },
            settings: fig.effects,
            next_implosion: Cadence::after(0.0),
            next_bolt: Cadence::after(0.0),
            next_lit: Cadence::after(0.0),
            next_color: Cadence::after(0.0),
            next_dissolve: Cadence::after(0.0),
            next_scramble: Cadence::after(0.0),
            next_constellation: Cadence::after(0.0),
        };
        fx.adopt_layers(fig);
        fx.rewind();
        fx
    }

    /// Take on the layers of `fig`. A layer that was there before, by name, carries on
    /// from the angle it had reached, so saving an edit does not make everything jump.
    fn adopt_layers(&mut self, fig: &Rendered) {
        let loop_secs = fig.loop_secs.max(1e-3);
        self.angles = fig
            .layers
            .iter()
            .map(|l| self.names.iter().position(|n| *n == l.name).map_or(0.0, |i| self.angles[i]))
            .collect();
        self.speeds = fig.layers.iter().map(|l| l.turns as f32 / loop_secs).collect();
        self.names = fig.layers.iter().map(|l| l.name.clone()).collect();
    }

    /// Each layer's own speed, revolutions per second, positive clockwise.
    pub fn speeds(&self) -> &[f32] {
        &self.speeds
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
        self.typing = Typing::new(&fig.glyphs, fig.center);
        self.adopt_layers(fig);
        self.rewind();
    }

    /// Start every schedule on a fresh wait, so nothing happens the moment the figure
    /// appears; let it turn for a moment first.
    fn rewind(&mut self) {
        let s = &self.settings;
        self.next_implosion.wait(s.implosions.every.draw(&mut self.rng));
        self.next_bolt.wait(s.lightning.every.draw(&mut self.rng));
        self.next_lit.wait(s.glyphs.every.draw(&mut self.rng));
        self.next_color.wait(s.colors.every.draw(&mut self.rng));
        self.next_dissolve.wait(s.dissolve.every.draw(&mut self.rng));
        self.next_scramble.wait(s.scramble.every.draw(&mut self.rng));
        self.next_constellation.wait(s.constellations.every.draw(&mut self.rng));
    }

    /// Show the whole figure at once, without drawing it in.
    pub fn skip_build(&mut self) {
        self.build = 1.0;
    }

    /// Link a few letters across the rings with glowing threads.
    pub fn constellation(&mut self) {
        let (c, n) = (self.sky.center, self.settings.constellation_stars);
        if let Some(k) = Constellation::new(&mut self.rng, &self.catalogue, &self.angles, c, n) {
            self.constellations.push(k);
        }
    }

    /// The pointer moved to `pos`, in canvas units, or left the window.
    pub fn pointer(&mut self, pos: Option<[f32; 2]>) {
        if self.settings.hover > 0.0 {
            self.hover.moved(pos);
        }
    }

    /// A character was typed: light the next letter of the ring, with a small flare where
    /// it sits on screen.
    pub fn key(&mut self) {
        if !self.settings.typing {
            return;
        }
        if let Some(g) = self.typing.key() {
            let at = on_screen(g.pos, self.sky.center, self.angles.get(g.layer).copied().unwrap_or(0.0));
            self.flares.push(Flare { pos: at, radius: g.radius * 2.5, age: 0.0, life: 0.4, strength: 0.4 });
        }
    }

    /// Backspace: put the last lit letter out.
    pub fn backspace(&mut self) {
        self.typing.back();
    }

    /// Set a run of neighbouring letters flickering through other letters of the figure.
    pub fn scramble(&mut self) {
        let s = self.settings;
        let run = scramble::scramble(&mut self.rng, &self.catalogue, self.sky.center, s.scramble_letters, s.scramble_time);
        for swap in run {
            self.swaps.push(swap);
        }
    }

    /// Roll a wave of colour across the figure, in a colour from the palette, whichever way
    /// `colors.direction` says.
    pub fn color_wave(&mut self) {
        let palette = self.settings.palette;
        let colors = palette.colors();
        let color = colors[self.rng.random_range(0..colors.len())];
        let inward = match self.settings.color_direction {
            Direction::Out => false,
            Direction::In => true,
            Direction::Both => self.rng.random_range(0.0..1.0) < 0.5,
        };
        self.waves.push(ColorWave::new(self.sky.disc, color, inward));
    }

    /// Burn one layer away and grow it back — one that is not already burning.
    pub fn dissolve(&mut self) {
        let n = self.bloom.layers();
        let busy: Vec<usize> = self.dissolves.iter().map(|d| d.layer).collect();
        let free: Vec<usize> = (0..n).filter(|i| !busy.contains(i)).collect();
        if let Some(&layer) = free.get(self.rng.random_range(0..free.len().max(1))) {
            self.dissolves.push(Dissolve::new(layer, self.settings.dissolve_time));
        }
    }

    /// Start a disturbance.
    pub fn drop(&mut self, kind: Drop) {
        self.pulses.push(Pulse::new(kind, self.sky.disc));
    }

    /// Set off the surge: a blood drop lands, the figure spins up and gathers light, and
    /// explodes. Does nothing while one is already running.
    pub fn surge(&mut self) {
        if self.surge.start(&mut self.rng, self.bloom.layers()) {
            self.drop(Drop::Heavy);
        }
    }

    /// How fast layer `i` turns right now, in revolutions per second, given its own
    /// `speed`. Normally just `speed`; during the surge much faster, and in the layer's
    /// own direction — a layer that normally stands still picks one by its position.
    pub fn spin_rate(&self, i: usize, speed: f32) -> f32 {
        let (mult, extra) = self.surge.spin(&self.settings.surge);
        let way = if speed != 0.0 { speed.signum() } else if i.is_multiple_of(2) { 1.0 } else { -1.0 };
        speed * (1.0 + mult) + way * extra
    }

    /// How far to nudge the whole picture this frame, in canvas units.
    pub fn shake(&self) -> [f32; 2] {
        self.shake
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

    /// Energy released at the centre: a ring of blood thrown out, a fan of lightning, and
    /// the whole formation lighting up. What an implosion does when it lands, and what the
    /// surge ends in.
    fn detonate(&mut self, power: f32) {
        self.drop(Drop::Rebound(power));
        // The fan is queued rather than fired here so it comes out as a rush over the
        // next fifth of a second.
        self.burst.start(&mut self.rng, power.clamp(0.5, 1.5));
        self.collapse = self.collapse.max(1.0);
        let strength = self.burst.power * 1.2;
        self.flares.push(Flare { pos: self.sky.center, radius: self.sky.disc * 0.3, age: 0.0, life: 0.5, strength });
    }

    /// Advance everything by `dt`, drop whatever has finished, follow up on what it left
    /// behind, and spawn whatever is due.
    pub fn advance(&mut self, dt: f32) {
        // Wrapped well before f32 loses the precision a slow shimmer needs.
        self.time = (self.time + dt) % 3600.0;
        self.bloom.fade(dt);
        self.waves.advance(dt);
        self.dissolves.advance(dt);
        self.swaps.advance(dt);
        self.constellations.advance(dt);
        self.hover.advance(dt);
        self.typing.advance(dt);
        self.collapse *= (-6.0 * dt).exp();
        self.bolts.advance(dt);
        self.flares.advance(dt);
        self.lit.advance(dt);

        // An implosion that reaches the middle lands, and throws back out everything it
        // gathered on the way in.
        for p in self.pulses.advance(dt).into_iter().filter(Pulse::landed) {
            self.detonate(p.gathered());
        }

        self.advance_surge(dt);
        // Each layer at its own fixed rate, unless the surge has it spinning up. Speeds are
        // clockwise; the angle the shader wants runs the other way.
        for i in 0..self.angles.len() {
            self.angles[i] -= self.spin_rate(i, self.speeds[i]) * dt * std::f32::consts::TAU;
        }

        self.burst.tick(dt);
        while let Some(angle) = self.burst.next_arm(&mut self.rng) {
            let (bolt, to) = self.sky.burst_arm(&mut self.rng, angle, self.burst.power);
            self.bolts.push(bolt);
            self.flare(to, 40.0..80.0, 0.3..0.6, self.burst.power);
        }

        self.advance_ambient(dt);
    }

    fn advance_surge(&mut self, dt: f32) {
        let timing = self.settings.surge;
        // Drawing itself in: over its own time at startup, and in step with the reform after
        // a surge, so the figure writes itself back rather than fading in.
        match self.surge.reforming(&timing) {
            Some(u) if self.settings.build.after_surge => self.build = u,
            _ => self.build = (self.build + dt / self.settings.build.duration).min(1.0),
        }
        if self.surge.advance(dt, &timing) {
            // Bigger than any implosion, and the letters in flight go with the formation.
            self.detonate(1.5);
            self.collapse = 1.6;
            self.lit = Pool::new(MAX_GLYPHS);
            // Whatever was typed goes with it.
            self.typing.clear();
        }
        // Every layer gathers light as the charge builds, all the way to full.
        self.bloom.raise_all(self.surge.charge(&timing));
        let amount = self.surge.shake(&timing);
        self.shake = if amount > 0.0 {
            [self.rng.random_range(-amount..amount), self.rng.random_range(-amount..amount)]
        } else {
            [0.0; 2]
        };
    }

    /// Each ambient effect on its own schedule. A disabled one keeps its clock running but
    /// does nothing, so re-enabling it in the file picks up straight away. While the surge
    /// runs they hold off — all but the lightning, which the charge whips up instead.
    fn advance_ambient(&mut self, dt: f32) {
        let s = self.settings;
        let charge = self.surge.charge(&s.surge);
        // Nor while the figure is still drawing itself in: a letter cannot light up, scramble
        // or burn before it is there.
        let calm = !self.surge.busy() && self.build >= 1.0;

        if self.next_implosion.tick(dt) {
            if s.implosions.enabled && calm {
                self.drop(Drop::Implosion);
            }
            self.next_implosion.wait(s.implosions.every.draw(&mut self.rng));
        }

        if self.next_bolt.tick(dt) {
            if s.lightning.enabled && (calm || charge > 0.0) {
                let (bolt, to) = self.sky.strike(&mut self.rng, self.spark_at);
                self.bolts.push(bolt);
                // Something has to light up where it landed, or the strike has no consequence.
                self.flare(to, 40.0..85.0, 0.35..0.70, 1.0);
                self.spark_at = to;
            }
            let lull = charge == 0.0 && self.rng.random_range(0.0..1.0) < s.lulls.chance;
            let wait = if lull { s.lulls.last } else { s.lightning.every };
            // Up to eight times as often at the height of the charge.
            self.next_bolt.wait(wait.draw(&mut self.rng) / (1.0 + 7.0 * charge));
        }

        if self.next_lit.tick(dt) {
            if s.glyphs.enabled && calm {
                for l in glyph::cluster(&mut self.rng, &self.catalogue, MAX_GLYPHS) {
                    self.lit.push(l);
                }
            }
            self.next_lit.wait(s.glyphs.every.draw(&mut self.rng));
        }

        if self.next_color.tick(dt) {
            if s.colors.enabled && calm {
                self.color_wave();
            }
            self.next_color.wait(s.colors.every.draw(&mut self.rng));
        }

        if self.next_dissolve.tick(dt) {
            if s.dissolve.enabled && calm {
                self.dissolve();
            }
            self.next_dissolve.wait(s.dissolve.every.draw(&mut self.rng));
        }

        if self.next_scramble.tick(dt) {
            if s.scramble.enabled && calm {
                self.scramble();
            }
            self.next_scramble.wait(s.scramble.every.draw(&mut self.rng));
        }

        if self.next_constellation.tick(dt) {
            if s.constellations.enabled && calm {
                self.constellation();
            }
            self.next_constellation.wait(s.constellations.every.draw(&mut self.rng));
        }
    }

    /// Write the layers' turn and every effect into the uniform block.
    pub fn write(&self, uni: &mut Uniforms) {
        let timing = &self.settings.surge;
        let s_echo = self.settings.echoes;
        let angles = &self.angles;
        for (i, l) in uni.layers.iter_mut().enumerate().take(angles.len()) {
            // The shader rotates by the angle rather than un-rotating an arctangent, so hand
            // it the sin and cos instead of making every pixel work them out.
            let (s, c) = angles[i].sin_cos();
            l.motion = [angles[i], self.bloom.level(i), s, c];
            let (scale, mut opacity) = self.surge.form(i, timing);
            if self.settings.build.after_surge && self.surge.reforming(timing).is_some() {
                // The build is doing the appearing; fading in on top would only mute it.
                opacity = 1.0;
            }
            let burnt: f32 = self.dissolves.iter().filter(|d| d.layer == i).map(Dissolve::amount).sum();
            // Echoes trail where the layer was a moment ago; only a fast spin leaves them far
            // enough behind to see. Signed like the angle, which runs against the speed.
            let lag = if s_echo > 0.0 { -self.spin_rate(i, self.speeds[i]) * std::f32::consts::TAU * ECHO_SECONDS } else { 0.0 };
            l.form = [scale, opacity, burnt.min(1.0), lag];
        }

        let s = &self.settings;
        // The ink writhes as the surge builds, up to five times its resting waver.
        let haze = s.haze * (1.0 + 4.0 * self.surge.charge(timing));
        uni.ambient = [s.shimmer.strength, s.shimmer.speed, haze, self.waves.len() as f32];
        for (slot, w) in uni.waves.iter_mut().zip(self.waves.iter()) {
            *slot = w.gpu();
        }
        uni.look[3] = self.time;
        // A layer flying apart is drawn larger than the disc, and the shader has to look
        // that far out for it.
        uni.quality[1] = self.sky.disc * self.surge.reach(timing);
        uni.look[2] = self.surge.blast(timing);

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
        // Scrambling letters are drawn in the same pass that hides lit ones, so their slots
        // widen the same box.
        for (slot, sw) in uni.swaps.iter_mut().zip(self.swaps.iter()) {
            *slot = sw.gpu();
            let (pos, reach) = sw.extent();
            for k in 0..2 {
                art[k] = art[k].min(pos[k] - reach);
                art[k + 2] = art[k + 2].max(pos[k] + reach);
            }
        }
        // ...and so do letters lit by typing.
        for (pos, reach) in self.typing.extents() {
            for k in 0..2 {
                art[k] = art[k].min(pos[k] - reach);
                art[k + 2] = art[k + 2].max(pos[k] + reach);
            }
        }
        let none = self.lit.is_empty() && self.swaps.is_empty() && self.typing.is_empty();
        uni.gbox = if none { [0.0; 4] } else { art };
        uni.gbox_p = if self.lit.is_empty() { [0.0; 4] } else { scr };
        let b = s.breath;
        uni.rhythm = [self.swaps.len() as f32, b.strength, b.period, self.build];

        let c = self.sky.center;
        let mut threads = 0;
        let all = self.constellations.iter().flat_map(|k| k.gpu(angles, c));
        for (slot, t) in uni.threads.iter_mut().zip(all) {
            *slot = t;
            threads += 1;
        }
        for (slot, r) in uni.ripples.iter_mut().zip(self.hover.ripples.iter()) {
            *slot = r.gpu(s.hover);
        }
        let mut marks = 0;
        for (slot, m) in uni.marks.iter_mut().zip(self.typing.gpu(angles, c)) {
            *slot = m;
            marks += 1;
        }
        uni.live2 = [threads.min(MAX_THREADS) as f32, self.hover.ripples.len() as f32, marks as f32, s_echo];
        uni.pointer = [self.hover.pos[0], self.hover.pos[1], self.hover.presence * s.hover, 0.0];

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

    fn effects() -> Effects {
        let fig = Figure::load(concat!(env!("CARGO_MANIFEST_DIR"), "/figure.json5")).unwrap();
        let r = fig.render(0.5);
        Effects::new(&r)
    }

    /// A long run at a ragged framerate never overfills a shader array, and whatever the
    /// counts say is live is actually written.
    #[test]
    fn a_long_run_stays_inside_the_uniform_arrays() {
        let mut fx = effects();
        let mut uni: Uniforms = bytemuck::Zeroable::zeroed();
        let mut seen = Census::default();
        for i in 0..6000 {
            if i % 400 == 0 {
                fx.drop(Drop::Implosion);
                fx.drop(Drop::Heavy);
            }
            fx.advance(if i % 7 == 0 { 0.05 } else { 1.0 / 60.0 });
            fx.write(&mut uni);
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
    fn started(mut settings: Settings, secs: f32) -> (usize, usize, usize) {
        // Timings only: start from a finished figure, not one still drawing itself in.
        settings.build.on_start = false;
        let fig = Figure::load(concat!(env!("CARGO_MANIFEST_DIR"), "/figure.json5")).unwrap();
        let mut r = fig.render(0.25);
        r.effects = settings;
        let mut fx = Effects::new(&r);
        let (mut rings, mut bolts, mut glyphs) = (0, 0, 0);
        let (mut p, mut b, mut g) = (0, 0, 0);
        for _ in 0..(secs * 60.0) as usize {
            fx.advance(1.0 / 60.0);
            let c = fx.census();
            // Count arrivals: a pool that grew started something.
            rings += c.pulses.saturating_sub(p);
            bolts += fx.bolts.len().saturating_sub(b);
            glyphs += c.glyphs.saturating_sub(g);
            (p, b, g) = (c.pulses, fx.bolts.len(), c.glyphs);
        }
        (rings, bolts, glyphs)
    }

    /// The figure file's timings are what actually drive the effects.
    #[test]
    fn settings_decide_what_happens_and_how_often() {
        let mut off = Settings::default();
        for s in [&mut off.implosions, &mut off.lightning, &mut off.glyphs] {
            s.enabled = false;
        }
        assert_eq!(started(off, 30.0), (0, 0, 0), "disabled effects still happened");

        let mut slow = Settings::default();
        slow.implosions.every = Seconds { min: 5.0, max: 5.0 };
        let mut fast = slow;
        fast.implosions.every = Seconds { min: 2.0, max: 2.0 };
        // Ten seconds, short of a ring's twelve-second life, and few enough that the rings
        // and the rebounds they turn into fit the pool: every implosion shows up as the
        // pool growing. (A landing swaps its ring for a rebound, which nets to nothing.)
        let (s, f) = (started(slow, 10.0).0, started(fast, 10.0).0);
        assert!((1..=2).contains(&s) && (4..=5).contains(&f), "slow {s}, fast {f}");
    }

    /// Enter: a drop, a spin-up that only grows, one explosion, and the layers flung out.
    #[test]
    fn the_surge_spins_up_explodes_and_breaks_the_formation() {
        let mut fx = effects();
        // Its own timing, not whatever the figure file is tuned to today.
        let t = SurgeTiming { charge: 1.5, scatter: 1.5, hold: 1.0, reform: 1.0, stay_broken: false };
        fx.settings.surge = t;
        fx.surge();
        assert_eq!(fx.pulses.len(), 1, "Enter drops blood");
        let mut last = fx.spin_rate(0, 0.1);
        let mut uni: Uniforms = bytemuck::Zeroable::zeroed();
        for _ in 0..((t.charge - 0.1) * 60.0) as usize {
            fx.advance(1.0 / 60.0);
            let now = fx.spin_rate(0, 0.1);
            assert!(now >= last, "the spin-up faltered");
            last = now;
        }
        assert!(last > 0.1 * 5.0, "barely spun up: {last}");
        assert!(fx.collapse < 1.5, "exploded early");
        // The explosion lands within the next fifth of a second; catch its peak.
        let mut peak = 0.0f32;
        for _ in 0..12 {
            fx.advance(1.0 / 60.0);
            peak = peak.max(fx.collapse);
        }
        assert!(peak >= 1.5, "no explosion");
        for _ in 0..30 {
            fx.advance(1.0 / 60.0);
        }
        fx.write(&mut uni);
        assert!(uni.layers.iter().take(fx.speeds().len()).all(|l| l.form[0] > 1.0), "a layer stayed put");
        assert!(uni.quality[1] > fx.sky.disc, "the shader would clip the flying layers");
    }

    /// The figure starts with nothing drawn, is whole after `build.duration`, and draws
    /// itself back in as it reforms after a surge.
    #[test]
    fn the_figure_draws_itself_in() {
        let mut fx = effects();
        fx.settings.build.duration = 1.0;
        fx.settings.surge = SurgeTiming { charge: 0.5, scatter: 0.5, hold: 0.5, reform: 1.0, stay_broken: false };
        let mut uni: Uniforms = bytemuck::Zeroable::zeroed();
        fx.write(&mut uni);
        assert_eq!(uni.rhythm[3], 0.0, "something was drawn before the build began");
        for _ in 0..70 {
            fx.advance(1.0 / 60.0);
        }
        fx.write(&mut uni);
        assert_eq!(uni.rhythm[3], 1.0);

        fx.surge();
        // Charge, break and hold: 1.5 s. Half-way through the reform, half drawn.
        for _ in 0..120 {
            fx.advance(1.0 / 60.0);
        }
        fx.write(&mut uni);
        assert!((uni.rhythm[3] - 0.5).abs() < 0.05, "{}", uni.rhythm[3]);
        assert!(uni.layers[0].form[1] > 0.99, "the reform fades in on top of the build");
    }

    /// An implosion runs all the way in, lands, and throws its rebound and fan out.
    #[test]
    fn an_implosion_lands_and_rebounds() {
        let mut fx = effects();
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
