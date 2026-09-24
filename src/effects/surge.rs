//! What Enter sets off: the figure spins up and gathers light, explodes, flies apart and —
//! unless told to stay broken — comes back together.
//!
//! This file only keeps time and says how far along each part of the sequence is. What
//! those numbers do to the picture is decided by `Effects` and the shader.

use super::settings::SurgeTiming;
use super::smoothstep;

use rand::rngs::SmallRng;
use rand::RngExt;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    Idle,
    /// Spinning up and gathering light.
    Charge,
    /// Exploded: the layers fly apart and fade.
    Scatter,
    /// Gone.
    Hold,
    /// Coming back together.
    Reform,
}

/// The fastest the layers turn at the height of the charge: this many times their own
/// speed, plus this many revolutions per second on top, so a layer that normally stands
/// still spins too.
const SPIN_MULT: f32 = 9.0;
const SPIN_EXTRA: f32 = 1.1;

pub struct Surge {
    phase: Phase,
    /// Seconds into the current phase.
    t: f32,
    /// How far each layer flies when the formation breaks, as extra scale at the end of
    /// the scatter. Drawn fresh each time so no two explosions look alike.
    flight: Vec<f32>,
}

impl Surge {
    pub fn new() -> Self {
        Surge { phase: Phase::Idle, t: 0.0, flight: Vec::new() }
    }

    /// Start the sequence, unless it is already running. True if it started.
    pub fn start(&mut self, rng: &mut SmallRng, layers: usize) -> bool {
        if self.phase != Phase::Idle {
            return false;
        }
        self.phase = Phase::Charge;
        self.t = 0.0;
        // Inner layers fly further: they have further to go to clear the outer ones.
        self.flight = (0..layers)
            .map(|i| {
                let inner = 1.0 - i as f32 / layers.max(1) as f32;
                0.35 + 0.9 * inner + rng.random_range(0.0..0.35)
            })
            .collect();
        true
    }

    /// Move on by `dt`. True on the one frame the charge turns into the explosion.
    pub fn advance(&mut self, dt: f32, timing: &SurgeTiming) -> bool {
        if self.phase == Phase::Idle {
            return false;
        }
        self.t += dt;
        let (length, next) = match self.phase {
            Phase::Charge => (timing.charge, Phase::Scatter),
            Phase::Scatter => (timing.scatter, Phase::Hold),
            Phase::Hold if timing.stay_broken => return false,
            Phase::Hold => (timing.hold, Phase::Reform),
            Phase::Reform => (timing.reform, Phase::Idle),
            Phase::Idle => unreachable!(),
        };
        if self.t < length {
            return false;
        }
        let exploded = self.phase == Phase::Charge;
        self.phase = next;
        self.t = 0.0;
        exploded
    }

    /// Whether the sequence is running, in which case the ambient effects hold off.
    pub fn busy(&self) -> bool {
        self.phase != Phase::Idle
    }

    /// Progress through the current phase, 0 to 1.
    fn u(&self, timing: &SurgeTiming) -> f32 {
        let length = match self.phase {
            Phase::Idle => return 0.0,
            Phase::Charge => timing.charge,
            Phase::Scatter => timing.scatter,
            Phase::Hold => timing.hold,
            Phase::Reform => timing.reform,
        };
        (self.t / length.max(1e-3)).min(1.0)
    }

    /// How far through coming back together it is, 0 to 1, while it is.
    pub fn reforming(&self, timing: &SurgeTiming) -> Option<f32> {
        (self.phase == Phase::Reform).then(|| self.u(timing))
    }

    /// How far the charge has built, 0 to 1. Rises slowly and then fast, so it reads as
    /// something running away rather than a fader being pushed.
    pub fn charge(&self, timing: &SurgeTiming) -> f32 {
        match self.phase {
            Phase::Charge => self.u(timing).powi(2),
            _ => 0.0,
        }
    }

