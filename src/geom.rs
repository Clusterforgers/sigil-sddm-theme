use serde::Deserialize;
use std::f32::consts::{PI, TAU};

/// One edge of a layer: either a circle or a regular polygon centred on the disc.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Boundary {
    /// Radius in source pixels.
    Circle(f32),
    /// `[sides, circumradius, phase_deg]` — a vertex sits at `phase_deg` from North.
    Poly(f32, f32, f32),
    /// `[points, r_outer, r_inner, phase_deg]` — an n-pointed star. Points sit at
    /// `r_outer`, the notches between them at `r_inner`.
    Star(f32, f32, f32, f32),
}

impl Boundary {
    /// Distance from the centre to this boundary along `alpha`.
    pub fn radius_at(self, alpha: f32) -> f32 {
        match self {
            Boundary::Circle(r) => r,
            Boundary::Poly(n, r, phase) => {
                if r <= 0.0 {
                    return 0.0;
                }
                let step = TAU / n;
                // Distance from the nearest vertex direction, folded into [-step/2, step/2].
                let a = (alpha - phase.to_radians() + step * 0.5).rem_euclid(step) - step * 0.5;
                // Apothem over cos gives the distance to the flat edge.
                r * (PI / n).cos() / a.cos()
            }
            Boundary::Star(n, ro, ri, phase) => {
                if ro <= 0.0 {
                    return 0.0;
                }
                let step = TAU / n;
                let half = step * 0.5;
                // Fold to one half-point: a in [0, step/2], 0 at a tip.
                let a = ((alpha - phase.to_radians() + half).rem_euclid(step) - half).abs();
                // Straight edge from tip (ro, 0) to notch (ri, half), in polar form.
                let num = ro * ri * (half).sin();
                let den = ro * (half - a).sin() + ri * a.sin();
                if den.abs() < 1e-6 {
                    ro
                } else {
                    num / den
                }
            }
        }
    }

    /// Smallest radius this boundary ever reaches.
    ///
    /// With `max_radius` this brackets the boundary over every angle. The viewer tests a
    /// pixel's radius against the bracket first and only evaluates `radius_at` when that
    /// leaves the answer open, which saves the trigonometry on most pixels.
    pub fn min_radius(self) -> f32 {
        match self {
            Boundary::Circle(r) => r,
            // The apothem: where a flat edge comes closest to the centre.
            Boundary::Poly(n, r, _) => {
                if r <= 0.0 {
                    0.0
                } else {
                    r * (PI / n).cos()
                }
            }
            // The notches between the points.
            Boundary::Star(_, ro, ri, _) => if ro <= 0.0 { 0.0 } else { ri },
        }
    }

    /// Largest radius this boundary ever reaches — used for bounding boxes.
    pub fn max_radius(self) -> f32 {
        match self {
            Boundary::Circle(r) => r,
            Boundary::Poly(_, r, _) => r,
            Boundary::Star(_, ro, _, _) => ro,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Layer {
    #[serde(default)]
    pub name: String,
    pub outer: Boundary,
    pub inner: Boundary,
    /// Signed whole revolutions completed over one loop. Integer ⇒ seamless loop.
    pub turns: i32,
}

impl Layer {
    /// Is `(r, alpha)` inside this layer's region?
    pub fn contains(&self, r: f32, alpha: f32) -> bool {
        r >= self.inner.radius_at(alpha) && r < self.outer.radius_at(alpha)
    }
}
