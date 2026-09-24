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
#[cfg(test)]
mod tests {
    use super::*;

    /// Scratch: every ink blob in the source, as polar coordinates, sorted by radius.
    ///
    /// The plate's ruling is one connected network, so anything touching it is swallowed
    /// by the size filter. Painting the drawing's own lines out of the scan first — they
    /// sit on the scan's to within a pixel — leaves the crosses standing alone.
    #[test]
    #[ignore]
    fn dump_blobs() {
        let cfg: crate::config::Config =
            serde_json::from_str(include_str!("../layers.json")).unwrap();
        let mut src = image::open("images/1.png").unwrap().to_rgb8();
        let c = cfg.center;

        // Lines only: same figure, every list of lettering and crosses emptied.
        let bare = crate::sigil::Figure {
            cell_text: vec![],
            letter_text: vec![],
            names: (vec![], 0.0, 0.0, 0.0),
            lens_crosses: (vec![], 0.0, 0.0),
            spirit_rows: vec![],
            spirit_names: vec![],
            crosses: vec![],
            hairline: 6.0,
            ..crate::sigil::Figure::default()
        };
        let lines = crate::sigil::draw(&cfg, &bare, 1.0);
        for (x, y, p) in src.enumerate_pixels_mut() {
            let l = lines.get_pixel(x, y).0;
            if l[0] as u32 + l[1] as u32 + l[2] as u32 > 200 {
                *p = image::Rgb([0, 0, 0]);
            }
        }

        let mut g = find(&src, c, 410.0);
        g.sort_by(|a, b| {
            let ra = (a.pos[0] - c[0]).hypot(a.pos[1] - c[1]);
            let rb = (b.pos[0] - c[0]).hypot(b.pos[1] - c[1]);
            ra.partial_cmp(&rb).unwrap()
        });
        // ...and what the drawing has so far, so a blob can say whether it is still missing.
        let drawn = crate::sigil::draw(&cfg, &crate::sigil::Figure::default(), 1.0);
        let lit = |img: &image::RgbImage, x: f32, y: f32, rad: f32| -> bool {
            let k = rad.ceil() as i32;
            let (w, h) = (img.width() as i32, img.height() as i32);
            for dy in -k..=k {
                for dx in -k..=k {
                    let (px, py) = (x as i32 + dx, y as i32 + dy);
                    if px < 0 || py < 0 || px >= w || py >= h {
                        continue;
                    }
                    let p = img.get_pixel(px as u32, py as u32).0;
                    if p[0] as u32 + p[1] as u32 + p[2] as u32 > 200 {
                        return true;
                    }
                }
            }
            false
        };

        println!("# {} blobs", g.len());
        for b in &g {
            let (dx, dy) = (b.pos[0] - c[0], b.pos[1] - c[1]);
            let r = dx.hypot(dy);
            let a = (-dx).atan2(-dy).to_degrees();
            let a = if a < 0.0 { a + 360.0 } else { a };
            let have = lit(&drawn, b.pos[0], b.pos[1], b.radius * 0.7);
            println!("{r:7.2} {a:7.2} {:5.1} {}", b.radius, if have { "have" } else { "MISSING" });
        }
    }
}
