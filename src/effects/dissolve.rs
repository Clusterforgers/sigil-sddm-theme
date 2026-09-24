use super::pool::Effect;

/// One layer burning away to embers along a ragged edge, and growing back.
pub struct Dissolve {
    pub layer: usize,
    age: f32,
    life: f32,
}

impl Dissolve {
    pub fn new(layer: usize, life: f32) -> Self {
        Dissolve { layer, age: 0.0, life }
    }

    /// How much of the layer is gone, 0 to 1. Never quite all of it: a few scraps are
    /// left to grow back from, which is what makes it read as the same thing re-forming
    /// rather than a new one fading in.
    pub fn amount(&self) -> f32 {
        let u = (self.age / self.life).clamp(0.0, 1.0);
        // Clamped: at the very end sin(pi) rounds to a hair below zero, and a negative
        // number to a fractional power is NaN — which the shader would draw as garbage.
        0.72 * (u * std::f32::consts::PI).sin().max(0.0).powf(0.8)
    }
}

impl Effect for Dissolve {
    fn advance(&mut self, dt: f32) {
        self.age += dt;
    }

    fn done(&self) -> bool {
        self.age >= self.life
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Starts and ends whole, and never burns quite everything.
    #[test]
    fn it_burns_away_and_grows_back() {
        let mut d = Dissolve::new(0, 2.0);
        assert_eq!(d.amount(), 0.0);
        d.advance(1.0);
        assert!(d.amount() > 0.6 && d.amount() < 1.0);
        d.advance(1.0);
        assert!(d.amount() < 1e-3 && d.done());
    }
}
