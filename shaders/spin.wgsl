const PI: f32 = 3.14159265359;
const TAU: f32 = 6.28318530718;
const MAX_LAYERS: u32 = 16u;
const MAX_PULSES: u32 = 8u;
const MAX_FLARES: u32 = 8u;
const MAX_BOLT_SEGS: u32 = 96u;
const MAX_GLYPHS: u32 = 16u;

struct GpuLayer {
    // kind, sides, radius, phase_rad   (kind: 0 circle, 1 poly, 2 star)
    outer: vec4<f32>,
    // star phase_rad, min radius, max radius, unused
    outer_x: vec4<f32>,
    inner: vec4<f32>,
    inner_x: vec4<f32>,
    // angle (radians), bloom 0..1, sin(angle), cos(angle)
    motion: vec4<f32>,
};

struct Uniforms {
    center: vec2<f32>,
    src_size: vec2<f32>,
    // scale, offset.x, offset.y, n_layers
    fit: vec4<f32>,
    // supersample, disc radius, glow pyramid top LOD, live lit glyphs
    params: vec4<f32>,
    // live pulses, live flares, live bolt segments, collapse flash
    counts: vec4<f32>,
    // (mip level to read the artwork at, layer-tint strength, unused, unused)
    misc: vec4<f32>,
    // (min x, min y, max x, max y) around the lit glyphs in the artwork
    gbox: vec4<f32>,
    // ...and around the risen copies, in un-rotated space
    gbox_p: vec4<f32>,
    background: vec4<f32>,
    // (crest radius, crest strength, envelope width, ripple wavelength) per pulse
    pulses: array<vec4<f32>, MAX_PULSES>,
    // (splash strength, splash radius, wake direction, unused) for the same pulse
    pulse_x: array<vec4<f32>, MAX_PULSES>,
    // (x, y, radius, strength) per flare
    flares: array<vec4<f32>, MAX_FLARES>,
    // (x0, y0, x1, y1) per lightning limb
    bolts: array<vec4<f32>, MAX_BOLT_SEGS>,
    // (strength, half-width, unused, unused) for the limb at the same index
    bolt_w: array<vec4<f32>, MAX_BOLT_SEGS>,
    // (x, y, half-width, half-height) per lit glyph, at where it has floated to
    glyphs: array<vec4<f32>, MAX_GLYPHS>,
    // (risen x, risen y, strength of the copy, progress) per glyph, un-rotated space
    glyph_a: array<vec4<f32>, MAX_GLYPHS>,
    // (sin, cos) of the angle that glyph own layer has turned to
    glyph_r: array<vec4<f32>, MAX_GLYPHS>,
    layers: array<GpuLayer, MAX_LAYERS>,
};

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var src_tex: texture_2d<f32>;
@group(0) @binding(2) var src_smp: sampler;
// Highlight of the source, box-filtered into a mip chain. See gpu::glow_pyramid.
@group(0) @binding(3) var glow_tex: texture_2d<f32>;

fn rem_euclid(x: f32, y: f32) -> f32 {
    return x - floor(x / y) * y;
}

// Distance from the centre to a boundary along `alpha` (radians, from North, CCW).
// One distinct colour per layer, outward-in, matching `LAYER_TINT` in `render.rs` and the
// viewer's 1-7 keys.
fn layer_tint(k: u32) -> vec3<f32> {
    switch (k) {
        case 0u: { return vec3<f32>(1.00, 0.28, 0.28); }
        case 1u: { return vec3<f32>(1.00, 0.62, 0.16); }
        case 2u: { return vec3<f32>(0.95, 0.95, 0.22); }
        case 3u: { return vec3<f32>(0.32, 0.95, 0.38); }
        case 4u: { return vec3<f32>(0.26, 0.90, 0.96); }
        case 5u: { return vec3<f32>(0.48, 0.58, 1.00); }
        default: { return vec3<f32>(0.96, 0.42, 0.96); }
    }
}

