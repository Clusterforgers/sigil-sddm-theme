// Constellations: glowing threads linking a few letters across the rings.

// Starlight: a cool, pale gold, brighter than the ink it crosses.
const THREAD: vec3<f32> = vec3<f32>(1.0, 0.95, 0.82);

/// The light the constellation threads cast at canvas point `p`: a fine bright line with a
/// soft glow round it, a spark running along it, and a small star at each letter it links.
fn threads_at(p: vec2<f32>) -> f32 {
    var acc = 0.0;
    let n = u32(u.live2.x);
    for (var i = 0u; i < n; i = i + 1u) {
        let t = u.threads[i];
        let a = t.seg.xy;
        let b = t.seg.zw;
        // Nothing reaches more than a few units off the line.
        let lo = min(a, b) - 14.0;
        let hi = max(a, b) + 14.0;
        if (p.x < lo.x || p.y < lo.y || p.x > hi.x || p.y > hi.y) { continue; }
        let d2 = seg_dist2(p, a, b);
        let line = exp(-d2 / 1.4) * 0.9 + exp(-d2 / 40.0) * 0.22;
        let spark = a + (b - a) * t.style.y;
        let ds = dot(p - spark, p - spark);
        let stars = exp(-dot(p - a, p - a) / 30.0) + exp(-dot(p - b, p - b) / 30.0);
        acc = acc + t.style.x * (line + exp(-ds / 14.0) * 1.6 + stars * 0.7);
    }
    return acc;
}
