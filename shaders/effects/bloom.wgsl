// Light spilling off the gold strokes: the per-layer keypress bloom, and the glow every
// other effect lights the artwork with.

// Colour the gold strokes glow with. Light spilling off the artwork, not new detail.
const GLOW: vec3<f32> = vec3<f32>(1.0, 0.74, 0.30);

/// Gather layer `k`'s highlights around `q` so bright strokes spill light into the dark ground.
///
/// Only the *amount* of nearby light is used, tinted with `tint` — sampling the colours
/// directly would drag neighbouring artwork in as ghost detail. The keypress
/// bloom and the red energy both come through here, with their tints blended by strength,
/// so lighting up for two reasons at once still costs one set of taps.
///
/// The gather itself is precomputed: level L of the glow pyramid already holds the mean
/// highlight over a 2^L box, so picking the level from the spill radius reads the same
/// neighbourhood average two rings of taps used to compute from scratch. Four taps on a
/// diagonal cross soften the box into something rounder, which is what the rotated
/// second ring used to be for. The level, spread and gain are fitted to the old kernel:
/// the pictures differ by ~1% RMSE, which is the disc-vs-ring shape and nothing else.
fn spill(q: vec2<f32>, k: u32, amount: f32, tint: vec3<f32>) -> vec3<f32> {
    let radius = 1.5 + 5.5 * min(amount, 1.0);
    let lod = clamp(log2(radius * 0.5), 0.0, u.quality.z);
    let uv = art_uv(q);
    let o = (radius * 0.35) / u.frame.z;
    var acc = textureSampleLevel(glow_tex, art_smp, uv + vec2<f32>( o,  o), k, lod).r;
    acc = acc + textureSampleLevel(glow_tex, art_smp, uv + vec2<f32>(-o,  o), k, lod).r;
    acc = acc + textureSampleLevel(glow_tex, art_smp, uv + vec2<f32>( o, -o), k, lod).r;
    acc = acc + textureSampleLevel(glow_tex, art_smp, uv + vec2<f32>(-o, -o), k, lod).r;
    return tint * (acc * 0.25) * amount * 1.7;
}
