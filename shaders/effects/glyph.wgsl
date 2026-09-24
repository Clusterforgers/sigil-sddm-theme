// Lit glyphs: a letter charges, lifts off its slot, and lets go.

// The gold a glyph runs at once it has been lit: hot and nearly white, so an activated
// letter reads as charged rather than merely brighter than its neighbours...
const GLYPH_HOT: vec3<f32> = vec3<f32>(1.0, 0.92, 0.62);
// ...and the violet it cools to as it lets go.
const GLYPH_GONE: vec3<f32> = vec3<f32>(0.62, 0.30, 1.0);

/// Gold through white to violet, as a glyph rises and lets go.
///
/// The ramp is compressed into the window where the copy is actually visible. Spread over
/// the whole life it finished violet at the moment the copy had already faded to nothing,
/// so the colour change was there but could never be caught.
fn glyph_colour(t: f32) -> vec3<f32> {
    let k = clamp(t / 0.55, 0.0, 1.0);
    if (k < 0.45) {
        return mix(GLYPH_HOT, vec3<f32>(1.0), k / 0.45);
    }
    return mix(vec3<f32>(1.0), GLYPH_GONE, (k - 0.45) / 0.55);
}

/// How much of a glyph on layer `k` has left its slot in the artwork.
///
/// Tested in `q`, the layer-rotated frame, because that is where the glyph actually sits
/// and it has to stay with its layer as that turns. Once it is up, the slot is empty —
/// which is the dark disc an earlier version went out of its way to avoid. That was right
/// then, when nothing was leaving and it read as a hole punched in the plate; now
/// something is, and the gap is the point.
fn glyph_hide(p: vec2<f32>, k: u32) -> f32 {
    var acc = 0.0;
    let n = u32(u.live.w);
    for (var i = 0u; i < n; i = i + 1u) {
        if (u32(u.glyphs[i].turn.z) != k) { continue; }
        let a = u.glyphs[i].risen;
        let g = u.glyphs[i].slot;
        let d = (p - g.xy) / g.zw;
        let t = max(1.0 - dot(d, d), 0.0);
        if (t <= 0.0) { continue; }
        // Up quickly, and back only once the copy has gone.
        let hide = smoothstep(0.0, 0.12, a.w) * (1.0 - smoothstep(0.72, 1.0, a.w));
        acc = acc + hide * t;
    }
    return min(acc, 1.0);
}

/// The copy a lit glyph sends up as it lets go.
///
/// Worked out in un-rotated space and drawn at the very end, outside the layer loop, so a
/// copy that has drifted into the next band is neither clipped by that boundary nor
/// displaced by its different rotation — which is exactly what used to happen.
///
/// As it rises it grows, cools, and is read from an ever coarser mip, so it comes apart
/// rather than merely dimming. The growth divides the offset into the artwork: inflating
/// the falloff alone only puts a bigger halo around a letter of unchanged size.
fn glyph_ghost(p: vec2<f32>) -> vec3<f32> {
    var acc = vec3<f32>(0.0);
    let n = u32(u.live.w);
    for (var i = 0u; i < n; i = i + 1u) {
        let a = u.glyphs[i].risen;
        if (a.z <= 0.002) { continue; }
        let g = u.glyphs[i].slot;
        let grow = 1.0 + 1.6 * a.w;
        let ext = g.zw * grow;
        let local = p - a.xy;
        let d = local / ext;
        let t = max(1.0 - dot(d, d), 0.0);
        if (t <= 0.0) { continue; }
        // Back into the artwork: undo the layer rotation, then undo the growth. Without
        // the rotation the copy would sit at whatever angle its layer happened to be at,
        // and could face the wrong way up entirely.
        let rot = u.glyphs[i].turn;
        let back = vec2<f32>(
            local.x * rot.y - local.y * rot.x,
            local.y * rot.y + local.x * rot.x,
        ) / grow;
        let ink = art(g.xy + back, u32(rot.z), u.quality.w + a.w * 2.5).rgb;
        // The ink supplies the shape and the ramp supplies the colour. Tinting the ink
        // itself cannot work: it is gold, so it has no blue for a violet to scale, and the
        // ramp would only ever come out as a duller orange.
        let lum = dot(ink, LUMA);
        acc = acc + glyph_colour(a.w) * (lum * a.z * t * t * 1.6);
    }
    return acc;
}
