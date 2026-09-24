use super::pool::Effect;
use super::smoothstep;
use crate::gpu::GpuFlare;

/// The afterglow left behind where a bolt struck. Placed in un-rotated coordinates, so
/// it stays where it landed while the layers keep turning underneath it.
pub struct Flare {
    pub pos: [f32; 2],
    pub radius: f32,
    pub age: f32,
    pub life: f32,
    pub strength: f32,
}

impl Flare {
    /// Strike fast, ebb slowly — a discharge rather than a throb.
    fn amount(&self) -> f32 {
        let u = self.age / self.life.max(1e-3);
        self.strength * smoothstep(0.0, 0.12, u) * (1.0 - smoothstep(0.25, 1.0, u))
    }

    pub fn gpu(&self) -> GpuFlare {
        GpuFlare { at: [self.pos[0], self.pos[1], self.radius, self.amount()] }
    }
}

impl Effect for Flare {
    fn advance(&mut self, dt: f32) {
        self.age += dt;
    }

    fn done(&self) -> bool {
        self.age >= self.life
    }
}
