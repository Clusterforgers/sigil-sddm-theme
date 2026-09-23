// Real-time version of src/render.rs.
//
// Same idea: rotation preserves radius, so one pass over the output pixels handles every
// layer. For each pixel we un-rotate by each layer's angle and take the first layer whose
// region contains the result. Keep `boundary_radius` in step with geom.rs.

const PI: f32 = 3.14159265359;
const TAU: f32 = 6.28318530718;
const MAX_LAYERS: u32 = 16u;

struct GpuLayer {
    // kind, sides, radius, phase_deg   (kind: 0 circle, 1 poly, 2 star)
    outer: vec4<f32>,
    // star inner radius, unused, unused, unused
    outer_x: vec4<f32>,
    inner: vec4<f32>,
    inner_x: vec4<f32>,
    // angle (radians), bloom 0..1, unused, unused
    motion: vec4<f32>,
};

struct Uniforms {
    center: vec2<f32>,
    src_size: vec2<f32>,
    // scale, offset.x, offset.y, n_layers
    fit: vec4<f32>,
    // supersample, unused, unused, unused
    params: vec4<f32>,
    background: vec4<f32>,
    layers: array<GpuLayer, MAX_LAYERS>,
};

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var src_tex: texture_2d<f32>;
@group(0) @binding(2) var src_smp: sampler;

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
        let a = rem_euclid(alpha - radians(b.w) + step * 0.5, step) - step * 0.5;
        return r * cos(PI / n) / cos(a);
    }

    // star: .z outer radius, .w inner radius, extra.x phase
    let ro = b.z;
    let ri = b.w;
    if (ro <= 0.0) { return 0.0; }
    let half = step * 0.5;
    let a = abs(rem_euclid(alpha - radians(extra.x) + half, step) - half);
    let den = ro * sin(half - a) + ri * sin(a);
    if (abs(den) < 1e-6) { return ro; }
    return ro * ri * sin(half) / den;
}

// Colour the gold strokes glow with. Light spilling off the artwork, not new detail.
const GLOW: vec3<f32> = vec3<f32>(1.0, 0.74, 0.30);

fn tap(q: vec2<f32>) -> vec3<f32> {
    return textureSampleLevel(src_tex, src_smp, q / u.src_size, 0.0).rgb;
}

fn luma(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.299, 0.587, 0.114));
}

/// How much light a tap contributes. Thresholded, so only the gold strokes spill and
/// the dark ground contributes nothing — without this the whole layer lifts into a
/// uniform grey veil instead of glowing.
///
/// The texture is sRGB, so `tap` already returns linear values: gold sits near 0.5,
/// the #0E0D0C ground near 0.004.
fn highlight(c: vec3<f32>) -> f32 {
    return max(luma(c) - 0.12, 0.0);
}

/// Gather highlights around `q` so bright strokes spill light into the dark ground.
///
/// Only the *amount* of nearby light is used, tinted with GLOW — sampling the colours
/// directly would drag neighbouring layers' artwork in as ghost detail.
///
/// Two rings of taps, the outer rotated half a step against the inner to break up the
/// directional streaking a single ring gives over fine line art. The inner ring is
/// weighted double so the glow hugs the stroke rather than washing the whole band.
fn bloom_at(q: vec2<f32>, amount: f32) -> vec3<f32> {
    let radius = 1.5 + 5.5 * amount;
    let n = 12;
    var acc = 0.0;
    for (var i = 0; i < n; i = i + 1) {
        let th = TAU * f32(i) / f32(n);
        acc = acc + 2.0 * highlight(tap(q + vec2<f32>(cos(th), sin(th)) * radius * 0.4));

        let th2 = th + PI / f32(n);
        acc = acc + highlight(tap(q + vec2<f32>(cos(th2), sin(th2)) * radius));
    }
    return GLOW * (acc / f32(3 * n)) * amount * 2.0;
}

// Colour of one sub-sample, given a point in SOURCE pixel coordinates.
fn shade(p: vec2<f32>) -> vec3<f32> {
    let d = p - u.center;
    let r = length(d);
    // alpha measured from North, CCW: dx = -r sin a, dy = -r cos a
    let alpha = atan2(-d.x, -d.y);

    let n = u32(u.fit.w);
    for (var k: u32 = 0u; k < n; k = k + 1u) {
        let L = u.layers[k];
        let a = alpha - L.motion.x;
        let ri = boundary_radius(L.inner, L.inner_x, a);
        let ro = boundary_radius(L.outer, L.outer_x, a);
        if (r >= ri && r < ro) {
            let q = vec2<f32>(u.center.x - r * sin(a), u.center.y - r * cos(a));
            let uv = q / u.src_size;
            if (uv.x < 0.0 || uv.y < 0.0 || uv.x > 1.0 || uv.y > 1.0) {
                break;
            }
            var c = textureSampleLevel(src_tex, src_smp, uv, 0.0).rgb;

            let b = L.motion.y;
            if (b > 0.002) {
                c = c + c * b * 1.3;          // drive the stroke itself brighter
                c = c + bloom_at(q, b);       // and let it spill into the ground
            }
            return c;
        }
    }
    return u.background.rgb;
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
