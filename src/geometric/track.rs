use super::clockwise;

use serde::Deserialize;
use std::f32::consts::{PI, TAU};

/// A closed curve around the centre that text and spokes are laid along.
///
/// Written `{ circle: 262 }` or `{ polygon: { sides: 7, r: 318, rotate: 0 } }`.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase", deny_unknown_fields)]
pub enum Track {
    Circle(f32),
    Polygon(Polygon),
}

/// A regular polygon centred on the figure.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Polygon {
    pub sides: u32,
    /// Circumradius: centre to vertex.
    pub r: f32,
    /// Degrees clockwise from North of one vertex; 0 puts a vertex at 12 o'clock.
    #[serde(default)]
    pub rotate: f32,
}

impl Polygon {
    /// Centre to the middle of an edge.
    pub fn apothem(&self) -> f32 {
        self.r * (PI / self.sides as f32).cos()
    }
}

impl Track {
    /// Distance from the centre to the track along internal angle `alpha`.
    pub fn radius_at(&self, alpha: f32) -> f32 {
        match *self {
            Track::Circle(r) => r,
            Track::Polygon(p) => {
                let step = TAU / p.sides as f32;
                // Offset from the nearest edge's midpoint, which sits half a step past a
                // vertex, folded into [-step/2, step/2].
                let a = (alpha - clockwise(p.rotate)).rem_euclid(step) - step * 0.5;
                p.apothem() / a.cos()
            }
        }
    }

    /// The same track moved `d` outward, measured square to the curve.
    ///
    /// Offsetting every edge of a polygon outward grows its apothem by the same amount, so
    /// the result is a polygon again and anything laid along it keeps a constant clearance.
    pub fn offset(&self, d: f32) -> Track {
        match *self {
            Track::Circle(r) => Track::Circle(r + d),
            Track::Polygon(p) => Track::Polygon(Polygon {
                r: p.r + d / (PI / p.sides as f32).cos(),
                ..p
            }),
        }
    }

    pub fn check(&self) -> Result<(), String> {
        match *self {
            Track::Circle(r) if r <= 0.0 => Err(format!("circle radius must be > 0, got {r}")),
            Track::Polygon(p) if p.sides < 3 => Err(format!("a polygon needs at least 3 sides, got {}", p.sides)),
            Track::Polygon(p) if p.r <= 0.0 => Err(format!("polygon radius must be > 0, got {}", p.r)),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEPTAGON: Track = Track::Polygon(Polygon { sides: 7, r: 200.0, rotate: 0.0 });

    #[test]
    fn rotate_zero_puts_a_vertex_at_north() {
        assert!((HEPTAGON.radius_at(0.0) - 200.0).abs() < 1e-3);
        // Half a step round is the middle of an edge.
        let Track::Polygon(p) = HEPTAGON else { unreachable!() };
        assert!((HEPTAGON.radius_at(clockwise(180.0 / 7.0)) - p.apothem()).abs() < 1e-3);
    }

    #[test]
    fn offset_keeps_a_constant_clearance() {
        // At an edge's midpoint the edge's normal is the radius, so the gap is exactly 10.
        let mid = clockwise(180.0 / 7.0);
        let gap = HEPTAGON.offset(10.0).radius_at(mid) - HEPTAGON.radius_at(mid);
        assert!((gap - 10.0).abs() < 1e-3, "gap {gap}");
    }
}