    /// How much faster than usual the layers turn: (times their own speed, extra
    /// revolutions per second).
    pub fn spin(&self, timing: &SurgeTiming) -> (f32, f32) {
        let u = self.u(timing);
        let k = match self.phase {
            Phase::Idle | Phase::Hold => 0.0,
            Phase::Charge => u * u,
            // Still spinning as the pieces fly, winding down.
            Phase::Scatter => 1.0 - 0.6 * u,
            // Coming back in with a last bit of spin that settles as it lands.
            Phase::Reform => 0.3 * (1.0 - u) * (1.0 - u),
        };
        (SPIN_MULT * k, SPIN_EXTRA * k)
    }

    /// Layer `i`'s size and opacity.
    pub fn form(&self, i: usize, timing: &SurgeTiming) -> (f32, f32) {
        let u = self.u(timing);
        let flight = self.flight.get(i).copied().unwrap_or(1.0);
        match self.phase {
            Phase::Idle => (1.0, 1.0),
            // Drawing in on itself before it goes.
            Phase::Charge => (1.0 - 0.04 * u.powi(3), 1.0),
            Phase::Scatter => {
                // Thrown outward hard, slowing as it goes, and gone before it stops.
                let out = 1.0 - (1.0 - u).powi(3);
                (1.0 + flight * out, 1.0 - smoothstep(0.1, 0.85, u))
            }
            Phase::Hold => (1.0, 0.0),
            // Settling in from slightly too large.
            Phase::Reform => (1.0 + 0.15 * (1.0 - u).powi(2), smoothstep(0.0, 0.8, u)),
        }
    }

    /// The largest any layer is drawn, so the shader knows how far out to look.
    pub fn reach(&self, timing: &SurgeTiming) -> f32 {
        (0..self.flight.len().max(1)).map(|i| self.form(i, timing).0).fold(1.0, f32::max)
    }

    /// How hard the screen shakes, in canvas units: a rumble that builds with the charge,
    /// then a hard jolt at the explosion that dies away.
    pub fn shake(&self, timing: &SurgeTiming) -> f32 {
        let u = self.u(timing);
        match self.phase {
            Phase::Charge => 2.5 * u.powi(3),
            Phase::Scatter => 14.0 * (1.0 - u).powi(4),
            _ => 0.0,
        }
    }

    /// The white flash of the explosion, 0 to 1.
    pub fn blast(&self, timing: &SurgeTiming) -> f32 {
        match self.phase {
            Phase::Scatter => (1.0 - self.u(timing)).powi(5),
            _ => 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn timing(stay_broken: bool) -> SurgeTiming {
        SurgeTiming { charge: 1.0, scatter: 1.0, hold: 1.0, reform: 1.0, stay_broken }
    }

    /// Run for `secs` at 60 fps; how many times it exploded.
    fn run(s: &mut Surge, t: &SurgeTiming, secs: f32) -> usize {
        (0..(secs * 60.0) as usize).filter(|_| s.advance(1.0 / 60.0, t)).count()
    }

    #[test]
    fn it_explodes_once_breaks_and_comes_back() {
        let (t, mut rng) = (timing(false), rand::make_rng());
        let mut s = Surge::new();
        assert!(s.start(&mut rng, 3));
        assert!(!s.start(&mut rng, 3), "a second Enter must not restart it");
        assert_eq!(run(&mut s, &t, 1.5), 1);
        assert!(s.form(0, &t).0 > 1.0, "layers fly outward after the explosion");
        run(&mut s, &t, 1.0);
        assert_eq!(s.form(0, &t).1, 0.0, "gone while held");
        run(&mut s, &t, 2.5);
        assert!(!s.busy());
        assert_eq!(s.form(0, &t), (1.0, 1.0), "back as it was");
    }

    #[test]
    fn stay_broken_means_it_never_comes_back() {
        let (t, mut rng) = (timing(true), rand::make_rng());
        let mut s = Surge::new();
        s.start(&mut rng, 3);
        run(&mut s, &t, 30.0);
        assert!(s.busy());
        assert_eq!(s.form(1, &t).1, 0.0);
    }
}
