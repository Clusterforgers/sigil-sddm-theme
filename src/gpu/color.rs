/// The sRGB electro-optical transfer function, matching what the GPU applies when it
/// samples an `Rgba8UnormSrgb` texture.
pub fn srgb_to_linear(v: u8) -> f32 {
    decode(v as f32 / 255.0)
}

/// `srgb_to_linear` for an encoded value already in 0..1.
pub fn decode(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// The inverse of `srgb_to_linear`, for writing filtered values back into an sRGB texture.
pub fn linear_to_srgb(v: f32) -> u8 {
    let c = v.clamp(0.0, 1.0);
    let s = if c <= 0.003_130_8 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
    (s * 255.0).round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn srgb_round_trips() {
        for v in 0..=255u8 {
            assert_eq!(linear_to_srgb(srgb_to_linear(v)), v);
        }
    }
}
