/// Seconds for a bloom to fade to ~2% of its peak.
const DECAY: f32 = 0.55;

/// Per-layer glow, 1.0 on a keypress, decaying toward 0.
pub struct Bloom {
    levels: Vec<f32>,
}

impl Bloom {
    pub fn new(layers: usize) -> Self {
        Bloom { levels: vec![0.0; layers] }
    }

    /// Light up layer `i`. Out-of-range layers are ignored, so any digit key is safe.
    pub fn light(&mut self, i: usize) {
        if let Some(b) = self.levels.get_mut(i) {
            *b = 1.0;
        }
    }

    pub fn light_all(&mut self) {
        self.levels.fill(1.0);
    }

    /// Exponential fade, so a bloom decays at the same rate whatever the framerate.
    pub fn fade(&mut self, dt: f32) {
        let k = (-dt / (DECAY / 4.0)).exp();
        for b in self.levels.iter_mut() {
            *b *= k;
            if *b < 0.002 {
                *b = 0.0;
            }
        }
    }

    pub fn level(&self, i: usize) -> f32 {
        self.levels.get(i).copied().unwrap_or(0.0)
    }

    pub fn peak(&self) -> f32 {
        self.levels.iter().copied().fold(0.0, f32::max)
    }
}