fn boundary_radius(b: vec4<f32>, extra: vec4<f32>, alpha: f32) -> f32 {
    if (b.x < 0.5) {
        return b.y;                       // circle: radius in .y
    }
    let n = b.y;
    let step = TAU / n;

    if (b.x < 1.5) {                      // regular polygon
        let r = b.z;
        if (r <= 0.0) { return 0.0; }
        let a = rem_euclid(alpha - b.w + step * 0.5, step) - step * 0.5;
        return r * cos(PI / n) / cos(a);
    }

    // star: .z outer radius, .w inner radius, extra.x phase
    let ro = b.z;
    let ri = b.w;
    if (ro <= 0.0) { return 0.0; }
    let half = step * 0.5;
    let a = abs(rem_euclid(alpha - extra.x + half, step) - half);
    let den = ro * sin(half - a) + ri * sin(a);
    if (abs(den) < 1e-6) { return ro; }
    return ro * ri * sin(half) / den;
}

// Colour the gold strokes glow with. Light spilling off the artwork, not new detail.
const GLOW: vec3<f32> = vec3<f32>(1.0, 0.74, 0.30);

// Arterial blood: dark, and with almost nothing left in green or blue. It is darker than
// the gold it runs through, which is why `shade` mixes it over the stroke rather than
// only adding — an additive term can brighten but never deepen.
const BLOOD: vec3<f32> = vec3<f32>(0.42, 0.012, 0.012);

// ...and the scarlet its lit rim runs at. Blood in bulk is nearly black; what makes a
// wave of it readable is the light caught along its leading edge, not the bulk behind.
const BLOOD_HOT: vec3<f32> = vec3<f32>(0.90, 0.06, 0.03);

// How far the wavefront displaces what lies under it, in source pixels. The dividers
// between layers sit in empty gaps by construction, so a shift of this order just moves
// the gap and cannot tear a line; well past it the wave would start dragging artwork
// from one layer into the next.
const WARP_GAIN: f32 = 9.0;

// Electric blue for the lightning, and the near-white its cores run at. The core is the
// light source itself, so it is the one thing drawn straight over the artwork.
const SPARK: vec3<f32> = vec3<f32>(0.22, 0.52, 1.0);
const SPARK_CORE: vec3<f32> = vec3<f32>(0.72, 0.88, 1.0);

/// Blood at radius `r`: (body, lit rim, trough, refraction).
///
/// A ring is only ever as readable as its edges, so this hands `shade` the pieces it
/// needs to build one with hard edges, rather than a single strength it can only fade out:
///
///   * `body` is the bulk, which is nearly black and darkens whatever it crosses;
///   * `edge` is the light caught along the leading rim — the one bright part of a wave
///     of something this dark;
///   * `trough` is the dip that runs ahead of the crest, as it does on water. Darkening
///     there is what gives the ring an outer boundary instead of a fade;
///   * `warp` displaces what lies under the wave, largest on its faces and zero at the
///     crest where the surface is flat.
fn pulse_at(r: f32) -> vec4<f32> {
    var body = 0.0;
    var edge = 0.0;
    var trough = 0.0;
    var warp = 0.0;
    let n = u32(u.counts.x);
    for (var i = 0u; i < n; i = i + 1u) {
        let p = u.pulses[i];
        let x = u.pulse_x[i];

        // The impact itself: a blob where the drop hit, gone almost as soon as the crest
        // has left it. An implosion collapsing inward lights this up instead.
        if (x.x > 0.0) {
            let t = max(1.0 - (r * r) / (x.y * x.y), 0.0);
            body = body + x.x * t * t;
            edge = edge + x.x * t * t * t * 0.8;
        }

        // Outside the reach of the wave there is nothing to work out.
        if (abs(r - p.x) > p.z * 7.0) { continue; }

        // Distance along the wave, positive behind the crest — which is inward for a wave
        // leaving the centre and outward for one closing on it, hence the sign in `x.z`.
        // Getting that backwards puts the wake in front of the wave, and an implosion
        // then washes the middle of the disc before it has arrived.
        let s = x.z * (p.x - r) / p.z;
        var env = 0.0;
        if (s < 0.0) {
            // Ahead of the crest: a steep face, undisturbed liquid just beyond it.
            let a = s * 2.6;
            env = exp(-a * a);
        } else {
            // Behind it: a wake of two or three ripples. Letting this run on any longer
            // reddens the whole disc, which is what stopped the ring reading as a ring.
            env = exp(-s * 1.8);
        }

        // Ripples in the wake. abs() because a trough shows as much blood as a crest —
        // this is light off a surface, not a height.
        let ripple = abs(cos(TAU * (p.x - r) / p.w));
        // The crest stays solid; the ripples only take over behind it.
        body = body + p.y * env * mix(1.0, ripple, clamp(s * 0.6, 0.0, 0.85));

        // A narrow hot line riding the front.
        let e = s * 2.8;
        edge = edge + p.y * exp(-e * e);

        // ...and the dip just ahead of it.
        let t = (s + 1.35) * 2.2;
        trough = trough + p.y * exp(-t * t);

        // Zero at the crest, signed either side of it: the shape of a lens.
        warp = warp + p.y * s * env;
    }
    return vec4<f32>(body, edge, trough, warp);
}

