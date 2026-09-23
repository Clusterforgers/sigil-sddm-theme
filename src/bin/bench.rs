//! Offscreen benchmark for the viewer's fragment shader.
//!
//! The viewer is vsync-locked and its window size is whatever the compositor gives it,
//! so neither its framerate nor its resolution is a usable measurement. This renders the
//! same pipeline into a texture at a fixed size, with no presentation and no compositor
//! involved, and reports milliseconds per frame.
//!
//! ```text
//! cargo run --release --bin bench -- --size 2880x1746 --ss 2 --bloom 1
//! ```

use imagespin::config;
use imagespin::gpu::{self, Gpu};

use std::time::Instant;

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

struct Args {
    width: u32,
    height: u32,
    ss: u32,
    /// Bloom applied to every layer, 0..1. 1 is the worst case Space produces.
    bloom: f32,
    /// Blood-red rings in flight, spread evenly across the disc.
    pulses: u32,
    /// Strike afterglows alight, scattered over the artwork.
    flares: u32,
    /// Lightning limbs in the air, laid out as spokes across the disc.
    bolts: u32,
    /// Activated glyphs, taken from the ones actually found in the artwork.
    glyphs: u32,
    /// How far the activated glyphs have floated, in source pixels.
    lift: f32,
    frames: u32,
    /// Write the rendered frame here, for checking a change did not alter the picture.
    out: Option<String>,
    config: String,
}

fn parse_args() -> Args {
    let mut a = Args {
        width: 2880,
        height: 1746,
        ss: 2,
        bloom: 0.0,
        pulses: 0,
        flares: 0,
        bolts: 0,
        glyphs: 0,
        lift: 10.0,
        frames: 120,
        out: None,
        config: "layers.json".into(),
    };
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < argv.len() {
        let val = || argv.get(i + 1).unwrap_or_else(|| panic!("{} needs a value", argv[i]));
        match argv[i].as_str() {
            "--size" => {
                let v = val();
                let (w, h) = v.split_once('x').expect("--size wants WxH");
                a.width = w.parse().unwrap();
                a.height = h.parse().unwrap();
            }
            "--ss" => a.ss = val().parse().unwrap(),
            "--bloom" => a.bloom = val().parse().unwrap(),
            "--pulses" => a.pulses = val().parse().unwrap(),
            "--flares" => a.flares = val().parse().unwrap(),
            "--bolts" => a.bolts = val().parse().unwrap(),
            "--glyphs" => a.glyphs = val().parse().unwrap(),
            "--lift" => a.lift = val().parse().unwrap(),
            "--frames" => a.frames = val().parse().unwrap(),
            "--config" => a.config = val().clone(),
            "--out" => a.out = Some(val().clone()),
            other => panic!("unknown argument {other}"),
        }
        i += 2;
    }
    a
}

