// Blood: rings that drop into the middle, or close in from the rim and land.

// Arterial blood: dark, and with almost nothing left in green or blue. It is darker than
// the gold it runs through, which is why `shade` mixes it over the stroke rather than
// only adding — an additive term can brighten but never deepen.
const BLOOD: vec3<f32> = vec3<f32>(0.42, 0.012, 0.012);

// ...and the scarlet its lit rim runs at. Blood in bulk is nearly black; what makes a
// wave of it readable is the light caught along its leading edge, not the bulk behind.
const BLOOD_HOT: vec3<f32> = vec3<f32>(0.90, 0.06, 0.03);

// How far the wavefront displaces what lies under it, in canvas units. Each layer is
// warped from its own texture, so no shift can tear a line between layers. Keep it below
// `LAYER_REACH` in spin.wgsl, which is how far past its ink a layer is still sampled.
const WARP_GAIN: f32 = 9.0;

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
    let n = u32(u.live.x);
    for (var i = 0u; i < n; i = i + 1u) {
        let p = u.pulses[i].wave;
        let x = u.pulses[i].splash;

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
