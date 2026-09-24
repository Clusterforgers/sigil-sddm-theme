use super::pool::Effect;
use super::smoothstep;
use crate::gpu::GpuPulse;

/// What kind of disturbance to start.
#[derive(Clone, Copy)]
pub enum Drop {
    /// The ambient one: something falls in the middle.
    Droplet,
    /// A far heavier drop, fired from the keyboard.
    Heavy,
    /// A ring that starts at the rim and closes on the centre, gathering as it goes.
    Implosion,
    /// What an implosion turns into when it lands, carrying its gathered strength.
    Rebound(f32),
}

/// A drop landing in something thick, or the same thing running in reverse.
///
/// Carries the splash at the point of impact as well as the ring, because the two are
/// one event: the crest is what the splash turns into.
pub struct Pulse {
    age: f32,
    /// Crest radius in canvas units, and how fast it is still travelling.
    radius: f32,
    speed: f32,
    /// Envelope width and ripple wavelength, in canvas units.
    width: f32,
    wavelength: f32,
    strength: f32,
    /// The impact flash at the centre: how bright it still is, and how wide.
    splash: f32,
    splash_radius: f32,
    /// Converging on the centre rather than leaving it.
    imploding: bool,
    /// Radius of the disc it runs across.
    disc: f32,
}

impl Pulse {
    /// A new disturbance on a disc of radius `d`. The four kinds differ only in how hard
    /// they hit and which way the ring runs.
    pub fn new(drop: Drop, d: f32) -> Self {
        let base = Pulse {
            age: 0.0,
            radius: 0.0,
            speed: d * 0.92,
            width: d * 0.09,
            wavelength: d * 0.21,
            strength: 0.95,
            splash: 1.2,
            splash_radius: d * 0.16,
            imploding: false,
            disc: d,
        };
        match drop {
            Drop::Droplet => base,
            Drop::Heavy => Pulse {
                speed: d * 0.72,
                width: d * 0.16,
                wavelength: d * 0.30,
                strength: 1.8,
                splash: 2.4,
                splash_radius: d * 0.28,
                ..base
            },
            // Starts at the rim with nothing at the centre: there has been no impact
            // yet, and the whole point is that it is on its way to one.
            Drop::Implosion => Pulse {
                radius: d * 1.02,
                speed: d * 0.45,
                width: d * 0.10,
                wavelength: d * 0.22,
                strength: 0.85,
                splash: 0.0,
                imploding: true,
                ..base
            },
            // Everything the implosion gathered, coming back out.
            Drop::Rebound(gathered) => Pulse {
                speed: d * 1.10,
                width: d * 0.14,
                wavelength: d * 0.24,
                strength: gathered * 2.1,
                splash: gathered * 3.4,
                splash_radius: d * 0.24,
                ..base
            },
        }
    }

    /// An implosion that has reached the middle. It does not just stop; it lands.
    pub fn landed(&self) -> bool {
        self.imploding && self.radius <= self.disc * 0.05
    }

    /// How much it is still carrying — what a landed implosion throws back out.
    pub fn gathered(&self) -> f32 {
        self.strength * (-1.05 * self.age).exp()
    }

    /// Crest strength.
    ///
    /// A ring spreads a fixed amount of energy over a growing circumference, so amplitude
    /// goes as `1/sqrt(r)` — which is also why an implosion gets brighter the further in
    /// it gets, rather than merely surviving. Viscosity damps the whole thing on top of
    /// that, and the rim fade keeps a wave from ending abruptly against the outer ring.
    fn amount(&self) -> f32 {
        let d = self.disc;
        let r = self.radius.max(d * 0.06);
        let spread = (d * 0.34 / r).sqrt().clamp(0.4, 2.8);
        let damp = (-1.05 * self.age).exp();
        self.strength * spread * damp * (1.0 - smoothstep(0.88, 1.10, self.radius / d))
    }

    pub fn gpu(&self) -> GpuPulse {
        // A converging ring leaves its wake behind it, which is outward.
        let wake = if self.imploding { -1.0 } else { 1.0 };
        GpuPulse {
            wave: [self.radius, self.amount(), self.width, self.wavelength],
            splash: [self.splash, self.splash_radius, wake, 0.0],
        }
    }
}

impl Effect for Pulse {
    fn advance(&mut self, dt: f32) {
        self.age += dt;
        if self.imploding {
            // Converging: the same energy crowds into an ever shorter circumference, so it
            // runs in faster the closer it gets.
            self.radius -= self.speed * dt;
            self.speed *= 1.0 + 1.5 * dt;
        } else {
            self.radius += self.speed * dt;
            // Thick liquid drags the crest down as it spreads.
            self.speed *= (-1.35 * dt).exp();
        }
        // However the ring behaves, the impact flash is brief.
        self.splash *= (-9.0 * dt).exp();
    }

    fn done(&self) -> bool {
        self.landed() || self.radius - self.width * 6.0 >= self.disc || self.age >= 12.0
    }
}