/// Afterglow left where lightning struck, at source-space point `p`.
///
/// Placed in un-rotated coordinates, so it stays where the bolt landed while the layers
/// keep turning underneath it.
fn flare_at(p: vec2<f32>) -> f32 {
    var acc = 0.0;
    let n = u32(u.counts.y);
    for (var i = 0u; i < n; i = i + 1u) {
        let s = u.flares[i];
        let d = (p - s.xy) / s.z;
        let t = max(1.0 - dot(d, d), 0.0);
        acc = acc + s.w * t * t;
    }
    return acc;
}

// The gold a glyph runs at once it has been lit: hot and nearly white, so an activated
// letter reads as charged rather than merely brighter than its neighbours.
const GLYPH_HOT: vec3<f32> = vec3<f32>(1.0, 0.92, 0.62);

/// Gold through white to violet, as a glyph rises and lets go.
///
/// The ramp is compressed into the window where the copy is actually visible. Spread over
/// the whole life it finished violet at the moment the copy had already faded to nothing,
/// so the colour change was there but could never be caught.
fn glyph_colour(t: f32) -> vec3<f32> {
    let u = clamp(t / 0.55, 0.0, 1.0);
    let hot = vec3<f32>(1.0, 0.92, 0.62);
    let white = vec3<f32>(1.0, 1.0, 1.0);
    let violet = vec3<f32>(0.62, 0.30, 1.0);
    if (u < 0.45) {
        return mix(hot, white, u / 0.45);
    }
    return mix(white, violet, (u - 0.45) / 0.55);
}

