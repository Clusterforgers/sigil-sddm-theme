// The figure drawing itself in: lines traced by a glowing pen, letters flashing into place.

// The colour of the pen's tip, and of a letter in the instant it appears.
const PEN: vec3<f32> = vec3<f32>(1.0, 0.86, 0.55);

/// The moment, 0 to 1, that layer `k`'s ink at layer-frame point `q` appears.
///
/// Read exactly, with no filtering: the moment is sixteen bits split over two bytes, and
/// blending neighbouring texels would blend the bytes separately into nonsense.
fn reveal_at(q: vec2<f32>, k: u32) -> f32 {
    let size = vec2<f32>(textureDimensions(reveal_tex));
    let texel = vec2<i32>(clamp(art_uv(q) * size, vec2<f32>(0.0), size - 1.0));
    let rg = textureLoad(reveal_tex, texel, k, 0).rg;
    return (rg.r * 255.0 * 256.0 + rg.g * 255.0) / 65535.0;
}

/// Layer `k`'s `ink` at `q` as the build has left it: hidden until its moment, then shown
/// with a brief glow — the pen's tip along a line, a flash on a letter.
fn built(ink: vec4<f32>, q: vec2<f32>, k: u32) -> vec4<f32> {
    let progress = u.rhythm.w;
    if (progress >= 1.0 || ink.a <= 0.0) { return ink; }
    let since = progress - reveal_at(q, k);
    let shown = clamp(since / 0.004, 0.0, 1.0);
    let tip = shown * (1.0 - clamp(since / 0.035, 0.0, 1.0));
    return vec4<f32>(ink.rgb * shown + PEN * (tip * ink.a * 2.4), ink.a * shown);
}