fn main() {
    env_logger::init();
    let args = parse_args();
    let (cfg, src) = config::load(&args.config);
    let layers = gpu::ordered_layers(&cfg);

    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        ..Default::default()
    }))
    .expect("no suitable GPU adapter");
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: None,
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits()),
        ..Default::default()
    }))
    .unwrap();

    // The bench measures the photograph on its own, so canvas and coordinates coincide.
    let nominal = [src.width() as f32, src.height() as f32];
    let mut uni = gpu::uniforms(&cfg, &layers, &src, nominal, args.ss);
    // Same aspect-preserving fit the viewer computes for its surface.
    let (sw, sh) = (src.width() as f32, src.height() as f32);
    let (w, h) = (args.width as f32, args.height as f32);
    let scale = (w / sw).min(h / sh);
    uni.fit = [scale, (w - sw * scale) * 0.5, (h - sh * scale) * 0.5, layers.len() as f32];
    // Same rule the viewer uses, bias included — see `source_lod` in live.rs.
    uni.misc[0] = (-(scale * args.ss as f32).max(1e-3).log2() - 0.35).max(0.0);
    for i in 0..layers.len() {
        // A fixed angle off the axes, so no layer lands on a degenerate case.
        let a = 0.37 * (i + 1) as f32;
        uni.layers[i].motion = [a, args.bloom, a.sin(), a.cos()];
    }

    // Rings spread evenly across the disc, and surges ringed around it, both at full
    // strength: more overlap than the viewer ever produces at once.
    let disc = uni.params[1];
    let np = args.pulses.min(imagespin::gpu::MAX_PULSES as u32);
    uni.counts[0] = np as f32;
    for i in 0..np as usize {
        let t = (i as f32 + 0.5) / np as f32;
        uni.pulses[i] = [disc * t, 1.0, disc * 0.09, disc * 0.21];
        uni.pulse_x[i] = [0.0, disc * 0.16, 1.0, 0.0];
    }
    let nf = args.flares.min(imagespin::gpu::MAX_FLARES as u32);
    uni.counts[1] = nf as f32;
    for i in 0..nf as usize {
        let a = std::f32::consts::TAU * i as f32 / nf as f32;
        uni.flares[i] = [
            cfg.center[0] + disc * 0.55 * a.sin(),
            cfg.center[1] + disc * 0.55 * a.cos(),
            60.0,
            1.0,
        ];
    }
    // Short limbs wound out across the disc. Not a real bolt's path, but limbs of a
    // realistic length scattered over a realistic area, which is what the cost turns on:
    // a spoke spanning the whole disc would have a bounding box the culling cannot use.
    let nb = args.bolts.min(imagespin::gpu::MAX_BOLT_SEGS as u32);
    uni.counts[2] = nb as f32;
    for i in 0..nb as usize {
        let t = (i as f32 + 0.5) / nb as f32;
        let a = std::f32::consts::TAU * t * 3.0;
        let rr = disc * (0.15 + 0.72 * t);
        let dir = a + 1.1;
        let len = disc * 0.07;
        let sx = cfg.center[0] + rr * a.sin();
        let sy = cfg.center[1] + rr * a.cos();
        uni.bolts[i] = [sx, sy, sx + len * dir.sin(), sy + len * dir.cos()];
        uni.bolt_w[i] = [1.0, 3.2, 0.0, 0.0];
    }

    // A real cluster, chosen the way the viewer chooses one: a seed and its nearest
    // neighbours. Spreading them over the whole plate would measure a bounding box the
    // viewer never produces, and would not show what a group looks like either.
    let cat = imagespin::glyphs::find(&src, cfg.center, disc);
    let ng = args.glyphs.min(imagespin::gpu::MAX_GLYPHS as u32).min(cat.len() as u32);
    uni.params[3] = ng as f32;
    if ng > 0 {
        let seed = cat[cat.len() / 3];
        let mut near: Vec<(f32, usize)> = cat
            .iter()
            .enumerate()
            .map(|(i, g)| {
                let (dx, dy) = (g.pos[0] - seed.pos[0], g.pos[1] - seed.pos[1]);
                (dx * dx + dy * dy, i)
            })
            .collect();
        near.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

        // Progress is read off the lift, so a still shows the colour and spread a glyph
        // would have reached by the time it had drifted that far.
        let progress = (args.lift / 20.0).clamp(0.0, 1.0);
        let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
        let (mut pl, mut ph) = ([f32::MAX; 2], [f32::MIN; 2]);
        for (i, &(_, idx)) in near.iter().take(ng as usize).enumerate() {
            let g = cat[idx];
            let ext = g.radius + 3.0;
            uni.glyphs[i] = [g.pos[0], g.pos[1], ext, ext];

            // The bench turns the layers like the viewer does, so the glyph has to be
            // carried round by its own layer before the rise is added. Assuming artwork
            // and screen coincide here puts the copy somewhere the shader never looks.
            let (ax, ay) = (g.pos[0] - cfg.center[0], g.pos[1] - cfg.center[1]);
            let r = (ax * ax + ay * ay).sqrt();
            let alpha = (-ax).atan2(-ay);
            let li = layers.iter().position(|l| l.contains(r, alpha)).unwrap_or(0);
            let ang = 0.37 * (li + 1) as f32;
            let (s, c) = (ang.sin(), ang.cos());
            let (mut px, mut py) = (ax * c + ay * s, ay * c - ax * s);
            let len = (px * px + py * py).sqrt().max(1e-3);
            px += px * args.lift / len;
            py += py * args.lift / len;
            let risen = [cfg.center[0] + px, cfg.center[1] + py];

            uni.glyph_a[i] = [risen[0], risen[1], 1.0, progress];
            uni.glyph_r[i] = [s, c, 0.0, 0.0];
            let reach = ext * (1.0 + 1.6 * progress) + args.lift;
            for k in 0..2 {
                lo[k] = lo[k].min(g.pos[k] - reach);
                hi[k] = hi[k].max(g.pos[k] + reach);
                pl[k] = pl[k].min(risen[k] - reach);
                ph[k] = ph[k].max(risen[k] + reach);
            }
        }
        uni.gbox = [lo[0], lo[1], hi[0], hi[1]];
        uni.gbox_p = [pl[0], pl[1], ph[0], ph[1]];
    }

    let gpu = Gpu::new(&device, &queue, FORMAT, &src, &uni);
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("bench target"),
        size: wgpu::Extent3d {
            width: args.width,
            height: args.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&Default::default());

    let run = |n: u32| {
        let start = Instant::now();
        for _ in 0..n {
            let mut enc = device.create_command_encoder(&Default::default());
            gpu.draw(&mut enc, &view);
            queue.submit(Some(enc.finish()));
        }
        // Submission is asynchronous, so the timing is meaningless until the queue drains.
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        start.elapsed().as_secs_f64() / n as f64
    };

    // Warm up: the first frames pay for shader compilation and lazy allocation.
    run(20);
    let secs = run(args.frames);

    println!(
        "{}x{}  ss={}  bloom={:.2}  pulses={}  flares={}  bolts={}  ->  {:.2} ms/frame  ({:.0} fps)",
        args.width,
        args.height,
        args.ss,
        args.bloom,
        np,
        nf,
        nb,
        secs * 1000.0,
        1.0 / secs,
    );

    if let Some(path) = &args.out {
        save(&device, &queue, &target, args.width, args.height, path);
    }
}

/// Read the rendered texture back and write it out as a PNG.
fn save(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    target: &wgpu::Texture,
    w: u32,
    h: u32,
    path: &str,
) {
    // Buffer rows in a texture copy must be a multiple of 256 bytes.
    let stride = (w * 4).div_ceil(256) * 256;
    let buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: (stride * h) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut enc = device.create_command_encoder(&Default::default());
    enc.copy_texture_to_buffer(
        target.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buf,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(stride),
                rows_per_image: Some(h),
            },
        },
        wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
    );
    queue.submit(Some(enc.finish()));

    buf.slice(..).map_async(wgpu::MapMode::Read, |r| r.unwrap());
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();

    let mapped = buf.slice(..).get_mapped_range().unwrap();
    let mut img = image::RgbImage::new(w, h);
    for y in 0..h {
        let row = (y * stride) as usize;
        for x in 0..w {
            let p = row + (x * 4) as usize;
            img.put_pixel(x, y, image::Rgb([mapped[p], mapped[p + 1], mapped[p + 2]]));
        }
    }
    img.save(path).unwrap_or_else(|e| panic!("cannot write {path}: {e}"));
    println!("  wrote {path}");
}
