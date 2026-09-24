// Lightning: jagged limbs arcing across the formation.

// Electric blue for the lightning, and the near-white its cores run at. The core is the
// light source itself, so it is the one thing drawn straight over the artwork.
const SPARK: vec3<f32> = vec3<f32>(0.22, 0.52, 1.0);
const SPARK_CORE: vec3<f32> = vec3<f32>(0.72, 0.88, 1.0);

/// Squared distance from `p` to the segment `a`-`b`.
fn seg_dist2(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let ab = b - a;
    let t = clamp(dot(p - a, ab) / max(dot(ab, ab), 1e-6), 0.0, 1.0);
    let d = p - (a + ab * t);
    return dot(d, d);
}

/// The lightning at `p`: x is the hot core, y the blue wash around it.
///
/// Every live bolt's limbs sit in one flat array, so this is a single pass. A limb only
/// lights its own neighbourhood, so the box test throws almost all of them out for almost
/// every pixel before any distance is computed — which is what keeps a screen full of
/// lightning about as cheap as an empty one.
///
/// The wash has compact support for that reason: an inverse-square skirt would have a
/// tail too long to cull, and the tail was worth very little to look at.
fn bolt_at(p: vec2<f32>) -> vec2<f32> {
    var core = 0.0;
    var halo = 0.0;
    let n = u32(u.live.z);
    for (var i = 0u; i < n; i = i + 1u) {
        let s = u.limbs[i].seg;
        let w = u.limbs[i].style;
        let reach = w.y * 12.0;
        let lo = min(s.xy, s.zw) - reach;
        let hi = max(s.xy, s.zw) + reach;
        if (p.x < lo.x || p.y < lo.y || p.x > hi.x || p.y > hi.y) { continue; }

        let d2 = seg_dist2(p, s.xy, s.zw);
        // Hot core: sharp, so the arc reads as a filament rather than a smear.
        let w2 = w.y * w.y;
        let f = w2 / (d2 + w2);
        core = core + w.x * f * f;
        // Wide wash: most of what makes a strike feel like it carries any power.
        let t = max(1.0 - d2 / (reach * reach), 0.0);
        halo = halo + w.x * t * t * 1.1;
    }
    return vec2<f32>(core, halo);
}
