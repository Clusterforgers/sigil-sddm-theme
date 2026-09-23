//! Real-time viewer: the sigil turns at a fixed rate; keys bloom parts of it.
//!
//! Renders with the same inverse map as the offline path, but in a fragment shader
//! (`shaders/spin.wgsl`), so the whole disc redraws every frame at display rate.
//!
//! Layer speeds are fixed, taken from layers.json so the window matches what the GIF
//! renders. Keys do not steer the motion; they bloom a layer, which fades on its own.

use imagespin::config::{self, parse_hex};
use imagespin::geom::Boundary;

use std::sync::Arc;
use std::time::Instant;

use wgpu::util::DeviceExt;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, Event, KeyEvent, WindowEvent};
use winit::event_loop::EventLoop;
use winit::keyboard::{Key, NamedKey};
use winit::window::WindowBuilder;

const MAX_LAYERS: usize = 16;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Default)]
struct GpuLayer {
    outer: [f32; 4],
    outer_x: [f32; 4],
    inner: [f32; 4],
    inner_x: [f32; 4],
    motion: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    center: [f32; 2],
    src_size: [f32; 2],
    fit: [f32; 4],
    params: [f32; 4],
    background: [f32; 4],
    layers: [GpuLayer; MAX_LAYERS],
}

/// Pack a boundary into the shader's (kind, sides, radius, phase) + extra layout.
fn pack(b: Boundary) -> ([f32; 4], [f32; 4]) {
    match b {
        Boundary::Circle(r) => ([0.0, r, 0.0, 0.0], [0.0; 4]),
        Boundary::Poly(n, r, ph) => ([1.0, n, r, ph], [0.0; 4]),
        Boundary::Star(n, ro, ri, ph) => ([2.0, n, ro, ri], [ph, 0.0, 0.0, 0.0]),
    }
}

/// Seconds for a bloom to fade to ~2% of its peak.
const BLOOM_DECAY: f32 = 0.55;

struct State {
    /// Revolutions per second, signed, fixed for the whole session.
    speed: Vec<f32>,
    angle: Vec<f32>,
    /// Per-layer glow, 1.0 on a keypress, decaying toward 0.
    bloom: Vec<f32>,
    names: Vec<String>,
    ss: u32,
}

impl State {
    fn describe(&self) {
        println!("\n  layers (speeds are fixed; edit layers.json to change them)");
        for (i, name) in self.names.iter().enumerate() {
            let s = self.speed[i];
            let period = if s.abs() < 1e-4 {
                "static".to_string()
            } else {
                format!("{:.1}s/rev {}", 1.0 / s.abs(), if s > 0.0 { "CW" } else { "CCW" })
            };
            println!("    {}  {:<18}  {:>6.3} rev/s   {}", i + 1, name, s, period);
        }
        println!("\n  press any key to bloom a layer — 1-{} pick one, Space blooms all",
            self.names.len().min(9));
    }

    /// Light up layer `i`.
    fn flash(&mut self, i: usize) {
        if let Some(b) = self.bloom.get_mut(i) {
            *b = 1.0;
        }
    }

    /// Which layer a non-numeric key lights up. Stable per key, spread across layers.
    fn layer_for_key(&self, c: char) -> usize {
        let n = self.names.len().max(1);
        (c as usize).wrapping_mul(2654435761) % n
    }
}

fn help() {
    println!(
        r#"
  imagespin live — the sigil turns at a fixed rate; keys make it bloom

    any key      bloom a layer
    1-9          bloom that specific layer
    Space        bloom every layer at once
    - / =        supersampling down / up (antialiasing vs framerate)
    F1           this help
    Esc          quit
"#
    );
}

