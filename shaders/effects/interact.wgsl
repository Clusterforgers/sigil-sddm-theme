// Answering whoever is at the screen: the pointer's ripples and glow, and the letters lit
// by typing.

// The light the pointer brings: warm, soft.
const HOVER: vec3<f32> = vec3<f32>(1.0, 0.85, 0.55);
// A typed letter: hotter and whiter than the gold round it.
const MARK: vec3<f32> = vec3<f32>(1.0, 0.9, 0.62);

/// The rings of light spreading from where the pointer passed, at canvas point `p`.
fn ripples_at(p: vec2<f32>) -> f32 {
    var acc = 0.0;
    let n = u32(u.live2.y);
    for (var i = 0u; i < n; i = i + 1u) {
        let r = u.ripples[i].at;
        let d = (length(p - r.xy) - r.z) / 7.0;
        acc = acc + r.w * exp(-d * d);
    }
    return acc;
}

/// How close the pointer is to canvas point `p`, 0..1, while it is over the window.
fn near_at(p: vec2<f32>) -> f32 {
    let presence = u.pointer.z;
    if (presence <= 0.0) { return 0.0; }
    let d = p - u.pointer.xy;
    return presence * exp(-dot(d, d) / (110.0 * 110.0));
}

/// How brightly the typed letters of layer `k` burn at layer-frame point `q`: steady while
/// lit, with a flare as each one lights.
fn marks_at(q: vec2<f32>, k: u32) -> f32 {
    var acc = 0.0;
    let n = u32(u.live2.z);
    for (var i = 0u; i < n; i = i + 1u) {
        let m = u.marks[i];
        if (u32(m.slot.w) != k) { continue; }
        let d2 = dot(q - m.slot.xy, q - m.slot.xy) / (m.slot.z * m.slot.z);
        let t = 1.0 - smoothstep(0.3, 1.0, d2);
        acc = acc + t * (m.glow.x * 0.9 + m.glow.y * 2.2);
    }
    return acc;
}

/// The glow the typed letters throw on the ground round them, at canvas point `p`.
fn marks_halo(p: vec2<f32>) -> f32 {
    var acc = 0.0;
    let n = u32(u.live2.z);
    for (var i = 0u; i < n; i = i + 1u) {
        let m = u.marks[i];
        let reach = m.slot.z * 3.0;
        let d = p - m.glow.zw;
        acc = acc + (m.glow.x * 0.12 + m.glow.y * 0.5) * exp(-dot(d, d) / (reach * reach));
    }
    return acc;
}
