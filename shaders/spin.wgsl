// Real-time version of src/render.rs.
//
// Same idea: rotation preserves radius, so one pass over the output pixels handles every
// layer. For each pixel we un-rotate by each layer's angle and take the first layer whose
// region contains the result. Keep `boundary_radius` in step with geom.rs.
//
// Two things keep the per-pixel cost down, both set up on the CPU in src/gpu.rs:
//
//   * every boundary carries the min/max radius it reaches over all angles, so a pixel's
//     radius usually settles containment with two compares instead of trigonometry;
//   * the bloom reads a pre-blurred highlight pyramid rather than gathering the source
//     with two rings of taps.

const PI: f32 = 3.14159265359;
const TAU: f32 = 6.28318530718;
const MAX_LAYERS: u32 = 16u;
const MAX_PULSES: u32 = 8u;
const MAX_FLARES: u32 = 8u;
const MAX_BOLT_SEGS: u32 = 96u;

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
    // supersample, disc radius, glow pyramid top LOD, unused
    params: vec4<f32>,
    // live pulses, live flares, live bolt segments, collapse flash
    counts: vec4<f32>,
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

// Colour of one sub-sample, given a point in SOURCE pixel coordinates.
fn shade(p: vec2<f32>) -> vec3<f32> {
    let d = p - u.center;
    let r2 = dot(d, d);
    // Nothing is drawn past the outermost boundary; most of the window is out here.
    if (r2 >= u.params.y * u.params.y) {
        return u.background.rgb;
    }
    let r = sqrt(r2);
    // alpha measured from North, CCW: dx = -r sin a, dy = -r cos a
    let alpha = atan2(-d.x, -d.y);

    // Where the energy is depends on the pixel, not on which layer ends up owning it,
    // so it is worth resolving before the search rather than inside it.
    let blood = pulse_at(r);
    let red = blood.x;
    let edge = blood.y;
    let trough = blood.z;
    let warp = blood.w;
    let arc = bolt_at(p);
    let blue = arc.y + flare_at(p);

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
            col = textureSampleLevel(src_tex, src_smp, uv, 0.0).rgb;

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
                    (GLOW * b + BLOOD * red + SPARK * blue + BLOOD_HOT * (edge * 0.6)) / amt,
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

    // The arc core is the light source itself rather than something lighting the
    // artwork, so it goes over the top of whatever the pixel turned out to be.
    return col + SPARK_CORE * arc.x;
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
    let ss = i32(u.params.x);
    let base = floor(pos.xy);
    var acc = vec3<f32>(0.0);

    for (var j = 0; j < ss; j = j + 1) {
        for (var i = 0; i < ss; i = i + 1) {
            let off = (vec2<f32>(f32(i), f32(j)) + 0.5) / f32(ss);
            // window pixel -> source pixel (aspect-preserving fit)
            let p = (base + off - u.fit.yz) / u.fit.x;
            acc = acc + shade(p);
        }
    }
    return vec4<f32>(acc / f32(ss * ss), 1.0);
}
