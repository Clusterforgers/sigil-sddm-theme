// The afterglow left where lightning struck.

/// Afterglow left where lightning struck, at source-space point `p`.
///
/// Placed in un-rotated coordinates, so it stays where the bolt landed while the layers
/// keep turning underneath it.
fn flare_at(p: vec2<f32>) -> f32 {
    var acc = 0.0;
    let n = u32(u.live.y);
    for (var i = 0u; i < n; i = i + 1u) {
        let s = u.flares[i].at;
        let d = (p - s.xy) / s.z;
        let t = max(1.0 - dot(d, d), 0.0);
        acc = acc + s.w * t * t;
    }
    return acc;
}
