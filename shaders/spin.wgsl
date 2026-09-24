// The frame itself: the layers stacked, turned, and every effect laid over them. The
// effects live in `effects/`; `common.wgsl` holds the layout.

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

// How far outside its ink a layer can still show something: the wave's warp pulls ink
// in from up to this far, and the bloom spills about as far again.
const LAYER_REACH: f32 = 16.0;

// The colour of the surge's explosion: white, a touch warm, as if the gold burned out.
const BLAST: vec3<f32> = vec3<f32>(1.0, 0.93, 0.80);

// Colour of one sub-sample, given a point in canvas coordinates.
//
// Every layer is its own image, so the layers are stacked bottom to top like sheets of
// glass, each turned by its own angle. The per-layer work is the lookup and whatever
// lights a layer's own strokes; everything that happens to a *place* — the blood, the
// lightning, the collapse flash — goes on the stack once, afterwards.
fn shade(p: vec2<f32>, f: Fields) -> vec3<f32> {
    let d = p - u.center;
    let r2 = dot(d, d);
    // Nothing is drawn past the outermost ink; most of the window is out here.
    if (r2 >= (u.quality.y + LAYER_REACH) * (u.quality.y + LAYER_REACH)) {
        return u.background.rgb;
    }
    let r = sqrt(r2);

    // The blood is a thing happening to the plate, so it stops at the plate's edge rather
    // than darkening the ground around it.
    let on_plate = 1.0 - smoothstep(u.quality.y, u.quality.y + LAYER_REACH, r);
    let red = f.blood.x * on_plate;
    let edge = f.blood.y * on_plate;
    let trough = f.blood.z * on_plate;
    let warp = f.blood.w;
    let arc = f.arc;
    let blue = arc.y + f.flare;

    var col = u.background.rgb;
    var glow = vec3<f32>(0.0);
    let n = u32(u.fit.w);
    for (var k: u32 = 0u; k < n; k = k + 1u) {
        let L = u.layers[k];

        // A layer the surge has faded out is not there at all...
        if (L.form.y <= 0.002) { continue; }
        // ...and one it has flung outward is the same layer seen at a smaller radius.
        let dl = d / L.form.x;
        let rl = r / L.form.x;

        // The radius alone rules most layers out without touching a texture.
        if (rl < L.extent.x - LAYER_REACH || rl > L.extent.y + LAYER_REACH) { continue; }

        // Un-turning the pixel by the layer's angle is just rotating the offset the other
        // way, so the precomputed sin/cos replace two more trig calls.
        let s = L.motion.z;
        let c = L.motion.w;
        let q = u.center + vec2<f32>(dl.x * c - dl.y * s, dl.y * c + dl.x * s);

        // The wave is a lens: push the point we sample along the radius by however much
        // the surface above it is tilted. Rotation preserves radius, so displacing
        // radially here needs no correction for the layer's own angle. This is what stops
        // the ring reading as a colour laid over the artwork instead of as something
        // happening to it.
        let qr = length(q - u.center);
        let qw = u.center + (q - u.center) * ((qr + warp * WARP_GAIN) / max(qr, 1e-3));

        // Read the artwork at a level matched to how hard it is being shrunk. The window
        // decides that, not the pixel, so the level is worked out once on the CPU.
        var ink = art(qw, k, u.quality.w);

        // A glyph that has lifted off leaves its slot empty; the copy is drawn somewhere
        // else entirely, at the end, in a frame this loop cannot clip. One box around the
        // lit glyphs, tested once: the groups are clusters, so almost every pixel leaves.
        if (u.live.w > 0.0 && q.x >= u.gbox.x && q.y >= u.gbox.y
            && q.x <= u.gbox.z && q.y <= u.gbox.w) {
            ink = ink * (1.0 - glyph_hide(q, k));
        }

        ink = ink * L.form.y;

        // Which layer this is, said in colour. Only the hue is replaced.
        if (u.look.y > 0.0) {
            let lum = dot(ink.rgb, LUMA);
            ink = vec4<f32>(mix(ink.rgb, layer_tint(k) * lum * 1.35, u.look.y), ink.a);
        }

        // Drive this layer's strokes brighter: its own keypress bloom, and the lightning.
        let b = L.motion.y;
        ink = vec4<f32>(ink.rgb + ink.rgb * (b * 1.3) + ink.rgb * SPARK * (blue * 2.0), ink.a);

        // Premultiplied "over".
        col = ink.rgb + col * (1.0 - ink.a);

        // ...and let the light spill off its strokes into the ground, tinted by whichever
        // source is doing the lighting. The lit rim counts for less than its brightness
        // suggests: it is a thin line, and at full weight it washes the wake out. The
        // spill samples the unwarped point: the glow is about where the artwork is, not
        // where the wave has bent it to.
        let amt = b + red + blue + edge * 0.6;
        if (amt > 0.002) {
            let tint = (GLOW * b + BLOOD * red + SPARK * blue + BLOOD_HOT * (edge * 0.6)) / amt;
            glow = glow + spill(q, k, amt, tint) * L.form.y;
        }
    }

    // The dip ahead of the crest. A ring needs a hard outer boundary to read as one, and
    // darkening is the only way to draw an edge on artwork this sparse.
    col = col * (1.0 - min(trough, 1.0) * 0.72);

    // Blood is darker than the gold it runs through, so it replaces the stroke's colour
    // rather than being added to it — and keeps rather less of the light that was there,
    // so the lit rim has something dark to stand against.
    if (red > 0.002) {
        let lum = dot(col, LUMA);
        col = mix(col, BLOOD * lum * 1.5, min(red, 1.0));
    }

    // A little lands on the ground and not just on the strokes, so a passing wave reads
    // as a wave rather than as a row of briefly brighter glyphs. Lightning lights the air
    // it crosses, so it puts down more.
    col = col + glow + BLOOD * (red * 0.05) + SPARK * (blue * 0.12);

    // The lit rim goes on last and on top: it is the one part of the wave that is
    // brighter than what it crosses, so the dark body must not mix it away.
    col = col + col * BLOOD_HOT * (edge * 0.85) + BLOOD_HOT * (edge * 0.12);

    // The whole formation answers when an implosion lands.
    col = col + col * (u.look.x * 0.9) + BLOOD_HOT * (u.look.x * 0.05);

    // The arc cores and the risen glyphs are light sources in their own right rather
    // than things lighting the artwork, so they go over the top of whatever the pixel
    // turned out to be — and outside the layer loop, so a copy that has drifted away
    // from its layer is not clipped or displaced by it.
    var risen = vec3<f32>(0.0);
    if (u.live.w > 0.0 && p.x >= u.gbox_p.x && p.y >= u.gbox_p.y
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
    let ss = i32(u.quality.x);
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
    let reach = u.quality.y + LAYER_REACH;
    if (dot(cd, cd) < reach * reach) {
        f = fields_at(centre);
    }

    for (var j = 0; j < ss; j = j + 1) {
        for (var i = 0; i < ss; i = i + 1) {
            let off = (vec2<f32>(f32(i), f32(j)) + 0.5) / f32(ss);
            // window pixel -> canvas point (aspect-preserving fit)
            let p = (base + off - u.fit.yz) / u.fit.x;
            acc = acc + shade(p, f);
        }
    }
    var col = acc / f32(ss * ss);

    // The surge's explosion: a white-hot flash, blinding at the centre and washing over
    // the whole screen, not just the disc, so it reads as a blast and not as the figure
    // lighting up.
    if (u.look.z > 0.0) {
        let fall = exp(-dot(cd, cd) / (u.quality.y * u.quality.y) * 0.6);
        col = col + BLAST * (u.look.z * (0.3 + 1.5 * fall));
    }
    return vec4<f32>(col, 1.0);
}
