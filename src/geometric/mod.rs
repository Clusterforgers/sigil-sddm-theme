//! Plane geometry around the figure's centre.
//!
//! Figure files give angles in degrees clockwise from North, like a clock. Internally an
//! angle `alpha` is radians counter-clockwise from North, which is what `shapes::pt` and
//! the shader use; `clockwise` and `slot` are the only places the two meet.

pub mod shapes;
mod track;

pub use track::{Polygon, Track};

use std::f32::consts::TAU;

/// The internal angle for `deg` degrees clockwise from North.
pub fn clockwise(deg: f32) -> f32 {
    -deg.to_radians()
}

/// The internal angle of the `i`th of `n` evenly spaced positions, running clockwise from
/// `rotate` degrees.
pub fn slot(i: usize, n: usize, rotate: f32) -> f32 {
    clockwise(rotate) - TAU * i as f32 / n as f32
}
