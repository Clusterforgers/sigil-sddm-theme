//! Hough-style polygon fitting.
//!
//! A drawn polygon outline is bright exactly where the boundary lies, so we score a
//! hypothesis `(sides, circumradius, phase)` by the mean image brightness along its
//! outline and keep the local maxima. This beats fitting angular harmonics, which get
//! swamped by the lettering.

use crate::geom::Boundary;
use image::RgbImage;
use rayon::prelude::*;
use std::f32::consts::TAU;

fn luma(p: &[u8; 3]) -> f32 {
    0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32
}

fn sample(src: &RgbImage, x: f32, y: f32) -> f32 {
    let (w, h) = (src.width() as i32, src.height() as i32);
    let (xi, yi) = (x.round() as i32, y.round() as i32);
    if xi < 0 || yi < 0 || xi >= w || yi >= h {
        return 0.0;
    }
    luma(&src.get_pixel(xi as u32, yi as u32).0)
}

/// Mean brightness along a boundary outline.
pub fn score(src: &RgbImage, center: (f32, f32), b: Boundary) -> f32 {
    let steps = 4096;
    let (cx, cy) = center;
    let mut acc = 0.0;
    for i in 0..steps {
        let a = TAU * i as f32 / steps as f32;
        let r = b.radius_at(a);
        // Take the best of three radii so a 1px-off hypothesis still scores.
        let mut best: f32 = 0.0;
        for d in [-1.0f32, 0.0, 1.0] {
            let rr = r + d;
            best = best.max(sample(src, cx - rr * a.sin(), cy - rr * a.cos()));
        }
        acc += best;
    }
    acc / steps as f32
}

/// Scan `(radius, phase)` for an n-gon and report the strongest local maxima.
pub fn scan(src: &RgbImage, center: (f32, f32), sides: u32, rmin: f32, rmax: f32, top: usize) {
    let n = sides as f32;
    let phase_span = 360.0 / n;
    let phase_steps = 240usize;
    let radii: Vec<f32> = {
        let mut v = vec![];
        let mut r = rmin;
        while r <= rmax {
            v.push(r);
            r += 0.5;
        }
        v
    };

    // grid[ri][pi]
    let grid: Vec<Vec<f32>> = radii
        .par_iter()
        .map(|&r| {
            (0..phase_steps)
                .map(|pi| {
                    let ph = phase_span * pi as f32 / phase_steps as f32;
                    score(src, center, Boundary::Poly(n, r, ph))
                })
                .collect()
        })
        .collect();

    // Local maxima over the 2-D grid (phase wraps).
    let mut peaks: Vec<(f32, f32, f32)> = vec![];
    for (ri, row) in grid.iter().enumerate() {
        for pi in 0..phase_steps {
            let v = row[pi];
            let mut is_max = true;
            for dr in -4i32..=4 {
                for dp in -4i32..=4 {
                    let rj = ri as i32 + dr;
                    if rj < 0 || rj as usize >= grid.len() {
                        continue;
                    }
                    let pj = (pi as i32 + dp).rem_euclid(phase_steps as i32) as usize;
                    if grid[rj as usize][pj] > v {
                        is_max = false;
                    }
                }
            }
            if is_max {
                peaks.push((v, radii[ri], phase_span * pi as f32 / phase_steps as f32));
            }
        }
    }
    peaks.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());

    println!("\n=== {sides}-gon candidates (r {rmin}..{rmax}) ===");
    println!("{:>8}  {:>7}  {:>8}", "score", "radius", "phase");
    for (v, r, ph) in peaks.iter().take(top) {
        println!("{v:8.1}  {r:7.1}  {ph:8.2}");
    }
}

/// Print the outline-brightness of every layer boundary in a config.
///
/// A divider that sits in an empty gap scores near the background level; one that lies
/// on top of drawn artwork scores high and will tear when the layers counter-rotate.
pub fn check(src: &RgbImage, center: (f32, f32), layers: &[crate::geom::Layer], bg: f32) {
    println!("{:>22}  {:>10}  {:>10}", "layer", "outer", "inner");
    for l in layers {
        let so = score(src, center, l.outer);
        let si = if l.inner.max_radius() <= 0.0 { 0.0 } else { score(src, center, l.inner) };
        let tag = |s: f32| {
            if s < bg + 12.0 { "clean" } else if s < bg + 40.0 { "marginal" } else { "CUTS ART" }
        };
        println!(
            "{:>22}  {:6.1} {:<9} {:6.1} {:<9}",
            l.name, so, tag(so), si, tag(si)
        );
    }
}

/// Mean brightness of a thin annulus — used to locate empty radial gaps.
pub fn gap_profile(src: &RgbImage, center: (f32, f32), sides: f32, phase: f32, rmin: f32, rmax: f32) {
    println!("\n  r (as {sides}-gon circumradius, phase {phase})   outline brightness");
    let mut r = rmin;
    while r <= rmax {
        let s = score(src, center, Boundary::Poly(sides, r, phase));
        let bar: String = std::iter::repeat('#').take((s / 3.0) as usize).collect();
        println!("  {r:6.1}  {s:6.1} |{bar}");
        r += 1.0;
    }
}
