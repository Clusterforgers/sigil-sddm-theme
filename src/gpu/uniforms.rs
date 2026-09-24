use super::color::srgb_to_linear;
use super::pyramid::glow_levels;
use crate::figure::Rendered;

pub const MAX_LAYERS: usize = 16;

/// Blood-red rings travelling out from the centre that can be in flight at once.
pub const MAX_PULSES: usize = 16;

/// Afterglows left where lightning struck that can be alight at once.
pub const MAX_FLARES: usize = 16;

/// Glyphs that can be lit and lifted off the plate at once.
pub const MAX_GLYPHS: usize = 32;

/// Limbs of lightning, across every live bolt, that can be drawn at once.
///
/// Large enough for the fan an implosion throws out of the centre, which is far more
/// lightning at once than the wandering strikes ever produce.
pub const MAX_BOLT_SEGS: usize = 96;

/// Colour waves that can be rolling out at once.
pub const MAX_WAVES: usize = 4;

/// Letters that can be scrambling at once.
pub const MAX_SWAPS: usize = 12;

/// Constellation threads, across every live constellation, that can be drawn at once.
pub const MAX_THREADS: usize = 16;

/// Ripples from the pointer that can be spreading at once.
pub const MAX_RIPPLES: usize = 8;

/// Letters that typing can have lit at once.
pub const MAX_MARKS: usize = 24;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Default)]
pub struct GpuLayer {
    /// (angle in radians, bloom 0..1, sin(angle), cos(angle))
    pub motion: [f32; 4],
    /// (nearest ink, furthest ink, unused, unused) from the centre; outside it the layer
    /// is skipped without sampling.
    pub extent: [f32; 4],
    /// (scale, opacity, dissolve, echo lag): how the layer is drawn right now. (1, 1, 0, 0)
    /// at rest; the surge flings layers outward and fades them, a dissolve burns them away,
    /// and a fast spin leaves echoes trailing this many radians behind.
    pub form: [f32; 4],
}

/// One ring of blood, in canvas units.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Default)]
pub struct GpuPulse {
    /// (crest radius, crest strength, envelope width, ripple wavelength)
    pub wave: [f32; 4],
    /// (splash strength, splash radius, wake direction, unused)
    pub splash: [f32; 4],
}

/// The afterglow where lightning struck: (x, y, radius, strength), in canvas units.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Default)]
pub struct GpuFlare {
    pub at: [f32; 4],
}

/// One straight limb of lightning.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Default)]
pub struct GpuLimb {
    /// (x0, y0, x1, y1), in canvas units
    pub seg: [f32; 4],
    /// (strength, half-width, unused, unused)
    pub style: [f32; 4],
}

/// A wave of colour rolling out from the centre.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Default)]
pub struct GpuWave {
    /// (front radius, width, strength, direction: 1 outward, -1 inward), in canvas units
    pub front: [f32; 4],
    /// (r, g, b, unused) in linear light
    pub color: [f32; 4],
}

/// A letter drawn in another letter's place while it scrambles.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Default)]
pub struct GpuSwap {
    /// (x, y, radius, layer) of the letter being replaced, where it sits in its layer
    pub slot: [f32; 4],
    /// (x, y, layer, strength) of the letter drawn instead
    pub source: [f32; 4],
    /// (cos, sin, scale, unused): how to turn and scale a point round the slot onto the
    /// same point round the source
    pub map: [f32; 4],
}

/// One thread of a constellation, in un-rotated canvas units.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Default)]
pub struct GpuThread {
    /// (x0, y0, x1, y1)
    pub seg: [f32; 4],
    /// (strength, where along it the spark is 0..1, unused, unused)
    pub style: [f32; 4],
}

/// A ripple spreading from where the pointer passed: (x, y, radius, strength).
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Default)]
pub struct GpuRipple {
    pub at: [f32; 4],
}

/// A letter lit by typing.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Default)]
pub struct GpuMark {
    /// (x, y, radius, layer) where it sits in its layer
    pub slot: [f32; 4],
    /// (steady glow, flash of lighting up, x, y on screen, un-rotated)
    pub glow: [f32; 4],
}

