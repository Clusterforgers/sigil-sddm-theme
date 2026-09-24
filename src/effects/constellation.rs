use super::pool::Effect;
use super::{on_screen, smoothstep};
use crate::figure::Glyph;
use crate::gpu::GpuThread;

use rand::rngs::SmallRng;
use rand::RngExt;

/// Seconds a constellation lasts, from the first thread to the last fading.
const LIFE: f32 = 2.6;
/// Seconds between one thread starting to draw and the next.
const STAGGER: f32 = 0.22;

/// Glowing threads linking a few letters across different rings, one after another, as if
/// the figure were spelling something out. The letters keep turning with their rings, and
/// the threads stretch to follow them.
pub struct Constellation {
    stars: Vec<Glyph>,
    age: f32,
}

impl Constellation {
    /// A chain of up to `count` letters, each in a different ring from the one before and
    /// a reasonable step away on screen, so the threads cross the figure rather than
    /// running along a single row.
    pub fn new(rng: &mut SmallRng, catalogue: &[Glyph], angles: &[f32], center: [f32; 2], count: u32) -> Option<Self> {
        let at = |g: &Glyph| on_screen(g.pos, center, angles.get(g.layer).copied().unwrap_or(0.0));
        let away = |g: &Glyph| (g.pos[0] - center[0]).hypot(g.pos[1] - center[1]) > 40.0;
        let pool: Vec<Glyph> = catalogue.iter().filter(|g| away(g)).copied().collect();
        if pool.is_empty() {
            return None;
        }
        let mut stars = vec![pool[rng.random_range(0..pool.len())]];
        while stars.len() < count as usize {
            let last = *stars.last().unwrap();
            let from = at(&last);
            let next: Vec<Glyph> = pool
                .iter()
                .filter(|g| g.layer != last.layer)
                .filter(|g| {
                    let p = at(g);
                    (70.0..220.0).contains(&(p[0] - from[0]).hypot(p[1] - from[1]))
                })
                .copied()
                .collect();
            match next.get(rng.random_range(0..next.len().max(1))) {
                Some(&g) => stars.push(g),
                None => break,
            }
        }
        (stars.len() >= 2).then_some(Constellation { stars, age: 0.0 })
    }

    /// Its threads as they stand, with the letters where their rings have turned them.
    pub fn gpu<'a>(&'a self, angles: &'a [f32], center: [f32; 2]) -> impl Iterator<Item = GpuThread> + 'a {
        let fade = 1.0 - smoothstep(0.7, 1.0, self.age / LIFE);
        let at = move |g: &Glyph| on_screen(g.pos, center, angles.get(g.layer).copied().unwrap_or(0.0));
        self.stars.windows(2).enumerate().filter_map(move |(i, pair)| {
            // Each thread draws out from its first letter over a fifth of a second.
            let t = (self.age - i as f32 * STAGGER) / 0.2;
            if t <= 0.0 {
                return None;
            }
            let (a, b) = (at(&pair[0]), at(&pair[1]));
            let reach = t.min(1.0);
            let end = [a[0] + (b[0] - a[0]) * reach, a[1] + (b[1] - a[1]) * reach];
            // A spark runs along each thread, again and again, while it lasts.
            let spark = (t * 0.35).fract();
            Some(GpuThread { seg: [a[0], a[1], end[0], end[1]], style: [fade, spark, 0.0, 0.0] })
        })
    }
}

impl Effect for Constellation {
    fn advance(&mut self, dt: f32) {
        self.age += dt;
    }

    fn done(&self) -> bool {
        self.age >= LIFE
    }
}
