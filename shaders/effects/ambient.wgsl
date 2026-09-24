// Always-on life in the ink: light gliding over the gilt, and the ink wavering like heat
// haze.

// The colour the gilt catches the light in: paler and whiter than the gold itself.
const SHEEN: vec3<f32> = vec3<f32>(1.0, 0.92, 0.72);

/// How much the gliding light catches the gilt at canvas point `p`, 0..1.
///
/// Worked out in the un-turned frame, so the light stays put while the layers turn under
/// it — which is what makes it read as light on metal, not paint moving with the ink. Two
/// bands opposite each other sweep round over time, twisted into a slow spiral by the
/// radius so they cross the rings at an angle rather than square on.
fn sheen_at(p: vec2<f32>) -> f32 {
    let strength = u.ambient.x;
    if (strength <= 0.0) { return 0.0; }
    let d = p - u.center;
    let alpha = atan2(-d.x, -d.y);
    let phase = u.look.w * u.ambient.y * TAU;
    let band = 0.5 + 0.5 * cos(2.0 * (alpha + phase) + length(d) * 0.012);
    return strength * pow(band, 18.0);
}

/// How far the ink at layer-frame point `q` of layer `k` is pushed by the heat haze, up to
/// `ambient.z` canvas units. Two drifting noise fields, offset per layer so neighbouring
/// rings do not waver in step.
fn haze_at(q: vec2<f32>, k: u32) -> vec2<f32> {
    let amount = u.ambient.z;
    if (amount <= 0.0) { return vec2<f32>(0.0); }
    let t = u.look.w;
    let o = f32(k) * 1.7;
    let n = noise2(q * 0.035 + vec2<f32>(t * 0.4 + o, -t * 0.3));
    let m = noise2(q * 0.035 + vec2<f32>(-t * 0.35, t * 0.45 + o) + 31.7);
    return (vec2<f32>(n, m) - 0.5) * 2.0 * amount;
}

/// Layer `k`'s brightness as the figure breathes: a slow swell and dim, each ring a beat
/// behind the one inside it, so every breath travels outward. Layers are stacked from the
/// core out, which is what makes the index work as a delay.
fn breath(k: u32) -> f32 {
    let strength = u.rhythm.y;
    if (strength <= 0.0) { return 1.0; }
    let phase = u.look.w / max(u.rhythm.z, 0.5) * TAU - f32(k) * 0.45;
    return 1.0 + strength * sin(phase);
}
