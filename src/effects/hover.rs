use super::pool::{Effect, Pool};
use crate::gpu::{GpuRipple, MAX_RIPPLES};

/// Seconds a ripple takes to spread and fade.
const LIFE: f32 = 1.3;
/// How far the pointer moves, in canvas units, before it throws another ripple.
const STRIDE: f32 = 28.0;

/// A small ring of light spreading from where the pointer passed.
pub struct Ripple {
    pos: [f32; 2],
    age: f32,
}

impl Ripple {
    pub fn gpu(&self, strength: f32) -> GpuRipple {
        let u = (self.age / LIFE).min(1.0);
        GpuRipple { at: [self.pos[0], self.pos[1], 95.0 * u.powf(0.7), strength * (1.0 - u) * (1.0 - u)] }
    }
}

impl Effect for Ripple {
    fn advance(&mut self, dt: f32) {
        self.age += dt;
    }

    fn done(&self) -> bool {
        self.age >= LIFE
    }
}

/// The pointer over the figure: where it is, how present it is — rising as it comes in and
/// falling away when it leaves — and the ripples it leaves behind as it moves.
pub struct Hover {
    pub pos: [f32; 2],
    pub presence: f32,
    inside: bool,
    last_ripple: Option<[f32; 2]>,
    pub ripples: Pool<Ripple>,
}

impl Hover {
    pub fn new() -> Self {
        Hover { pos: [0.0; 2], presence: 0.0, inside: false, last_ripple: None, ripples: Pool::new(MAX_RIPPLES) }
    }

    /// The pointer moved to `pos`, in canvas units, or left the window.
    pub fn moved(&mut self, pos: Option<[f32; 2]>) {
        self.inside = pos.is_some();
        let Some(p) = pos else {
            self.last_ripple = None;
            return;
        };
        self.pos = p;
        let far = self.last_ripple.is_none_or(|q| (p[0] - q[0]).hypot(p[1] - q[1]) >= STRIDE);
        if far {
            self.ripples.push(Ripple { pos: p, age: 0.0 });
            self.last_ripple = Some(p);
        }
    }

    pub fn advance(&mut self, dt: f32) {
        self.ripples.advance(dt);
        // Eases in and out over about a quarter of a second, so the glow follows gently.
        let target = if self.inside { 1.0 } else { 0.0 };
        self.presence += (target - self.presence) * (1.0 - (-dt * 8.0).exp());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A ripple for every stride of movement, not every twitch; presence follows the
    /// pointer in and out of the window.
    #[test]
    fn ripples_follow_movement_and_presence_follows_the_pointer() {
        let mut h = Hover::new();
        h.moved(Some([0.0, 0.0]));
        h.moved(Some([5.0, 0.0]));
        assert_eq!(h.ripples.len(), 1, "a twitch threw a ripple");
        h.moved(Some([40.0, 0.0]));
        assert_eq!(h.ripples.len(), 2);
        for _ in 0..60 {
            h.advance(1.0 / 60.0);
        }
        assert!(h.presence > 0.99);
        h.moved(None);
        for _ in 0..60 {
            h.advance(1.0 / 60.0);
        }
        assert!(h.presence < 0.01);
    }
}
