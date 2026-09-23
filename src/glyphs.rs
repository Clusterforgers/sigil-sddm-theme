//! Finds the individual letters and symbols in the artwork.
//!
//! The viewer wants to light one up and float it off the plate, which means knowing where
//! each one *is*. Nothing in the pipeline knew that — the source is a picture, and the
//! shader only ever asks it for the colour at a point. So this walks the image once at
//! startup and picks out the blobs of gold small enough to be a glyph rather than part of
//! the ring-and-polygon line work.
//!
//! The artwork makes this easy: about 88% of it is flat ground at luma 13 and the ink is
//! roughly 4% of the pixels, so a threshold separates them cleanly. What it cannot do is
//! separate a letter that happens to touch a cell divider — those merge into the line
//! network and are dropped by the size filter, which is the right failure: a glyph that
//! floats off with a piece of ruling attached would look like a mistake.

use image::RgbImage;

/// One letter or symbol, in source pixels.
#[derive(Debug, Clone, Copy)]
pub struct Glyph {
    pub pos: [f32; 2],
    /// Enough to cover the glyph, used both to light it and to bound the search.
    pub radius: f32,
}

/// Anything wider or taller than this is ruling, not a glyph.
const MAX_SPAN: u32 = 64;
/// ...and anything smaller is a speck of antialiasing or a stray dot.
const MIN_SPAN: u32 = 4;
const MIN_PIXELS: u32 = 14;

/// Every glyph in `src`, found by flood-filling the lit pixels.
///
/// `disc` bounds the search to the plate itself so the empty frame costs nothing.
pub fn find(src: &RgbImage, center: [f32; 2], disc: f32) -> Vec<Glyph> {
    let (w, h) = (src.width(), src.height());
    let lit = |x: u32, y: u32| -> bool {
        let p = src.get_pixel(x, y).0;
        // The ink is gold and the ground is near-black, so any channel will do; red is
        // the strongest of the three here.
        p[0] as u32 * 299 + p[1] as u32 * 587 + p[2] as u32 * 114 > 60_000
    };

    // Only look inside the plate.
    let r2 = disc * disc;
    let inside = |x: u32, y: u32| -> bool {
        let (dx, dy) = (x as f32 - center[0], y as f32 - center[1]);
        dx * dx + dy * dy <= r2
    };

    let mut seen = vec![false; (w * h) as usize];
    let mut out = Vec::new();
    let mut stack: Vec<(u32, u32)> = Vec::new();

    for y0 in 0..h {
        for x0 in 0..w {
            let i0 = (y0 * w + x0) as usize;
            if seen[i0] || !inside(x0, y0) || !lit(x0, y0) {
                continue;
            }

            // Flood the blob, tracking only what is needed to judge and place it.
            let (mut lo_x, mut hi_x, mut lo_y, mut hi_y) = (x0, x0, y0, y0);
            let (mut sx, mut sy, mut n) = (0u64, 0u64, 0u32);
            let mut oversized = false;

            seen[i0] = true;
            stack.push((x0, y0));
            while let Some((x, y)) = stack.pop() {
                lo_x = lo_x.min(x);
                hi_x = hi_x.max(x);
                lo_y = lo_y.min(y);
                hi_y = hi_y.max(y);
                sx += x as u64;
                sy += y as u64;
                n += 1;

                // The ruling is one enormous connected network. Once a blob is clearly
                // that, stop measuring it but keep flooding, or it gets rediscovered from
                // every other pixel in it.
                if hi_x - lo_x > MAX_SPAN * 4 || hi_y - lo_y > MAX_SPAN * 4 {
                    oversized = true;
                }

                for (dx, dy) in [
                    (-1i32, 0i32), (1, 0), (0, -1), (0, 1),
                    (-1, -1), (1, -1), (-1, 1), (1, 1),
                ] {
                    let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                    if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                        continue;
                    }
                    let (nx, ny) = (nx as u32, ny as u32);
                    let i = (ny * w + nx) as usize;
                    if !seen[i] && lit(nx, ny) {
                        seen[i] = true;
                        stack.push((nx, ny));
                    }
                }
            }

            if oversized {
                continue;
            }
            let (sw, sh) = (hi_x - lo_x + 1, hi_y - lo_y + 1);
            if sw > MAX_SPAN || sh > MAX_SPAN || sw < MIN_SPAN || sh < MIN_SPAN || n < MIN_PIXELS {
                continue;
            }
            out.push(Glyph {
                // Centroid rather than bbox centre: it sits where the ink actually is,
                // which matters for a glyph like `J` that is far from square.
                pos: [sx as f32 / n as f32, sy as f32 / n as f32],
                radius: 0.5 * (sw.max(sh) as f32) + 2.0,
            });
        }
    }
    out
}