/// How much of a glyph has left its slot in the artwork.
///
/// Tested in `q`, the layer-rotated frame, because that is where the glyph actually sits
/// and it has to stay with its layer as that turns. Once it is up, the slot is empty —
/// which is the dark disc an earlier version went out of its way to avoid. That was right
/// then, when nothing was leaving and it read as a hole punched in the plate; now
/// something is, and the gap is the point.
fn glyph_hide(p: vec2<f32>) -> f32 {
    var acc = 0.0;
    let n = u32(u.params.w);
    for (var i = 0u; i < n; i = i + 1u) {
        let a = u.glyph_a[i];
        let g = u.glyphs[i];
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
    let n = u32(u.params.w);
    for (var i = 0u; i < n; i = i + 1u) {
        let a = u.glyph_a[i];
        if (a.z <= 0.002) { continue; }
        let g = u.glyphs[i];
        let grow = 1.0 + 1.6 * a.w;
        let ext = g.zw * grow;
        let local = p - a.xy;
        let d = local / ext;
        let t = max(1.0 - dot(d, d), 0.0);
        if (t <= 0.0) { continue; }
        // Back into the artwork: undo the layer rotation, then undo the growth. Without
        // the rotation the copy would sit at whatever angle its layer happened to be at,
        // and could face the wrong way up entirely.
        let rot = u.glyph_r[i];
        let art = vec2<f32>(
            local.x * rot.y - local.y * rot.x,
            local.y * rot.y + local.x * rot.x,
        ) / grow;
        let uv = (g.xy + art) / u.src_size;
        let ink = textureSampleLevel(src_tex, src_smp, uv, u.misc.x + a.w * 2.5).rgb;
        // The ink supplies the shape and the ramp supplies the colour. Tinting the ink
        // itself cannot work: it is gold, so it has no blue for a violet to scale, and the
        // ramp would only ever come out as a duller orange.
        let lum = dot(ink, vec3<f32>(0.299, 0.587, 0.114));
        acc = acc + glyph_colour(a.w) * (lum * a.z * t * t * 1.6);
    }
    return acc;
}

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
    let n = u32(u.counts.z);
    for (var i = 0u; i < n; i = i + 1u) {
        let s = u.bolts[i];
        let w = u.bolt_w[i];
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

/// Gather highlights around `q` so bright strokes spill light into the dark ground.
///
/// Only the *amount* of nearby light is used, tinted with `tint` — sampling the colours
/// directly would drag neighbouring layers' artwork in as ghost detail. The keypress
/// bloom and the red energy both come through here, with their tints blended by strength,
/// so lighting up for two reasons at once still costs one set of taps.
///
/// The gather itself is precomputed: level L of the glow pyramid already holds the mean
/// highlight over a 2^L box, so picking the level from the spill radius reads the same
/// neighbourhood average two rings of taps used to compute from scratch. Four taps on a
/// diagonal cross soften the box into something rounder, which is what the rotated
/// second ring used to be for. The level, spread and gain are fitted to the old kernel:
/// the pictures differ by ~1% RMSE, which is the disc-vs-ring shape and nothing else.
fn spill(q: vec2<f32>, amount: f32, tint: vec3<f32>) -> vec3<f32> {
    let radius = 1.5 + 5.5 * min(amount, 1.0);
    let lod = clamp(log2(radius * 0.5), 0.0, u.params.z);
    let uv = q / u.src_size;
    let o = (radius * 0.35) / u.src_size;
    var acc = textureSampleLevel(glow_tex, src_smp, uv + vec2<f32>( o.x,  o.y), lod).r;
    acc = acc + textureSampleLevel(glow_tex, src_smp, uv + vec2<f32>(-o.x,  o.y), lod).r;
    acc = acc + textureSampleLevel(glow_tex, src_smp, uv + vec2<f32>( o.x, -o.y), lod).r;
    acc = acc + textureSampleLevel(glow_tex, src_smp, uv + vec2<f32>(-o.x, -o.y), lod).r;
    return tint * (acc * 0.25) * amount * 1.7;
}

/// Everything that lives in un-rotated space and varies smoothly across one pixel.
///
/// These are the expensive loops — the limb list especially, which can run to fifty-odd
/// entries during an implosion — and none of them is a function of which layer a sub-sample
/// lands in. So they are worked out once at the pixel centre and shared, instead of four
/// times over at `ss=2`. The layer search and the artwork lookup still get supersampled,
/// because that is where the detail actually is.
///
/// The glyphs deliberately stay out of this: they are positioned in the artwork and ride
/// their layer as it turns, so they have to be tested in rotated space, per sub-sample.
struct Fields {
    // body, lit rim, trough, refraction
    blood: vec4<f32>,
    // hot core, blue halo
    arc: vec2<f32>,
    flare: f32,
};

fn fields_at(p: vec2<f32>) -> Fields {
    let d = p - u.center;
    var f: Fields;
    f.blood = pulse_at(length(d));
    f.arc = bolt_at(p);
    f.flare = flare_at(p);
    return f;
}

// Colour of one sub-sample, given a point in SOURCE pixel coordinates.
fn shade(p: vec2<f32>, f: Fields) -> vec3<f32> {
    let d = p - u.center;
    let r2 = dot(d, d);
    // Nothing is drawn past the outermost boundary; most of the window is out here.
    if (r2 >= u.params.y * u.params.y) {
        return u.background.rgb;
    }
    let r = sqrt(r2);
    // alpha measured from North, CCW: dx = -r sin a, dy = -r cos a
    let alpha = atan2(-d.x, -d.y);

    let red = f.blood.x;
    let edge = f.blood.y;
    let trough = f.blood.z;
    let warp = f.blood.w;
    let arc = f.arc;
    let blue = arc.y + f.flare;

    var col = u.background.rgb;
    let n = u32(u.fit.w);
    for (var k: u32 = 0u; k < n; k = k + 1u) {
        let L = u.layers[k];

        // The radius alone rules this layer out at every angle.
        if (r < L.inner_x.y || r >= L.outer_x.z) { continue; }

        // ...and where it does not, it often still settles one side of the test on its
        // own, leaving at most one boundary to actually evaluate.
        let a = alpha - L.motion.x;
        var ri = -1.0;
        if (r < L.inner_x.z) { ri = boundary_radius(L.inner, L.inner_x, a); }
        var ro = 3.4e38;
        if (r >= L.outer_x.y) { ro = boundary_radius(L.outer, L.outer_x, a); }

        if (r >= ri && r < ro) {
            // Un-rotating the pixel by the layer's angle is just rotating the offset the
            // other way, so the precomputed sin/cos replace two more trig calls.
            let s = L.motion.z;
            let c = L.motion.w;
            let q = u.center + vec2<f32>(d.x * c - d.y * s, d.y * c + d.x * s);

            // One box around the lit glyphs, tested once. The groups are clusters by
            // construction, so it is tight and almost every pixel leaves here.
            var hide = 0.0;
            if (u.params.w > 0.0 && q.x >= u.gbox.x && q.y >= u.gbox.y
                && q.x <= u.gbox.z && q.y <= u.gbox.w) {
                hide = glyph_hide(q);
            }

            // The wave is a lens: push the point we sample along the radius by however
            // much the surface above it is tilted. Rotation preserves radius, so
            // displacing radially here needs no correction for the layer's own angle.
            // This is what stops the ring reading as a colour laid over the artwork
            // instead of as something happening to it.
            let qd = q - u.center;
            let qr = length(qd);
            let qw = u.center + qd * ((qr + warp * WARP_GAIN) / max(qr, 1e-3));

            let uv = qw / u.src_size;
            if (uv.x < 0.0 || uv.y < 0.0 || uv.x > 1.0 || uv.y > 1.0) {
                break;
            }
            // Read the artwork at a level matched to how hard it is being shrunk. The
            // window decides that, not the pixel, so the level is worked out once on the
            // CPU — and the sampler cannot work it out for itself here anyway, since this
            // sits inside a loop that breaks.
            col = textureSampleLevel(src_tex, src_smp, uv, u.misc.x).rgb;

            // A glyph that has lifted off leaves its slot empty; the copy is drawn
            // somewhere else entirely, at the end, in a frame this loop cannot clip.
            col = col * (1.0 - hide);

            // Which layer this pixel belongs to, said in colour. Only the hue is replaced,
            // and it goes before the effects so they keep their own colours on top.
            if (u.misc.y > 0.0) {
                let lum = dot(col, vec3<f32>(0.299, 0.587, 0.114));
                col = mix(col, layer_tint(k) * lum * 1.35, u.misc.y);
            }

            // The dip ahead of the crest. A ring needs a hard outer boundary to read as
            // one, and darkening is the only way to draw an edge on artwork this sparse.
            col = col * (1.0 - min(trough, 1.0) * 0.72);

            // Blood is darker than the gold it runs through, so it replaces the stroke's
            // colour rather than being added to it — and keeps rather less of the light
            // that was there, so the lit rim has something dark to stand against.
            if (red > 0.002) {
                let lum = dot(col, vec3<f32>(0.299, 0.587, 0.114));
                col = mix(col, BLOOD * lum * 1.5, min(red, 1.0));
            }

            let b = L.motion.y;
            // The lit rim counts for less than its brightness suggests: it is a thin
            // line, and letting it drive the spill at full weight washes the wake out.
            let amt = b + red + blue + edge * 0.6;
            if (amt > 0.002) {
                // Drive the stroke itself brighter, each source in its own colour.
                col = col + col * (b * 1.3) + col * SPARK * (blue * 2.0);
                // ...and let the whole lot spill into the ground, tinted by whichever
                // of them is doing the lighting. The spill samples the unwarped point:
                // the glow pyramid is about where the artwork is, not where the wave has
                // bent it to.
                col = col + spill(
                    q,
                    amt,
                    (GLOW * b + BLOOD * red + SPARK * blue + BLOOD_HOT * (edge * 0.6))
                        / amt,
                );
                // A little lands on the ground and not just on the strokes, so a passing
                // wave reads as a wave rather than as a row of briefly brighter glyphs.
                // Lightning lights the air it crosses, so it puts down more.
                col = col + BLOOD * (red * 0.05) + SPARK * (blue * 0.12);
            }
            // The lit rim goes on last and on top: it is the one part of the wave that is
            // brighter than what it crosses, so the dark body must not mix it away.
            col = col + col * BLOOD_HOT * (edge * 0.85) + BLOOD_HOT * (edge * 0.12);
            break;
        }
    }

    // The whole formation answers when an implosion lands.
    col = col + col * (u.counts.w * 0.9) + BLOOD_HOT * (u.counts.w * 0.05);

    // The arc cores and the risen glyphs are light sources in their own right rather
    // than things lighting the artwork, so they go over the top of whatever the pixel
    // turned out to be — and, just as importantly, outside the layer loop, so a copy that
    // has drifted into the next band is not clipped or displaced by it.
    var risen = vec3<f32>(0.0);
    if (u.params.w > 0.0 && p.x >= u.gbox_p.x && p.y >= u.gbox_p.y
        && p.x <= u.gbox_p.z && p.y <= u.gbox_p.w) {
        risen = glyph_ghost(p);
    }
    return col + SPARK_CORE * arc.x + risen;
}

@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    // Fullscreen triangle.
    let x = f32(i32(i) / 2) * 4.0 - 1.0;
    let y = f32(i32(i) & 1) * 4.0 - 1.0;
    return vec4<f32>(x, y, 0.0, 1.0);
}

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    // A uniform grid, deliberately. A rotated grid is the usual improvement, but it only
    // pays for geometric edges, and this renderer has none worth the name: the disc cut and
    // every layer boundary sit in empty gaps by construction, so all the visible detail is
    // texture. Measured, the rotated grid was simply a wider filter — slightly less
    // high-frequency energy, but further from a brute-force reference, which is the wrong
    // trade when the complaint is blur.
    let ss = i32(u.params.x);
    let base = floor(pos.xy);
    var acc = vec3<f32>(0.0);

    // Resolved once at the pixel centre and shared by every sub-sample.
    //
    // Guarded by the same disc test `shade` uses, because most of a window is outside the
    // plate and none of this applies there. Without the guard, hoisting hands the empty
    // frame a bill it never used to pay, and for a handful of limbs that costs more than
    // the sharing saves.
    let centre = (base + vec2<f32>(0.5) - u.fit.yz) / u.fit.x;
    let cd = centre - u.center;
    var f: Fields;
    if (dot(cd, cd) < u.params.y * u.params.y) {
        f = fields_at(centre);
    }

    for (var j = 0; j < ss; j = j + 1) {
        for (var i = 0; i < ss; i = i + 1) {
            let off = (vec2<f32>(f32(i), f32(j)) + 0.5) / f32(ss);
            // window pixel -> source pixel (aspect-preserving fit)
            let p = (base + off - u.fit.yz) / u.fit.x;
            acc = acc + shade(p, f);
        }
    }
    return vec4<f32>(acc / f32(ss * ss), 1.0);
}
