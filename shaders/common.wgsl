// Shared by every other shader file: the uniform layout and the bindings.
// `gpu::shader::SOURCE` puts this first.

const PI: f32 = 3.14159265359;
const TAU: f32 = 6.28318530718;
const MAX_LAYERS: u32 = 16u;
const MAX_PULSES: u32 = 16u;
const MAX_FLARES: u32 = 16u;
const MAX_BOLT_SEGS: u32 = 96u;
const MAX_GLYPHS: u32 = 32u;
const MAX_WAVES: u32 = 4u;
const MAX_SWAPS: u32 = 12u;
const MAX_THREADS: u32 = 16u;
const MAX_RIPPLES: u32 = 8u;
const MAX_MARKS: u32 = 24u;

struct GpuLayer {
    // angle (radians), bloom 0..1, sin(angle), cos(angle)
    motion: vec4<f32>,
    // nearest ink, furthest ink, unused, unused — radii from the centre
    extent: vec4<f32>,
    // scale, opacity, dissolve, echo lag — (1, 1, 0, 0) at rest; the surge flings and fades
    // them, a dissolve burns them away, a fast spin leaves echoes that many radians behind
    form: vec4<f32>,
};

struct GpuPulse {
    // crest radius, crest strength, envelope width, ripple wavelength
    wave: vec4<f32>,
    // splash strength, splash radius, wake direction, unused
    splash: vec4<f32>,
};

struct GpuFlare {
    // x, y, radius, strength
    at: vec4<f32>,
};

struct GpuLimb {
    // x0, y0, x1, y1
    seg: vec4<f32>,
    // strength, half-width, unused, unused
    style: vec4<f32>,
};

struct GpuWave {
    // front radius, width, strength, direction (1 outward, -1 inward)
    front: vec4<f32>,
    // linear rgb, unused
    color: vec4<f32>,
};

struct GpuSwap {
    // x, y, radius, layer of the letter being replaced
    slot: vec4<f32>,
    // x, y, layer, strength of the letter drawn instead
    source: vec4<f32>,
    // cos, sin, scale, unused — slot point to source point
    map: vec4<f32>,
};

struct GpuThread {
    // x0, y0, x1, y1 — un-rotated
    seg: vec4<f32>,
    // strength, where along it the spark is 0..1, unused, unused
    style: vec4<f32>,
};

struct GpuRipple {
    // x, y, radius, strength
    at: vec4<f32>,
};

struct GpuMark {
    // x, y, radius, layer — where the letter sits in its layer
    slot: vec4<f32>,
    // steady glow, flash, x, y on screen
    glow: vec4<f32>,
};

struct GpuGlyph {
    // x, y, half-width, half-height where it sits in the artwork
    slot: vec4<f32>,
    // risen x, risen y, strength of the copy, progress — un-rotated space
    risen: vec4<f32>,
    // sin, cos of the angle the glyph's layer has turned to, that layer's index
    turn: vec4<f32>,
};

// Mirrors `gpu::Uniforms`; see the tests in `gpu/uniforms.rs`.
struct Uniforms {
    center: vec2<f32>,
    // flip height (0: y already runs down), encode sRGB (0: the target does it)
    host: vec2<f32>,
    // origin x, origin y, side, unused — the square every layer texture covers
    frame: vec4<f32>,
    // scale, offset.x, offset.y, live layers
    fit: vec4<f32>,
    // supersample, disc radius, glow pyramid top level, artwork mip level
    quality: vec4<f32>,
    // live pulses, live flares, live limbs, live glyphs
    live: vec4<f32>,
    // collapse flash, layer-tint strength, explosion flash, seconds since start
    look: vec4<f32>,
    // shimmer strength, shimmer revolutions per second, haze, live waves
    ambient: vec4<f32>,
    // live swaps, breath strength, breath period, build progress (1 or more: built)
    rhythm: vec4<f32>,
    // live threads, live ripples, live marks, echo strength
    live2: vec4<f32>,
    // x, y, presence 0..1, unused — the pointer, un-rotated
    pointer: vec4<f32>,
    // (min x, min y, max x, max y) around the lit glyphs in the artwork
    gbox: vec4<f32>,
    // ...and around the risen copies, in un-rotated space
    gbox_p: vec4<f32>,
    background: vec4<f32>,
    // The per-effect arrays, each element's vec4s laid end to end. The same bytes as an
    // array of the structs above, but Qt's OpenGL backend sets a uniform block member by
    // member and cannot see inside an array of structs; `uni_pulse(i)` and the rest below
    // put the structs back together.
    pulses: array<vec4<f32>, MAX_PULSES * 2u>,
    flares: array<vec4<f32>, MAX_FLARES>,
    limbs: array<vec4<f32>, MAX_BOLT_SEGS * 2u>,
    glyphs: array<vec4<f32>, MAX_GLYPHS * 3u>,
    waves: array<vec4<f32>, MAX_WAVES * 2u>,
    swaps: array<vec4<f32>, MAX_SWAPS * 3u>,
    threads: array<vec4<f32>, MAX_THREADS * 2u>,
    ripples: array<vec4<f32>, MAX_RIPPLES>,
    marks: array<vec4<f32>, MAX_MARKS * 2u>,
    layers: array<vec4<f32>, MAX_LAYERS * 3u>,
};