fn main() {
    env_logger::init();
    let cfg_path = std::env::args().nth(1).unwrap_or_else(|| "layers.json".into());
    let (cfg, src) = config::load(&cfg_path);

    // Match the offline renderer: outermost layer first, so the first region hit wins.
    let mut order: Vec<usize> = (0..cfg.layers.len()).collect();
    order.sort_by(|&a, &b| {
        cfg.layers[b]
            .outer
            .max_radius()
            .partial_cmp(&cfg.layers[a].outer.max_radius())
            .unwrap()
    });
    let layers: Vec<_> = order.into_iter().map(|i| cfg.layers[i].clone()).collect();
    assert!(layers.len() <= MAX_LAYERS, "at most {MAX_LAYERS} layers");

    // Fixed for the session: the same speeds the GIF renders at.
    let fps = cfg.fps.max(1) as f32;
    let loop_secs = cfg.frames as f32 / fps;
    let mut st = State {
        speed: layers.iter().map(|l| l.turns as f32 / loop_secs).collect(),
        angle: vec![0.0; layers.len()],
        bloom: vec![0.0; layers.len()],
        names: layers.iter().map(|l| l.name.clone()).collect(),
        ss: 2,
    };

    help();
    st.describe();

    let event_loop = EventLoop::new().unwrap();
    let window = Arc::new(
        WindowBuilder::new()
            .with_title("imagespin — live")
            .with_inner_size(PhysicalSize::new(1280, 720))
            .build(&event_loop)
            .unwrap(),
    );

    let instance = wgpu::Instance::default();
    let surface = instance.create_surface(window.clone()).unwrap();
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: Some(&surface),
        force_fallback_adapter: false,
    }))
    .expect("no suitable GPU adapter");
    println!("  GPU: {}\n", adapter.get_info().name);

    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: None,
            required_features: wgpu::Features::empty(),
            // downlevel_defaults caps textures at 2048, which a HiDPI surface exceeds.
            required_limits: wgpu::Limits::downlevel_defaults()
                .using_resolution(adapter.limits()),
        },
        None,
    ))
    .unwrap();

    let caps = surface.get_capabilities(&adapter);
    let format = caps
        .formats
        .iter()
        .copied()
        .find(|f| f.is_srgb())
        .unwrap_or(caps.formats[0]);
    let mut surf_cfg = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format,
        width: window.inner_size().width.max(1),
        height: window.inner_size().height.max(1),
        present_mode: wgpu::PresentMode::AutoVsync,
        alpha_mode: caps.alpha_modes[0],
        view_formats: vec![],
        desired_maximum_frame_latency: 2,
    };
    surface.configure(&device, &surf_cfg);

    // Source image as an sRGB texture.
    let (sw, sh) = (src.width(), src.height());
    let rgba: Vec<u8> = src.pixels().flat_map(|p| [p.0[0], p.0[1], p.0[2], 255]).collect();
    let tex = device.create_texture_with_data(
        &queue,
        &wgpu::TextureDescriptor {
            label: Some("sigil"),
            size: wgpu::Extent3d { width: sw, height: sh, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        },
        wgpu::util::TextureDataOrder::LayerMajor,
        &rgba,
    );
    let view = tex.create_view(&Default::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        ..Default::default()
    });

    let bg = parse_hex(&cfg.background);
    let mut uni = Uniforms {
        center: cfg.center,
        src_size: [sw as f32, sh as f32],
        fit: [1.0, 0.0, 0.0, layers.len() as f32],
        params: [st.ss as f32, 0.0, 0.0, 0.0],
        // The texture is sRGB, so linearise the background to match what it decodes to.
        background: [
            (bg[0] as f32 / 255.0).powf(2.2),
            (bg[1] as f32 / 255.0).powf(2.2),
            (bg[2] as f32 / 255.0).powf(2.2),
            1.0,
        ],
        layers: [GpuLayer::default(); MAX_LAYERS],
    };
    for (i, l) in layers.iter().enumerate() {
        let (o, ox) = pack(l.outer);
        let (n, nx) = pack(l.inner);
        uni.layers[i] = GpuLayer { outer: o, outer_x: ox, inner: n, inner_x: nx, motion: [0.0; 4] };
    }

    let ubuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("uniforms"),
        contents: bytemuck::bytes_of(&uni),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("spin"),
        source: wgpu::ShaderSource::Wgsl(include_str!("../../shaders/spin.wgsl").into()),
    });

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
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    });
    let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &bgl,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: ubuf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&view) },
            wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&sampler) },
        ],
    });

    let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[&bgl],
        push_constant_ranges: &[],
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: Some(&pl),
        vertex: wgpu::VertexState { module: &shader, entry_point: "vs", buffers: &[] },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: "fs",
            targets: &[Some(format.into())],
        }),
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview: None,
    });

    let mut last = Instant::now();
    let mut fps_t = Instant::now();
    let mut fps_n = 0u32;

    event_loop
        .run(move |event, elwt| {
            elwt.set_control_flow(winit::event_loop::ControlFlow::Poll);
            match event {
                Event::WindowEvent { event, .. } => match event {
                    WindowEvent::CloseRequested => elwt.exit(),

                    WindowEvent::Resized(size) => {
                        surf_cfg.width = size.width.max(1);
                        surf_cfg.height = size.height.max(1);
                        surface.configure(&device, &surf_cfg);
                    }

                    WindowEvent::KeyboardInput {
                        event: KeyEvent { logical_key, state: ElementState::Pressed, .. },
                        ..
                    } => {
                        // Nothing here changes how fast anything turns — keys only bloom.
                        let n = layers.len();
                        match logical_key.as_ref() {
                            Key::Named(NamedKey::Escape) => elwt.exit(),
                            Key::Named(NamedKey::F1) => help(),

                            // Supersampling is a render-quality knob, not a motion one.
                            Key::Character("-") => st.ss = st.ss.saturating_sub(1).max(1),
                            Key::Character("=") | Key::Character("+") => {
                                st.ss = (st.ss + 1).min(4)
                            }

                            Key::Named(NamedKey::Space) => {
                                for i in 0..n {
                                    st.flash(i);
                                }
                            }

                            // A digit picks its layer; any other character gets one of its own.
                            Key::Character(s) => {
                                for c in s.chars() {
                                    match c.to_digit(10) {
                                        Some(d) if d >= 1 && (d as usize) <= n => {
                                            st.flash(d as usize - 1)
                                        }
                                        _ => {
                                            let i = st.layer_for_key(c);
                                            st.flash(i)
                                        }
                                    }
                                }
                            }

                            // Every remaining key still blooms something.
                            _ => st.flash(n / 2),
                        }
                    }

                    WindowEvent::RedrawRequested => {
                        let now = Instant::now();
                        let dt = (now - last).as_secs_f32();
                        last = now;

                        // Constant rate, always.
                        for (a, s) in st.angle.iter_mut().zip(&st.speed) {
                            *a += s * dt * std::f32::consts::TAU;
                        }
                        // Exponential fade, so a bloom decays at the same rate whatever
                        // the framerate.
                        let fade = (-dt / (BLOOM_DECAY / 4.0)).exp();
                        for b in st.bloom.iter_mut() {
                            *b *= fade;
                            if *b < 0.002 {
                                *b = 0.0;
                            }
                        }

                        // Fit the source into the window, preserving aspect.
                        let (w, h) = (surf_cfg.width as f32, surf_cfg.height as f32);
                        let scale = (w / sw as f32).min(h / sh as f32);
                        uni.fit = [scale, (w - sw as f32 * scale) * 0.5, (h - sh as f32 * scale) * 0.5, layers.len() as f32];
                        uni.params[0] = st.ss as f32;
                        for i in 0..layers.len() {
                            uni.layers[i].motion[0] = st.angle[i];
                            uni.layers[i].motion[1] = st.bloom[i];
                        }
                        queue.write_buffer(&ubuf, 0, bytemuck::bytes_of(&uni));

                        let frame = match surface.get_current_texture() {
                            Ok(f) => f,
                            Err(_) => {
                                surface.configure(&device, &surf_cfg);
                                return;
                            }
                        };
                        let v = frame.texture.create_view(&Default::default());
                        let mut enc = device.create_command_encoder(&Default::default());
                        {
                            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                                label: None,
                                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                    view: &v,
                                    resolve_target: None,
                                    ops: wgpu::Operations {
                                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                                        store: wgpu::StoreOp::Store,
                                    },
                                })],
                                depth_stencil_attachment: None,
                                timestamp_writes: None,
                                occlusion_query_set: None,
                            });
                            rp.set_pipeline(&pipeline);
                            rp.set_bind_group(0, &bind, &[]);
                            rp.draw(0..3, 0..1);
                        }
                        queue.submit(Some(enc.finish()));
                        frame.present();

                        fps_n += 1;
                        if fps_t.elapsed().as_secs_f32() >= 2.0 {
                            window.set_title(&format!(
                                "imagespin — live   {:.0} fps   {}x{} ss",
                                fps_n as f32 / fps_t.elapsed().as_secs_f32(),
                                st.ss, st.ss
                            ));
                            fps_n = 0;
                            fps_t = Instant::now();
                        }
                        window.request_redraw();
                    }
                    _ => {}
                },
                _ => {}
            }
        })
        .unwrap();
}
