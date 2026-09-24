// A layer burning away to embers along a ragged edge, and growing back.

// The colour of the burning edge.
const EMBER: vec3<f32> = vec3<f32>(1.0, 0.42, 0.08);

/// Layer `k` at layer-frame point `q`, burnt by `amount` (0..1): (how much of the ink is
/// left, how brightly the burning edge glows). The noise lives in the layer's own frame,
/// so the burn pattern turns with the layer instead of sliding across it.
fn burn(q: vec2<f32>, k: u32, amount: f32) -> vec2<f32> {
    if (amount <= 0.0) { return vec2<f32>(1.0, 0.0); }
    let p = q * 0.03 + vec2<f32>(f32(k) * 13.1, f32(k) * 7.3);
    let n = 0.65 * noise2(p) + 0.35 * noise2(p * 2.7 + 5.2);
    // Below the threshold the ink is gone; just above it, what is left glows.
    let edge = n - amount;
    let left = smoothstep(0.0, 0.03, edge);
    let glow = left * (1.0 - smoothstep(0.03, 0.12, edge)) * min(amount * 6.0, 1.0);
    return vec2<f32>(left, glow);
}