/// A glyph that has been lit and is lifting off the plate.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Default)]
pub struct GpuGlyph {
    /// (x, y, half-width, half-height) where it sits in the artwork; the risen copy works
    /// out its own offset from there.
    pub slot: [f32; 4],
    /// (risen x, risen y, how strongly the copy shows, how far through its life it is).
    /// The position is un-rotated screen space, because the copy has left its layer and
    /// must not be clipped by it.
    pub risen: [f32; 4],
    /// (sin, cos, layer, unused): which layer carries the glyph, and the angle it has
    /// turned to. The copy needs the angle to keep the letter the right way up as the
    /// layer rotates, and the slot is only emptied in that layer.
    pub turn: [f32; 4],
}

/// The whole uniform block. Mirrored by `Uniforms` in `shaders/common.wgsl`; field order,
/// array lengths and the structs above all have to agree with it exactly.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Uniforms {
    /// Centre of the figure, in canvas units.
    pub center: [f32; 2],
    pub _pad: [f32; 2],
    /// (origin x, origin y, side, unused): the square every layer texture covers.
    pub frame: [f32; 4],
    /// (scale, offset x, offset y, live layers): how the canvas is fitted to the window.
    pub fit: [f32; 4],
    /// (supersampling, disc radius, glow pyramid top level, artwork mip level)
    pub quality: [f32; 4],
    /// (live pulses, live flares, live limbs, live glyphs)
    pub live: [f32; 4],
    /// (collapse flash, layer-tint strength, explosion flash, seconds since start)
    pub look: [f32; 4],
    /// (shimmer strength, shimmer revolutions per second, haze in canvas units, live waves)
    pub ambient: [f32; 4],
    /// (live swaps, breath strength, breath period in seconds, build progress: 0 is nothing
    /// drawn yet, 1 or more is the whole figure)
    pub rhythm: [f32; 4],
    /// (live threads, live ripples, live marks, echo strength)
    pub live2: [f32; 4],
    /// (x, y, presence 0..1, unused): the pointer, in un-rotated canvas units
    pub pointer: [f32; 4],
    /// (min x, min y, max x, max y) around the lit glyphs where they sit in the artwork,
    /// for skipping the pass that hides them.
    pub gbox: [f32; 4],
    /// ...and the same around the risen copies, in un-rotated screen space.
    pub gbox_p: [f32; 4],
    /// The background colour, linearised to match what the GPU sees when it samples.
    pub background: [f32; 4],
    pub pulses: [GpuPulse; MAX_PULSES],
    pub flares: [GpuFlare; MAX_FLARES],
    pub limbs: [GpuLimb; MAX_BOLT_SEGS],
    pub glyphs: [GpuGlyph; MAX_GLYPHS],
    pub waves: [GpuWave; MAX_WAVES],
    pub swaps: [GpuSwap; MAX_SWAPS],
    pub threads: [GpuThread; MAX_THREADS],
    pub ripples: [GpuRipple; MAX_RIPPLES],
    pub marks: [GpuMark; MAX_MARKS],
    pub layers: [GpuLayer; MAX_LAYERS],
}

