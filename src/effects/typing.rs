use super::on_screen;
use crate::figure::Glyph;
use crate::gpu::{GpuMark, MAX_MARKS};

/// Seconds a letter takes to flare up when lit, and to go out when un-typed.
const FLARE: f32 = 0.45;
const FADE: f32 = 0.3;

/// Typing into the figure: every character lights another letter of one ring, stepping
/// about a twelfth of the way round each time, clockwise from North — so what is typed
/// spreads round the ring like the marks of a clock face instead of bunching up in one
/// word. Backspace puts the last one out again.
pub struct Typing {
    /// The ring's letters, clockwise from North.
    sequence: Vec<Glyph>,
    /// The lit ones: (index into `sequence`, seconds since lit, seconds since put out).
    lit: Vec<(usize, f32, Option<f32>)>,
    typed: usize,
}

impl Typing {
    /// Walk the ring with the most letters — outside the middle, where the words are set
    /// square to the page.
    pub fn new(catalogue: &[Glyph], center: [f32; 2]) -> Self {
        let away = |g: &&Glyph| (g.pos[0] - center[0]).hypot(g.pos[1] - center[1]) > 40.0;
        let layers = catalogue.iter().map(|g| g.layer).max().map_or(0, |m| m + 1);
        let busiest = (0..layers).max_by_key(|&l| catalogue.iter().filter(away).filter(|g| g.layer == l).count());
        let bearing = |g: &Glyph| (g.pos[0] - center[0]).atan2(center[1] - g.pos[1]).rem_euclid(std::f32::consts::TAU);
        let mut sequence: Vec<Glyph> =
            catalogue.iter().filter(away).filter(|g| Some(g.layer) == busiest).copied().collect();
        sequence.sort_by(|a, b| bearing(a).total_cmp(&bearing(b)));
        Typing { sequence, lit: Vec::new(), typed: 0 }
    }

    /// A character was typed: light the next letter. Returns where it sits in its layer,
    /// for anything else that wants to answer the keystroke there.
    pub fn key(&mut self) -> Option<Glyph> {
        if self.sequence.is_empty() {
            return None;
        }
        // A twelfth of the way round per key, nudged on by one each lap so the second
        // lap does not land on the first.
        let n = self.sequence.len();
        let stride = (n / 12).max(1);
        let i = (self.typed * stride + self.typed / 12) % n;
        self.typed += 1;
        if self.lit.len() >= MAX_MARKS {
            self.lit.remove(0);
        }
        self.lit.push((i, 0.0, None));
        Some(self.sequence[i])
    }

    /// Backspace: put the last lit letter out.
    pub fn back(&mut self) {
        if let Some(last) = self.lit.iter_mut().rev().find(|l| l.2.is_none()) {
            last.2 = Some(0.0);
            self.typed = self.typed.saturating_sub(1);
        }
    }

    /// Put every letter out: what was typed has been sent.
    pub fn clear(&mut self) {
        for l in self.lit.iter_mut().filter(|l| l.2.is_none()) {
            l.2 = Some(0.0);
        }
        self.typed = 0;
    }

    pub fn advance(&mut self, dt: f32) {
        for l in self.lit.iter_mut() {
            l.1 += dt;
            if let Some(out) = l.2.as_mut() {
                *out += dt;
            }
        }
        self.lit.retain(|l| l.2.is_none_or(|out| out < FADE));
    }

    pub fn is_empty(&self) -> bool {
        self.lit.is_empty()
    }

    /// The lit letters as the shader wants them.
    pub fn gpu<'a>(&'a self, angles: &'a [f32], center: [f32; 2]) -> impl Iterator<Item = GpuMark> + 'a {
        self.lit.iter().map(move |&(i, age, out)| {
            let g = self.sequence[i];
            let fading = out.map_or(1.0, |o| 1.0 - (o / FADE).min(1.0));
            let steady = (age / 0.12).min(1.0) * fading;
            let flash = (1.0 - age / FLARE).max(0.0).powi(2);
            let s = on_screen(g.pos, center, angles.get(g.layer).copied().unwrap_or(0.0));
            GpuMark { slot: [g.pos[0], g.pos[1], g.radius, g.layer as f32], glow: [steady, flash, s[0], s[1]] }
        })
    }

    /// Where each lit letter sits in its layer, and how far it reaches.
    pub fn extents(&self) -> impl Iterator<Item = ([f32; 2], f32)> + '_ {
        self.lit.iter().map(|&(i, _, _)| (self.sequence[i].pos, self.sequence[i].radius))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ring() -> Vec<Glyph> {
        // Four letters round a ring of radius 100, and one lonely one in another layer.
        let mut g: Vec<Glyph> = [(0.0, -100.0), (100.0, 0.0), (0.0, 100.0), (-100.0, 0.0)]
            .iter()
            .map(|&(x, y)| Glyph { pos: [x, y], radius: 5.0, layer: 1 })
            .collect();
        g.push(Glyph { pos: [60.0, 0.0], radius: 5.0, layer: 0 });
        g
    }

    #[test]
    fn it_walks_the_busiest_ring_clockwise_and_backspace_steps_back() {
        // Four letters: a twelfth of the way round is one letter, so it walks them in turn.
        let mut t = Typing::new(&ring(), [0.0, 0.0]);
        let first = t.key().unwrap();
        let second = t.key().unwrap();
        assert_eq!(first.pos, [0.0, -100.0], "starts at North");
        assert_eq!(second.pos, [100.0, 0.0], "then clockwise");
        t.back();
        assert_eq!(t.key().unwrap().pos, [100.0, 0.0], "backspace gave the letter back");
    }
}
