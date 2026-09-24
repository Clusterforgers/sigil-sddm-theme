use super::pool::Effect;
use super::smoothstep;
use crate::figure::Rgb;
use crate::gpu::{srgb_to_linear, GpuWave};

/// Seconds a colour wave takes to roll from the centre out past the rim.
const LIFE: f32 = 4.0;

/// A wave of colour rolling out from the centre, or in from the rim. The gold takes on the
/// colour as the front passes and gets it back behind it, so the whole figure changes
/// colour one ring at a time.
pub struct ColorWave {
    age: f32,
    disc: f32,
    inward: bool,
    /// Linear light, as the shader mixes in.
    color: [f32; 3],
}

impl ColorWave {
    pub fn new(disc: f32, color: Rgb, inward: bool) -> Self {
        let [r, g, b] = color.0;
        ColorWave { age: 0.0, disc, inward, color: [srgb_to_linear(r), srgb_to_linear(g), srgb_to_linear(b)] }
    }

    pub fn gpu(&self) -> GpuWave {
        let u = (self.age / LIFE).min(1.0);
        // Outward: quick off the mark, easing as it spreads, the way a stain soaks out.
        // Inward: off the rim quickly, slowing as it gathers at the centre.
        let reach = if self.inward { (1.0 - u) * (1.0 - u) } else { 1.0 - (1.0 - u) * (1.0 - u) };
        let radius = self.disc * 1.25 * reach;
        let strength = smoothstep(0.0, 0.08, u) * (1.0 - smoothstep(0.7, 1.0, u));
        let [r, g, b] = self.color;
        GpuWave { front: [radius, self.disc * 0.22, strength, if self.inward { -1.0 } else { 1.0 }], color: [r, g, b, 0.0] }
    }
}

impl Effect for ColorWave {
    fn advance(&mut self, dt: f32) {
        self.age += dt;
    }

    fn done(&self) -> bool {
        self.age >= LIFE
    }
}