/// Everything in the uniform block that does not change from frame to frame.
pub fn uniforms(fig: &Rendered, ss: u32) -> Uniforms {
    assert!(fig.layers.len() <= MAX_LAYERS, "at most {MAX_LAYERS} layers");
    let [r, g, b] = fig.background.0;
    let px = fig.layers.first().map_or(1, |l| l.image.width());

    let mut uni: Uniforms = bytemuck::Zeroable::zeroed();
    uni.center = fig.center;
    uni.frame = [fig.frame.origin[0], fig.frame.origin[1], fig.frame.side, 0.0];
    uni.fit = [1.0, 0.0, 0.0, fig.layers.len() as f32];
    // Fully built until someone says otherwise: nothing here is drawing itself in.
    uni.rhythm[3] = 1.0;
    // Nothing is drawn past the outermost ink, so the shader can stop there.
    uni.quality = [ss as f32, fig.disc(), (glow_levels(px, px) - 1) as f32, 0.0];
    // The textures are sRGB, so linearise the background to match what they decode to.
    uni.background = [srgb_to_linear(r), srgb_to_linear(g), srgb_to_linear(b), 1.0];
    for (slot, l) in uni.layers.iter_mut().zip(&fig.layers) {
        slot.extent = [l.extent[0], l.extent[1], 0.0, 0.0];
        slot.form = [1.0, 1.0, 0.0, 0.0];
    }
    uni
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The uniform block has to fit what `downlevel_defaults` guarantees, which is the
    /// limit the viewer asks for. It has grown every time an effect was added, so this
    /// is worth pinning rather than finding out on someone else's GPU.
    #[test]
    fn uniforms_fit_the_downlevel_limit() {
        let size = std::mem::size_of::<Uniforms>();
        let limit = wgpu::Limits::downlevel_defaults().max_uniform_buffer_binding_size as usize;
        assert!(size <= limit, "uniform block is {size} bytes, limit is {limit}");
    }

    /// The WGSL declares its own copy of these array lengths, and the two `Uniforms`
    /// layouts have to agree exactly — if they drift, the shader reads every field after
    /// the short array from the wrong offset and the disc simply stops being drawn, with
    /// no error anywhere. That has happened twice; hence this.
    #[test]
    fn shader_array_lengths_match_the_rust_ones() {
        let src = super::super::shader::SOURCE;
        let declared = |name: &str| -> usize {
            let pat = format!("const {name}: u32 = ");
            let at = src.find(&pat).unwrap_or_else(|| panic!("{name} not declared in WGSL"));
            let rest = &src[at + pat.len()..];
            let end = rest.find('u').expect("WGSL u32 literal");
            rest[..end].trim().parse().expect("WGSL u32 literal")
        };
        assert_eq!(declared("MAX_LAYERS"), MAX_LAYERS, "MAX_LAYERS");
        assert_eq!(declared("MAX_PULSES"), MAX_PULSES, "MAX_PULSES");
        assert_eq!(declared("MAX_FLARES"), MAX_FLARES, "MAX_FLARES");
        assert_eq!(declared("MAX_BOLT_SEGS"), MAX_BOLT_SEGS, "MAX_BOLT_SEGS");
        assert_eq!(declared("MAX_GLYPHS"), MAX_GLYPHS, "MAX_GLYPHS");
        assert_eq!(declared("MAX_WAVES"), MAX_WAVES, "MAX_WAVES");
        assert_eq!(declared("MAX_SWAPS"), MAX_SWAPS, "MAX_SWAPS");
        assert_eq!(declared("MAX_THREADS"), MAX_THREADS, "MAX_THREADS");
        assert_eq!(declared("MAX_RIPPLES"), MAX_RIPPLES, "MAX_RIPPLES");
        assert_eq!(declared("MAX_MARKS"), MAX_MARKS, "MAX_MARKS");
    }

    /// Every piece of WGSL is valid on its own terms once assembled, and the uniform block
    /// it declares is exactly as large as the Rust one. Catches a field added on one side
    /// only, which the array-length test cannot see.
    #[test]
    fn shader_parses_and_its_uniform_block_matches() {
        let module = naga::front::wgsl::parse_str(super::super::shader::SOURCE)
            .unwrap_or_else(|e| panic!("{}", e.emit_to_string(super::super::shader::SOURCE)));
        let mut v = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        );
        v.validate(&module).expect("WGSL does not validate");

        let (_, ty) = module
            .types
            .iter()
            .find(|(_, t)| t.name.as_deref() == Some("Uniforms"))
            .expect("no Uniforms struct in WGSL");
        let naga::TypeInner::Struct { span, .. } = ty.inner else { panic!("Uniforms is not a struct") };
        assert_eq!(span as usize, std::mem::size_of::<Uniforms>(), "WGSL and Rust Uniforms differ in size");
    }
}
