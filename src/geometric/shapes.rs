use super::{slot, Track};

use std::f32::consts::PI;
use tiny_skia::{Path, PathBuilder};

/// A point at `(r, alpha)` around the centre `c`, alpha internal (radians, counter-clockwise
/// from North).
pub fn pt(c: [f32; 2], r: f32, alpha: f32) -> (f32, f32) {
    (c[0] - r * alpha.sin(), c[1] - r * alpha.cos())
}

/// The vertices of a regular `n`-gon, clockwise from the one at `rotate` degrees.
pub fn polygon(c: [f32; 2], n: usize, r: f32, rotate: f32) -> Vec<(f32, f32)> {
    (0..n).map(|i| pt(c, r, slot(i, n, rotate))).collect()
}

/// The vertices of an `{n/skip}` star polygon, in the order they are joined.
///
/// Stepping `skip` at a time closes only after all `n` are visited when the two are
/// coprime, which makes one continuous star rather than a ring of triangles.
pub fn star(c: [f32; 2], n: usize, r: f32, rotate: f32, skip: usize) -> Vec<(f32, f32)> {
    let v = polygon(c, n, r, rotate);
    (0..n).map(|i| v[(i * skip) % n]).collect()
}

/// The circumradius of the star sitting `width` inside an `{n/skip}` star of circumradius
/// `r`, perpendicular to the chords.
///
/// Offsetting every chord inwards scales the star, so the result is a star again and the
/// ribbon between the two has parallel sides.
pub fn star_inside(r: f32, skip: usize, n: usize, width: f32) -> f32 {
    r - width / (PI * skip as f32 / n as f32).cos()
}

/// Where a symbol goes in the lens between the circle `roof` and the track `floor` below
/// it, and how big it can be there: offset from the centre, then size.
///
/// The lens closes to nothing where the floor rises to meet the roof and opens to its
/// deepest where it falls away, so a symbol set at one size would either burst the narrow
/// end or rattle around in the wide one. It hangs from the roof either way.
pub fn lens_fit(floor: &Track, roof: f32, biggest: f32, alpha: f32) -> (f32, f32, f32) {
    // 1.6, not 1.0: a symbol may spill well past the floor rather than shrink to nothing,
    // and only the very tips of the lens really pinch.
    let size = biggest.min((roof - floor.radius_at(alpha)) * 1.6).max(1.0);
    let r = roof - size * 0.5 - 1.5;
    (-r * alpha.sin(), -r * alpha.cos(), size)
}

/// A closed polygon through `pts`, in order.
pub fn closed_path(pts: &[(f32, f32)]) -> Option<Path> {
    let (first, rest) = pts.split_first()?;
    let mut pb = PathBuilder::new();
    pb.move_to(first.0, first.1);
    for p in rest {
        pb.line_to(p.0, p.1);
    }
    pb.close();
    pb.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometric::Polygon;

    /// A drawn polygon and the track of the same name agree at every vertex, which is what
    /// keeps text laid along a track sitting on the lines drawn for it.
    #[test]
    fn drawn_vertices_sit_on_the_track() {
        let track = Track::Polygon(Polygon { sides: 7, r: 314.0, rotate: 12.0 });
        for (x, y) in polygon([0.0, 0.0], 7, 314.0, 12.0) {
            let radius = (x * x + y * y).sqrt();
            let alpha = (-x).atan2(-y);
            assert!((radius - track.radius_at(alpha)).abs() < 0.05);
        }
    }

    #[test]
    fn stars_visit_every_vertex_once() {
        let mut s = star([0.0, 0.0], 7, 1.0, 0.0, 3);
        s.sort_by(|a, b| a.0.total_cmp(&b.0));
        s.dedup_by(|a, b| (a.0 - b.0).abs() < 1e-6 && (a.1 - b.1).abs() < 1e-6);
        assert_eq!(s.len(), 7);
    }
}
