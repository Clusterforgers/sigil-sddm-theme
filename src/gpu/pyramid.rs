use super::color::{decode, linear_to_srgb};

use image::RgbaImage;

/// A layer pixel as premultiplied linear light. tiny-skia premultiplies in sRGB space, so
/// the colour is divided back out, decoded, and premultiplied again.
fn linear(px: &image::Rgba<u8>) -> [f32; 4] {
    let a = px.0[3] as f32 / 255.0;
    if a == 0.0 {
        return [0.0; 4];
    }
    let c = |v: u8| decode((v as f32 / 255.0 / a).min(1.0)) * a;
    [c(px.0[0]), c(px.0[1]), c(px.0[2]), a]
}

/// How much light a pixel contributes to the bloom — the shader's old `highlight()`,
/// evaluated once on the CPU instead of 24 times per pixel per frame.
fn highlight(p: [f32; 4]) -> f32 {
    (0.299 * p[0] + 0.587 * p[1] + 0.114 * p[2] - 0.12 * p[3]).max(0.0)
}

/// Number of levels in the glow pyramid for a source of this size.
pub fn glow_levels(w: u32, h: u32) -> u32 {
    32 - w.max(h).max(1).leading_zeros()
}

/// Halve `level` (a `w`x`h` grid) until it is one texel, appending each level's encoding to
/// the output. Box filter, clamping at odd edges so no source pixel is dropped.
fn build_chain<T: Copy>(
    mut level: Vec<T>,
    (mut w, mut h): (u32, u32),
    average: impl Fn(T, T, T, T) -> T,
    mut encode: impl FnMut(&[T], &mut Vec<u8>),
) -> (Vec<u8>, u32) {
    let mut out = Vec::new();
    encode(&level, &mut out);
    let mut levels = 1u32;
    while w > 1 || h > 1 {
        let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
        let mut next = Vec::with_capacity((nw * nh) as usize);
        for y in 0..nh {
            for x in 0..nw {
                let (x0, y0) = (2 * x, 2 * y);
                let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
                let at = |px: u32, py: u32| level[(py * w + px) as usize];
                next.push(average(at(x0, y0), at(x1, y0), at(x0, y1), at(x1, y1)));
            }
        }
        encode(&next, &mut out);
        level = next;
        w = nw;
        h = nh;
        levels += 1;
    }
    (out, levels)
}

/// One layer and its mip chain, concatenated smallest-last: premultiplied, filtered in
/// linear light, stored sRGB-encoded for an `Rgba8UnormSrgb` texture.
///
/// Returns the packed levels and how many there are.
pub fn source_pyramid(layer: &RgbaImage) -> (Vec<u8>, u32) {
    build_chain(
        layer.pixels().map(linear).collect(),
        layer.dimensions(),
        |a, b, c, d| std::array::from_fn(|k| (a[k] + b[k] + c[k] + d[k]) * 0.25),
        |lv, out| {
            for p in lv {
                let a = (p[3] * 255.0).round() as u8;
                out.extend([linear_to_srgb(p[0]), linear_to_srgb(p[1]), linear_to_srgb(p[2]), a]);
            }
        },
    )
}

/// One layer's highlight map and its box-filtered mip chain, concatenated smallest-last.
///
/// Returns the packed levels and how many there are.
pub fn glow_pyramid(layer: &RgbaImage) -> (Vec<u8>, u32) {
    let light = layer.pixels().map(|p| (highlight(linear(p)) * 255.0).round() as u8).collect();
    build_chain(
        light,
        layer.dimensions(),
        |a, b, c, d| ((a as u32 + b as u32 + c as u32 + d as u32 + 2) / 4) as u8,
        |lv, out| out.extend_from_slice(lv),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Both chains end at one texel and hold exactly the bytes wgpu expects for a full mip
    /// chain of their format; a short buffer is a panic at upload, not a compile error.
    #[test]
    fn chains_are_complete() {
        let img = RgbaImage::from_pixel(13, 6, image::Rgba([200, 150, 40, 255]));
        let (rgba, n) = source_pyramid(&img);
        let (glow, m) = glow_pyramid(&img);
        assert_eq!(n, glow_levels(13, 6));
        assert_eq!(n, m);
        // 13x6, 6x3, 3x1, 1x1
        let texels = 13 * 6 + 6 * 3 + 3 + 1;
        assert_eq!(rgba.len(), texels * 4);
        assert_eq!(glow.len(), texels);
    }

    /// Premultiplied in, premultiplied out: a half-covered pixel keeps its colour, and
    /// a transparent one stays black rather than picking up a fringe.
    #[test]
    fn transparency_survives_the_round_trip() {
        let half = RgbaImage::from_pixel(1, 1, image::Rgba([100, 60, 10, 128]));
        let (px, _) = source_pyramid(&half);
        assert_eq!(px[3], 128);
        // What the GPU decodes is the colour's linear value times coverage.
        let want = decode(100.0 / 128.0) * (128.0 / 255.0);
        assert!((super::super::color::srgb_to_linear(px[0]) - want).abs() < 0.01, "{px:?}");
        let clear = RgbaImage::from_pixel(1, 1, image::Rgba([0, 0, 0, 0]));
        assert_eq!(source_pyramid(&clear).0, [0, 0, 0, 0]);
    }
}
