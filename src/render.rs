use crate::geom::Layer;
use image::RgbImage;
use rayon::prelude::*;
use std::f32::consts::TAU;

/// One distinct colour per layer, outermost first, for telling which ring turns with
/// which. `layer_tint` in `spin.wgsl` is a hand-kept copy; change one and change the other.
pub const LAYER_TINT: [[f32; 3]; 7] = [
    [1.00, 0.28, 0.28], // red
    [1.00, 0.62, 0.16], // orange
    [0.95, 0.95, 0.22], // yellow
    [0.32, 0.95, 0.38], // green
    [0.26, 0.90, 0.96], // cyan
    [0.48, 0.58, 1.00], // blue
    [0.96, 0.42, 0.96], // magenta
];

pub struct Renderer {
    pub src: RgbImage,
    pub center: (f32, f32),
    pub out_w: u32,
    pub out_h: u32,
    pub scale: f32,
    pub background: [u8; 3],
    /// Sorted outermost-first.
    pub layers: Vec<Layer>,
    pub ss: u32,
    /// Paint each layer in its own colour instead of the artwork's.
    pub tint: bool,
}

impl Renderer {
    pub fn new(
        src: RgbImage,
        center: (f32, f32),
        out_w: u32,
        out_h: u32,
        background: [u8; 3],
        mut layers: Vec<Layer>,
        ss: u32,
    ) -> Self {
        layers.sort_by(|a, b| {
            b.outer
                .max_radius()
                .partial_cmp(&a.outer.max_radius())
                .unwrap()
        });
        let scale = out_w as f32 / src.width() as f32;
        Self { src, center, out_w, out_h, scale, background, layers, ss, tint: false }
    }

    /// Bilinear sample of the source, or `None` outside its bounds.
    fn sample(&self, x: f32, y: f32) -> Option<[f32; 3]> {
        let (w, h) = (self.src.width() as i32, self.src.height() as i32);
        let (fx, fy) = (x - 0.5, y - 0.5);
        let (x0, y0) = (fx.floor() as i32, fy.floor() as i32);
        let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
        if x0 < -1 || y0 < -1 || x0 >= w || y0 >= h {
            return None;
        }
        let mut acc = [0.0f32; 3];
        for (dy, wy) in [(0, 1.0 - ty), (1, ty)] {
            for (dx, wx) in [(0, 1.0 - tx), (1, tx)] {
                let wgt = wx * wy;
                if wgt == 0.0 {
                    continue;
                }
                let (px, py) = ((x0 + dx).clamp(0, w - 1), (y0 + dy).clamp(0, h - 1));
                let p = self.src.get_pixel(px as u32, py as u32).0;
                for c in 0..3 {
                    acc[c] += wgt * p[c] as f32;
                }
            }
        }
        Some(acc)
    }

    /// Colour of one sub-sample, in source coordinates, with per-layer angles `thetas`.
    fn shade(&self, sx: f32, sy: f32, thetas: &[f32]) -> [f32; 3] {
        let (cx, cy) = self.center;
        let (dx, dy) = (sx - cx, sy - cy);
        let r = (dx * dx + dy * dy).sqrt();
        // alpha measured from North, CCW: dx = -r sin a, dy = -r cos a
        let alpha = (-dx).atan2(-dy);

        for (i, (layer, &theta)) in self.layers.iter().zip(thetas).enumerate() {
            let a = alpha - theta;
            if layer.contains(r, a) {
                let (px, py) = (cx - r * a.sin(), cy - r * a.cos());
                if let Some(c) = self.sample(px, py) {
                    if !self.tint {
                        return c;
                    }
                    // Only the hue is replaced, so the lettering stays legible.
                    let lum = 0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2];
                    let t = LAYER_TINT[i.min(LAYER_TINT.len() - 1)];
                    return [
                        (t[0] * lum * 1.35).min(255.0),
                        (t[1] * lum * 1.35).min(255.0),
                        (t[2] * lum * 1.35).min(255.0),
                    ];
                }
                break;
            }
        }
        [
            self.background[0] as f32,
            self.background[1] as f32,
            self.background[2] as f32,
        ]
    }

    /// Render frame `i` of `n`. Returns a tightly packed RGB buffer.
    pub fn frame(&self, i: u32, n: u32) -> Vec<u8> {
        let thetas: Vec<f32> = self
            .layers
            .iter()
            .map(|l| {
                // Reduce the revolution count modulo n in integer arithmetic, so frame n
                // lands on exactly 0.0 rather than TAU*turns. Without this the loop point
                // differs from frame 0 by a little float rounding.
                let k = (l.turns as i64 * i as i64).rem_euclid(n as i64);
                TAU * k as f32 / n as f32
            })
            .collect();

        let ss = self.ss.max(1);
        let inv_n = 1.0 / (ss * ss) as f32;
        let (w, h) = (self.out_w, self.out_h);
        let mut buf = vec![0u8; (w * h * 3) as usize];

        buf.par_chunks_mut((w * 3) as usize)
            .enumerate()
            .for_each(|(oy, row)| {
                for ox in 0..w {
                    let mut acc = [0.0f32; 3];
                    for sj in 0..ss {
                        for si in 0..ss {
                            let px = ox as f32 + (si as f32 + 0.5) / ss as f32;
                            let py = oy as f32 + (sj as f32 + 0.5) / ss as f32;
                            let c = self.shade(px / self.scale, py / self.scale, &thetas);
                            for k in 0..3 {
                                acc[k] += c[k];
                            }
                        }
                    }
                    for k in 0..3 {
                        row[(ox * 3) as usize + k] = (acc[k] * inv_n).round().clamp(0.0, 255.0) as u8;
                    }
                }
            });
        buf
    }

    /// Bounding box in output pixels that every layer stays inside, padded a little.
    pub fn disc_rect(&self) -> (u32, u32, u32, u32) {
        let rmax = self
            .layers
            .iter()
            .map(|l| l.outer.max_radius())
            .fold(0.0f32, f32::max);
        let r = rmax * self.scale + 2.0;
        let (cx, cy) = (self.center.0 * self.scale, self.center.1 * self.scale);
        let x0 = (cx - r).floor().max(0.0) as u32;
        let y0 = (cy - r).floor().max(0.0) as u32;
        let x1 = (cx + r).ceil().min(self.out_w as f32) as u32;
        let y1 = (cy + r).ceil().min(self.out_h as f32) as u32;
        (x0, y0, x1 - x0, y1 - y0)
    }
}
