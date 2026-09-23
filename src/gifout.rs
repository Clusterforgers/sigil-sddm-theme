//! GIF output with a single global palette.
//!
//! Rotation only resamples colours that already exist in the source, so one palette
//! built from the source serves every frame. That avoids the frame-to-frame flicker
//! you get from per-frame quantisation, and shrinks the file.

use color_quant::NeuQuant;
use gif::{Encoder, Frame, Repeat};
use std::fs::File;
use std::io::BufWriter;

/// Quantised palette plus a 32^3 RGB→index lookup cube.
pub struct Palette {
    pub rgb: Vec<u8>,
    lut: Vec<u8>,
    pub transparent: u8,
}

impl Palette {
    pub fn build(src_rgb: &[u8], colors: usize, background: [u8; 3]) -> Self {
        // NeuQuant wants RGBA.
        let mut rgba = Vec::with_capacity(src_rgb.len() / 3 * 4);
        for px in src_rgb.chunks_exact(3) {
            rgba.extend_from_slice(&[px[0], px[1], px[2], 255]);
        }
        let nq = NeuQuant::new(10, colors.clamp(2, 256), &rgba);
        let mut rgb: Vec<u8> = nq
            .color_map_rgba()
            .chunks_exact(4)
            .flat_map(|c| [c[0], c[1], c[2]])
            .collect();

        // Guarantee the exact background colour is present, so the area outside the
        // disc is not dithered into a near-miss.
        let n = rgb.len() / 3;
        let has_bg = (0..n).any(|i| rgb[i * 3..i * 3 + 3] == background);
        if !has_bg {
            if n < 256 {
                rgb.extend_from_slice(&background);
            } else {
                rgb[0..3].copy_from_slice(&background);
            }
        }

        // Reserve one slot past the real colours as the transparency index used for
        // inter-frame delta encoding.
        let transparent = (rgb.len() / 3) as u8;

        // Precompute nearest-palette-index for every 5-bit RGB cell.
        let n = rgb.len() / 3;
        let mut lut = vec![0u8; 32 * 32 * 32];
        for r in 0..32usize {
            for g in 0..32usize {
                for b in 0..32usize {
                    let (tr, tg, tb) = ((r * 255 / 31) as i32, (g * 255 / 31) as i32, (b * 255 / 31) as i32);
                    let mut best = 0usize;
                    let mut best_d = i32::MAX;
                    for i in 0..n {
                        let dr = rgb[i * 3] as i32 - tr;
                        let dg = rgb[i * 3 + 1] as i32 - tg;
                        let db = rgb[i * 3 + 2] as i32 - tb;
                        let d = dr * dr + dg * dg + db * db;
                        if d < best_d {
                            best_d = d;
                            best = i;
                        }
                    }
                    lut[(r << 10) | (g << 5) | b] = best as u8;
                }
            }
        }
        Palette { rgb, lut, transparent }
    }

    #[inline]
    pub fn index(&self, p: &[u8]) -> u8 {
        let i = ((p[0] as usize >> 3) << 10) | ((p[1] as usize >> 3) << 5) | (p[2] as usize >> 3);
        self.lut[i]
    }

    /// Map an RGB buffer (stride `w`) to palette indices, cropped to a rect.
    pub fn map_rect(&self, rgb: &[u8], w: u32, rect: (u32, u32, u32, u32)) -> Vec<u8> {
        let (x0, y0, rw, rh) = rect;
        let mut out = Vec::with_capacity((rw * rh) as usize);
        for y in y0..y0 + rh {
            let row = ((y * w + x0) * 3) as usize;
            for x in 0..rw {
                out.push(self.index(&rgb[row + (x * 3) as usize..row + (x * 3) as usize + 3]));
            }
        }
        out
    }

    /// Squared RGB distance between two palette entries.
    fn dist2(&self, a: u8, b: u8) -> i32 {
        let (a, b) = (a as usize * 3, b as usize * 3);
        let d = |k: usize| self.rgb[a + k] as i32 - self.rgb[b + k] as i32;
        d(0) * d(0) + d(1) * d(1) + d(2) * d(2)
    }

    /// Delta-encode `cur` against what is currently on screen.
    ///
    /// The artwork is sparse gold lines on a flat ground, so most of the disc keeps
    /// essentially the same colour from frame to frame; letting those pixels fall through
    /// to the previous frame shrinks the file a lot. `tol` is a squared-RGB threshold —
    /// a pixel within it of the displayed colour is left alone.
    ///
    /// `shown` is updated to track what the viewer will actually see, so the error a
    /// pixel can accumulate stays bounded by `tol` no matter how many frames it skips.
    pub fn delta(&self, cur: &mut [u8], shown: &mut [u8], tol: i32) -> f32 {
        let mut changed = 0usize;
        for (c, s) in cur.iter_mut().zip(shown.iter_mut()) {
            if *c == *s || self.dist2(*c, *s) <= tol {
                *c = self.transparent;
            } else {
                *s = *c;
                changed += 1;
            }
        }
        changed as f32 / cur.len().max(1) as f32
    }
}

pub struct GifWriter {
    enc: Encoder<BufWriter<File>>,
    delay: u16,
}

impl GifWriter {
    pub fn new(path: &str, w: u16, h: u16, pal: &Palette, fps: u32) -> Result<Self, gif::EncodingError> {
        let mut rgb = pal.rgb.clone();
        // Must be long enough to cover the reserved transparency index, and GIF global
        // palettes must have a power-of-two length.
        let want = (pal.transparent as usize + 1).next_power_of_two().max(2);
        rgb.resize(want * 3, 0);
        let file = BufWriter::new(File::create(path)?);
        let mut enc = Encoder::new(file, w, h, &rgb)?;
        enc.set_repeat(Repeat::Infinite)?;
        // GIF delay is in hundredths of a second.
        let delay = ((100.0 / fps as f64).round() as u16).max(2);
        Ok(GifWriter { enc, delay })
    }

    pub fn write(
        &mut self,
        indices: Vec<u8>,
        rect: (u32, u32, u32, u32),
        transparent: Option<u8>,
    ) -> Result<(), gif::EncodingError> {
        let (x, y, w, h) = rect;
        let mut f = Frame::default();
        f.left = x as u16;
        f.top = y as u16;
        f.width = w as u16;
        f.height = h as u16;
        f.delay = self.delay;
        f.transparent = transparent;
        // Keep, so transparent pixels reveal the frame underneath rather than the
        // background colour.
        f.dispose = gif::DisposalMethod::Keep;
        f.buffer = indices.into();
        self.enc.write_frame(&f)?;
        Ok(())
    }
}