@group(0) @binding(0) var<uniform> u: Uniforms;

// Element `i` of each array in the uniform block, as its struct.
fn uni_pulse(i: u32) -> GpuPulse { return GpuPulse(u.pulses[2u * i], u.pulses[2u * i + 1u]); }
fn uni_flare(i: u32) -> GpuFlare { return GpuFlare(u.flares[i]); }
fn uni_limb(i: u32) -> GpuLimb { return GpuLimb(u.limbs[2u * i], u.limbs[2u * i + 1u]); }
fn uni_glyph(i: u32) -> GpuGlyph { return GpuGlyph(u.glyphs[3u * i], u.glyphs[3u * i + 1u], u.glyphs[3u * i + 2u]); }
fn uni_wave(i: u32) -> GpuWave { return GpuWave(u.waves[2u * i], u.waves[2u * i + 1u]); }
fn uni_swap(i: u32) -> GpuSwap { return GpuSwap(u.swaps[3u * i], u.swaps[3u * i + 1u], u.swaps[3u * i + 2u]); }
fn uni_thread(i: u32) -> GpuThread { return GpuThread(u.threads[2u * i], u.threads[2u * i + 1u]); }
fn uni_ripple(i: u32) -> GpuRipple { return GpuRipple(u.ripples[i]); }
fn uni_mark(i: u32) -> GpuMark { return GpuMark(u.marks[2u * i], u.marks[2u * i + 1u]); }
fn uni_layer(i: u32) -> GpuLayer { return GpuLayer(u.layers[3u * i], u.layers[3u * i + 1u], u.layers[3u * i + 2u]); }
// One slice per layer: premultiplied, transparent where the layer has no ink.
@group(0) @binding(1) var art_tex: texture_2d_array<f32>;
@group(0) @binding(2) var art_smp: sampler;
// Each layer's highlight, box-filtered into a mip chain. See gpu::glow_pyramid.
@group(0) @binding(3) var glow_tex: texture_2d_array<f32>;
// When each part of each layer appears as the figure builds itself. See figure::build.
@group(0) @binding(4) var reveal_tex: texture_2d_array<f32>;

const LUMA: vec3<f32> = vec3<f32>(0.299, 0.587, 0.114);

// Where canvas point `p` falls in the layer textures.
fn art_uv(p: vec2<f32>) -> vec2<f32> {
    return (p - u.frame.xy) / u.frame.z;
}

// Layer `k` at canvas point `p` (in that layer's own, un-turned frame), premultiplied.
fn art(p: vec2<f32>, k: u32, lod: f32) -> vec4<f32> {
    return textureSampleLevel(art_tex, art_smp, art_uv(p), k, lod);
}

// One distinct colour per layer, bottom first, matching `figure::LAYER_TINT` and the
// legend the viewer prints.
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

// A repeatable pseudo-random value in 0..1 for each whole-number cell.
fn hash2(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

// Smooth value noise in 0..1: random at each whole-number point, eased between them.
fn noise2(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash2(i);
    let b = hash2(i + vec2<f32>(1.0, 0.0));
    let c = hash2(i + vec2<f32>(0.0, 1.0));
    let d = hash2(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}
