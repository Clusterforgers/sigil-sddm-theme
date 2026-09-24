// Letters flickering through other letters of the figure, then settling back.

struct Swapped {
    // the borrowed letters, premultiplied
    ink: vec4<f32>,
    // how much of the real letter to hide, 0..1
    hide: f32,
};

/// What the scrambling letters of layer `k` put at layer-frame point `q`.
///
/// Each borrowed letter is read from its own layer's texture, turned by the difference in
/// the two letters' angles round the centre and scaled to the slot. Every letter stands
/// upright to the centre, so that turn is all it takes for the borrowed letter to stand
/// the right way up in its new place.
fn swapped_at(q: vec2<f32>, k: u32) -> Swapped {
    var out: Swapped;
    out.ink = vec4<f32>(0.0);
    out.hide = 0.0;
    let n = u32(u.rhythm.x);
    for (var i = 0u; i < n; i = i + 1u) {
        let s = u.swaps[i];
        if (u32(s.slot.w) != k || s.source.w <= 0.0) { continue; }
        let off = q - s.slot.xy;
        let d2 = dot(off, off) / (s.slot.z * s.slot.z);
        if (d2 >= 0.8) { continue; }
        // Soft at the rim, so the patch blends into the ground round it, and short of the
        // slot's full reach: out there the donor's neighbours begin, and would show as faint
        // arcs round the borrowed letter.
        let t = (1.0 - smoothstep(0.35, 0.8, d2)) * s.source.w;
        let m = s.map;
        let src = s.source.xy + vec2<f32>(off.x * m.x - off.y * m.y, off.x * m.y + off.y * m.x) * m.z;
        out.ink = out.ink + art(src, u32(s.source.z), u.quality.w) * t;
        out.hide = max(out.hide, t);
    }
    return out;
}
