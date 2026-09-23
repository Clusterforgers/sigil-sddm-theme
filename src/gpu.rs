//! The GPU side of the real-time renderer: uniform layout, textures, pipeline.
//!
//! Shared by the viewer (`src/bin/live.rs`) and the benchmark (`src/bin/bench.rs`) so
//! both measure and draw exactly the same thing. The shader itself is
//! `shaders/spin.wgsl`; this module only feeds it.

use crate::config::{parse_hex, Config};
use crate::geom::{Boundary, Layer};

use image::RgbImage;
use wgpu::util::DeviceExt;

pub const MAX_LAYERS: usize = 16;

/// Blood-red rings travelling out from the centre that can be in flight at once.
pub const MAX_PULSES: usize = 8;

/// Afterglows left where lightning struck that can be alight at once.
pub const MAX_FLARES: usize = 8;

/// Glyphs that can be lit and lifted off the plate at once.
pub const MAX_GLYPHS: usize = 16;

/// Limbs of lightning/// Limbs of lightning — across every live bolt — that can be drawn at once.
///
/// Large enough for the fan an implosion throws out of the centre, which is far more
/// lightning at once than the wandering strikes ever produce.
pub const MAX_BOLT_SEGS: usize = 96;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Default)]
pub struct GpuLayer {
    pub outer: [f32; 4],
    pub outer_x: [f32; 4],
    pub inner: [f32; 4],
    pub inner_x: [f32; 4],
    pub motion: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Uniforms {
    pub center: [f32; 2],
    pub src_size: [f32; 2],
    /// scale, offset.x, offset.y, n_layers
    pub fit: [f32; 4],
    /// supersample, disc radius, glow pyramid top LOD, live lit glyphs
    pub params: [f32; 4],
    /// live pulses, live flares, live bolt segments, collapse flash
    pub counts: [f32; 4],
    /// (mip level to read the artwork at, unused, unused, unused)
    pub misc: [f32; 4],
    /// (min x, min y, max x, max y) around the lit glyphs where they sit in the
    /// artwork, for skipping the pass that hides them
    pub gbox: [f32; 4],
    /// ...and the same around the risen copies, in un-rotated screen space
    pub gbox_p: [f32; 4],
    pub background: [f32; 4],
    /// (crest radius, crest strength, envelope width, ripple wavelength) per pulse,
    /// in source pixels
    pub pulses: [[f32; 4]; MAX_PULSES],
    /// (splash strength, splash radius, wake direction, unused) for the pulse at the
    /// same index
    pub pulse_x: [[f32; 4]; MAX_PULSES],
    /// (x, y, radius, strength) per flare, in source pixels
    pub flares: [[f32; 4]; MAX_FLARES],
    /// (x0, y0, x1, y1) per lightning limb, in source pixels
    pub bolts: [[f32; 4]; MAX_BOLT_SEGS],
    /// (strength, half-width, unused, unused) for the limb at the same index
    pub bolt_w: [[f32; 4]; MAX_BOLT_SEGS],
    /// (x, y, half-width, half-height) per lit glyph, at the position it actually
    /// occupies in the artwork; the risen copy works out its own offset from there
    pub glyphs: [[f32; 4]; MAX_GLYPHS],
    /// (risen x, risen y, how strongly the copy shows, how far through its life it
    /// is). The position is un-rotated screen space, because the copy has left its layer
    /// and must not be clipped by it.
    pub glyph_a: [[f32; 4]; MAX_GLYPHS],
    /// (sin, cos, unused, unused) of the angle that glyph's own layer has turned to. The
    /// copy needs it to keep the letter the right way up as the layer rotates.
    pub glyph_r: [[f32; 4]; MAX_GLYPHS],
    pub layers: [GpuLayer; MAX_LAYERS],
}

/// Pack a boundary into the shader's layout:
///
/// * `b`     = (kind, sides, radius, phase_rad | star inner radius)
/// * `extra` = (star phase_rad, min radius, max radius, unused)
///
/// `extra.yz` bracket the boundary over every angle. The shader tests a pixel's radius
/// against that bracket first, and only falls back to evaluating the real boundary when
/// the bracket leaves the answer open — which for most pixels it does not.
///
/// Phases are converted to radians here so the shader never has to.
pub fn pack(b: Boundary) -> ([f32; 4], [f32; 4]) {
    let (lo, hi) = (b.min_radius(), b.max_radius());
    match b {
        Boundary::Circle(r) => ([0.0, r, 0.0, 0.0], [0.0, lo, hi, 0.0]),
        Boundary::Poly(n, r, ph) => ([1.0, n, r, ph.to_radians()], [0.0, lo, hi, 0.0]),
        Boundary::Star(n, ro, ri, ph) => ([2.0, n, ro, ri], [ph.to_radians(), lo, hi, 0.0]),
    }
}

/// Layers sorted the way the offline renderer walks them: outermost first, so the first
/// region a pixel falls in wins.
pub fn ordered_layers(cfg: &Config) -> Vec<Layer> {
    let mut order: Vec<usize> = (0..cfg.layers.len()).collect();
    order.sort_by(|&a, &b| {
        cfg.layers[b]
            .outer
            .max_radius()
            .partial_cmp(&cfg.layers[a].outer.max_radius())
            .unwrap()
    });
    order.into_iter().map(|i| cfg.layers[i].clone()).collect()
}

/// Everything in the uniform block that does not change from frame to frame.
/// `canvas` is the texture that will be uploaded; `nominal` is the coordinate space
/// `layers.json` is written in. They differ once the linework is drawn over the photograph
/// at a multiple of its size — the texture gets denser, the coordinates do not move. Every
/// lookup is in normalised uv, so nothing downstream needs to know which is which.
pub fn uniforms(
    cfg: &Config,
    layers: &[Layer],
    canvas: &RgbImage,
    nominal: [f32; 2],
    ss: u32,
) -> Uniforms {
    assert!(layers.len() <= MAX_LAYERS, "at most {MAX_LAYERS} layers");
    let bg = parse_hex(&cfg.background);
    // Nothing is drawn past the outermost boundary, so the shader can stop there.
    let disc = layers.iter().map(|l| l.outer.max_radius()).fold(0.0f32, f32::max);

    let mut uni = Uniforms {
        center: cfg.center,
        src_size: nominal,
        fit: [1.0, 0.0, 0.0, layers.len() as f32],
        params: [
            ss as f32,
            disc,
            (glow_levels(canvas.width(), canvas.height()) - 1) as f32,
            0.0,
        ],
        counts: [0.0; 4],
        misc: [0.0; 4],
        gbox: [0.0; 4],
        gbox_p: [0.0; 4],
        // The texture is sRGB, so linearise the background to match what it decodes to.
        background: [
            srgb_to_linear(bg[0]),
            srgb_to_linear(bg[1]),
            srgb_to_linear(bg[2]),
            1.0,
        ],
        pulses: [[0.0; 4]; MAX_PULSES],
        pulse_x: [[0.0; 4]; MAX_PULSES],
        flares: [[0.0; 4]; MAX_FLARES],
        bolts: [[0.0; 4]; MAX_BOLT_SEGS],
        bolt_w: [[0.0; 4]; MAX_BOLT_SEGS],
        glyphs: [[0.0; 4]; MAX_GLYPHS],
        glyph_a: [[0.0; 4]; MAX_GLYPHS],
        glyph_r: [[0.0; 4]; MAX_GLYPHS],
        layers: [GpuLayer::default(); MAX_LAYERS],
    };
    for (i, l) in layers.iter().enumerate() {
        let (o, ox) = pack(l.outer);
        let (n, nx) = pack(l.inner);
        uni.layers[i] = GpuLayer { outer: o, outer_x: ox, inner: n, inner_x: nx, motion: [0.0; 4] };
    }
    uni
}

/// The sRGB electro-optical transfer function, matching what the GPU applies when it
/// samples an `Rgba8UnormSrgb` texture.
fn srgb_to_linear(v: u8) -> f32 {
    let c = v as f32 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// How much light a pixel contributes to the bloom — the shader's old `highlight()`,
/// evaluated once on the CPU instead of 24 times per pixel per frame.
fn highlight(px: &image::Rgb<u8>) -> f32 {
    let (r, g, b) = (srgb_to_linear(px.0[0]), srgb_to_linear(px.0[1]), srgb_to_linear(px.0[2]));
    (0.299 * r + 0.587 * g + 0.114 * b - 0.12).max(0.0)
}

/// Number of levels in the glow pyramid for a source of this size.
pub fn glow_levels(w: u32, h: u32) -> u32 {
    32 - w.max(h).max(1).leading_zeros()
}

/// The inverse of `srgb_to_linear`, for writing filtered values back into an sRGB texture.
fn linear_to_srgb(v: f32) -> u8 {
    let c = v.clamp(0.0, 1.0);
    let s = if c <= 0.003_130_8 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
    (s * 255.0).round() as u8
}

/// The artwork and its mip chain, as RGBA, concatenated smallest-last.
///
/// Without this the viewer samples the artwork at level 0 whatever the window is doing, and
/// at the sizes people actually use the disc is *minified* — at 714x427 the fit scale is
/// 0.357, so even at `ss=2` that is 0.71 texels per sample. Thin gold lines then get point
/// sampled and crawl. A prefiltered chain costs nothing when the disc is magnified and is
/// the whole difference when it is not.
///
/// The filtering happens in **linear** light. The texture is `Rgba8UnormSrgb`, so averaging
/// the stored bytes would be averaging the wrong quantity and would quietly shift the
/// artwork's brightness as it shrank.
pub fn source_pyramid(src: &RgbImage) -> (Vec<u8>, u32) {
    let (mut w, mut h) = (src.width(), src.height());
    // Work in linear light, three channels, and only encode on the way out.
    let mut level: Vec<[f32; 3]> = src
        .pixels()
        .map(|p| [srgb_to_linear(p.0[0]), srgb_to_linear(p.0[1]), srgb_to_linear(p.0[2])])
        .collect();

    let encode = |lv: &[[f32; 3]]| -> Vec<u8> {
        lv.iter()
            .flat_map(|c| {
                [linear_to_srgb(c[0]), linear_to_srgb(c[1]), linear_to_srgb(c[2]), 255]
            })
            .collect::<Vec<u8>>()
    };

    let mut out = encode(&level);
    let mut levels = 1u32;
    while w > 1 || h > 1 {
        let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
        let mut next = vec![[0.0f32; 3]; (nw * nh) as usize];
        for y in 0..nh {
            for x in 0..nw {
                // Box filter, clamping at odd edges so no source pixel is dropped.
                let (x0, y0) = (2 * x, 2 * y);
                let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
                let at = |px: u32, py: u32| level[(py * w + px) as usize];
                let (a, b, c, d) = (at(x0, y0), at(x1, y0), at(x0, y1), at(x1, y1));
                for k in 0..3 {
                    next[(y * nw + x) as usize][k] = (a[k] + b[k] + c[k] + d[k]) * 0.25;
                }
            }
        }
        out.extend_from_slice(&encode(&next));
        level = next;
        w = nw;
        h = nh;
        levels += 1;
    }
    (out, levels)
}

/// The highlight map and its box-filtered mip chain, concatenated smallest-last.
///
/// This is the whole point of the fast bloom: mip level L holds the average highlight
/// over a 2^L box, which is the neighbourhood gather the viewer used to brute-force with
/// two rings of taps every frame. Built once, it turns that gather into a few bilinear
/// fetches at a level chosen from the bloom radius.
///
/// Returns the packed levels and how many there are.
pub fn glow_pyramid(src: &RgbImage) -> (Vec<u8>, u32) {
    let (mut w, mut h) = (src.width(), src.height());
    let mut level: Vec<u8> = src.pixels().map(|p| (highlight(p) * 255.0).round() as u8).collect();

    let mut out = level.clone();
    let mut levels = 1u32;
    while w > 1 || h > 1 {
        let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
        let mut next = vec![0u8; (nw * nh) as usize];
        for y in 0..nh {
            for x in 0..nw {
                // Box filter, clamping at odd edges so no source pixel is dropped.
                let (x0, y0) = (2 * x, 2 * y);
                let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
                let at = |px: u32, py: u32| level[(py * w + px) as usize] as u32;
                next[(y * nw + x) as usize] =
                    ((at(x0, y0) + at(x1, y0) + at(x0, y1) + at(x1, y1) + 2) / 4) as u8;
            }
        }
        out.extend_from_slice(&next);
        level = next;
        w = nw;
        h = nh;
        levels += 1;
    }
    (out, levels)
}

/// The pipeline and its bindings, with no window or surface attached.
pub struct Gpu {
    pub pipeline: wgpu::RenderPipeline,
    pub bind: wgpu::BindGroup,
    pub ubuf: wgpu::Buffer,
}

impl Gpu {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        src: &RgbImage,
        uni: &Uniforms,
    ) -> Self {
        let (sw, sh) = (src.width(), src.height());

        // Source image as an sRGB texture, prefiltered so it can be minified without
        // the thin lines crawling.
        let (rgba, src_levels) = source_pyramid(src);
        let tex = device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("sigil"),
                size: wgpu::Extent3d { width: sw, height: sh, depth_or_array_layers: 1 },
                mip_level_count: src_levels,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::MipMajor,
            &rgba,
        );
        let view = tex.create_view(&Default::default());

        // Pre-blurred highlight pyramid for the bloom.
        let (glow_data, levels) = glow_pyramid(src);
        let glow = device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("glow pyramid"),
                size: wgpu::Extent3d { width: sw, height: sh, depth_or_array_layers: 1 },
                mip_level_count: levels,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                // Linear, not sRGB: these are already-linear light amounts.
                format: wgpu::TextureFormat::R8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::MipMajor,
            &glow_data,
        );
        let glow_view = glow.create_view(&Default::default());

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            // Trilinear, so the bloom can slide smoothly between pyramid levels.
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        let ubuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("uniforms"),
            contents: bytemuck::bytes_of(uni),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("spin"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/spin.wgsl").into()),
        });

        let tex_entry = |binding: u32| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                tex_entry(1),
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                tex_entry(3),
            ],
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: ubuf.as_entire_binding() },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&glow_view),
                },
            ],
        });

        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pl),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(format.into())],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });

        Gpu { pipeline, bind, ubuf }
    }

    /// Draw one full-screen pass into `target`.
    pub fn draw(&self, enc: &mut wgpu::CommandEncoder, target: &wgpu::TextureView) {
        let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        rp.set_pipeline(&self.pipeline);
        rp.set_bind_group(0, &self.bind, &[]);
        rp.draw(0..3, 0..1);
    }
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
    /// no error anywhere. That has happened once; hence this.
    #[test]
    fn shader_array_lengths_match_the_rust_ones() {
        let src = include_str!("../shaders/spin.wgsl");
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
    }
}
