// Waves of colour rolling out from the centre, recolouring the gold as they pass.

/// The colour the waves lay on radius `r`: (linear rgb weighted by strength, total
/// strength). A soft front, and a fading tail behind it so the colour lingers a moment
/// before the gold comes back.
fn tint_at(r: f32) -> vec4<f32> {
    var acc = vec4<f32>(0.0);
    let n = u32(u.ambient.w);
    for (var i = 0u; i < n; i = i + 1u) {
        let w = u.waves[i];
        // Positive behind the front: inward for a wave travelling out, outward for one
        // travelling in.
        let s = w.front.w * (w.front.x - r) / w.front.y;
        var env = exp(-s * s * 2.0);
        if (s > 0.0) { env = max(env, 0.6 * exp(-s * 1.2)); }
        let k = w.front.z * env;
        acc = acc + vec4<f32>(w.color.rgb * k, k);
    }
    return acc;
}
