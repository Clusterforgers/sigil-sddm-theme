use imagespin::effects::Effects;
use imagespin::figure::{Figure, Rendered};
use imagespin::gpu::{self, Gpu, Uniforms};

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
    /// Activated glyphs, taken from the ones the figure actually draws.
    glyphs: u32,
    /// How far the activated glyphs have floated, in canvas units.
    lift: f32,
    frames: u32,
    /// Write the rendered frame here, for checking a change did not alter the picture.
    out: Option<String>,
    /// Instead of the stress scene: run the real effects this many seconds and render that
    /// moment. For seeing them without a window.
    run: Option<f32>,
    /// Set this off at the start of the run: `surge`, `color`, `dissolve`, `scramble` or
    /// `constellation`; or keep it up through the run: `typing` or `hover`.
    trigger: Option<String>,
    figure: String,
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
        run: None,
        trigger: None,
        figure: "figure.json5".into(),
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
            "--figure" => a.figure = val().clone(),
            "--out" => a.out = Some(val().clone()),
            "--run" => a.run = Some(val().parse().unwrap()),
            "--trigger" => a.trigger = Some(val().clone()),
            // The moment `N` seconds after Enter.
            "--surge" => {
                a.run = Some(val().parse().unwrap());
                a.trigger = Some("surge".into());
            }
            other => panic!("unknown argument {other}"),
        }
        i += 2;
    }
    a
}

fn main() {
    env_logger::init();
    let args = parse_args();
    let canvas_scale = 2.0;
    let fig = Figure::load(&args.figure).unwrap_or_else(|e| panic!("{e}")).render(canvas_scale);
    let n_layers = fig.layers.len();

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

    // The same layers the viewer uploads, drawn at the same density.
    let mut uni = gpu::uniforms(&fig, args.ss);
    // Same aspect-preserving fit the viewer computes for its surface.
    let [sw, sh] = fig.canvas;
    let (w, h) = (args.width as f32, args.height as f32);
    let scale = (w / sw).min(h / sh);
    uni.fit = [scale, (w - sw * scale) * 0.5, (h - sh * scale) * 0.5, n_layers as f32];
    // Same rule the viewer uses, bias included — see `source_lod` in live.rs.
    uni.quality[3] = (-(scale * args.ss as f32 / canvas_scale).max(1e-3).log2() - 0.35).max(0.0);
    for i in 0..n_layers {
        // A fixed angle off the axes, so no layer lands on a degenerate case.
        let a = 0.37 * (i + 1) as f32;
        uni.layers[i].motion = [a, args.bloom, a.sin(), a.cos()];
    }

    // Rings spread evenly across the disc, and surges ringed around it, both at full
    // strength: more overlap than the viewer ever produces at once.
    let disc = uni.quality[1];
    let np = args.pulses.min(imagespin::gpu::MAX_PULSES as u32);
    uni.live[0] = np as f32;
    for i in 0..np as usize {
        let t = (i as f32 + 0.5) / np as f32;
        uni.pulses[i].wave = [disc * t, 1.0, disc * 0.09, disc * 0.21];
        uni.pulses[i].splash = [0.0, disc * 0.16, 1.0, 0.0];
    }
    let nf = args.flares.min(imagespin::gpu::MAX_FLARES as u32);
    uni.live[1] = nf as f32;
    for i in 0..nf as usize {
        let a = std::f32::consts::TAU * i as f32 / nf as f32;
        uni.flares[i].at = [
            fig.center[0] + disc * 0.55 * a.sin(),
            fig.center[1] + disc * 0.55 * a.cos(),
            60.0,
            1.0,
        ];
    }
    // Short limbs wound out across the disc. Not a real bolt's path, but limbs of a
    // realistic length scattered over a realistic area, which is what the cost turns on:
    // a spoke spanning the whole disc would have a bounding box the culling cannot use.
    let nb = args.bolts.min(imagespin::gpu::MAX_BOLT_SEGS as u32);
    uni.live[2] = nb as f32;
    for i in 0..nb as usize {
        let t = (i as f32 + 0.5) / nb as f32;
        let a = std::f32::consts::TAU * t * 3.0;
        let rr = disc * (0.15 + 0.72 * t);
        let dir = a + 1.1;
        let len = disc * 0.07;
        let sx = fig.center[0] + rr * a.sin();
        let sy = fig.center[1] + rr * a.cos();
        uni.limbs[i].seg = [sx, sy, sx + len * dir.sin(), sy + len * dir.cos()];
        uni.limbs[i].style = [1.0, 3.2, 0.0, 0.0];
    }

    // A real cluster, chosen the way the viewer chooses one: a seed and its nearest
    // neighbours. Spreading them over the whole plate would measure a bounding box the
    // viewer never produces, and would not show what a group looks like either.
    let cat = &fig.glyphs;
    let ng = args.glyphs.min(imagespin::gpu::MAX_GLYPHS as u32).min(cat.len() as u32);
    uni.live[3] = ng as f32;
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
        near.sort_by(|a, b| a.0.total_cmp(&b.0));

        // Progress is read off the lift, so a still shows the colour and spread a glyph
        // would have reached by the time it had drifted that far.
        let progress = (args.lift / 20.0).clamp(0.0, 1.0);
        let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
        let (mut pl, mut ph) = ([f32::MAX; 2], [f32::MIN; 2]);
        for (i, &(_, idx)) in near.iter().take(ng as usize).enumerate() {
            let g = cat[idx];
            let ext = g.radius + 3.0;
            uni.glyphs[i].slot = [g.pos[0], g.pos[1], ext, ext];

            // The bench turns the layers like the viewer does, so the glyph has to be
            // carried round by its own layer before the rise is added. Assuming artwork
            // and screen coincide here puts the copy somewhere the shader never looks.
            let (ax, ay) = (g.pos[0] - fig.center[0], g.pos[1] - fig.center[1]);
            let li = g.layer;
            let ang = 0.37 * (li + 1) as f32;
            let (s, c) = (ang.sin(), ang.cos());
            let (mut px, mut py) = (ax * c + ay * s, ay * c - ax * s);
            let len = (px * px + py * py).sqrt().max(1e-3);
            px += px * args.lift / len;
            py += py * args.lift / len;
            let risen = [fig.center[0] + px, fig.center[1] + py];

            uni.glyphs[i].risen = [risen[0], risen[1], 1.0, progress];
            uni.glyphs[i].turn = [s, c, li as f32, 0.0];
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

    if let Some(secs) = args.run {
        simulate(&fig, &mut uni, secs, args.trigger.as_deref());
    }

    let gpu = Gpu::new(&device, &queue, FORMAT, &fig, &uni);
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

/// Set off `trigger`, if any, and run the effects and the rotation for `secs` exactly as the
/// viewer does, then leave that moment in `uni`.
fn simulate(fig: &Rendered, uni: &mut Uniforms, secs: f32, trigger: Option<&str>) {
    const DT: f32 = 1.0 / 60.0;
    let mut fx = Effects::new(fig);
    // With nothing set off, the run is the figure drawing itself in; with something, it
    // starts from the whole figure, since that is what the effect needs to play over.
    if trigger.is_some() {
        fx.skip_build();
    }
    match trigger {
        Some("surge") => fx.surge(),
        Some("color") => fx.color_wave(),
        Some("dissolve") => fx.dissolve(),
        Some("scramble") => fx.scramble(),
        Some("constellation") => fx.constellation(),
        Some("typing") | Some("hover") | None => {}
        Some(other) => panic!(
            "--trigger takes surge, color, dissolve, scramble, constellation, typing or hover, not {other}"
        ),
    }
    let steps = (secs / DT).round() as usize;
    for i in 0..steps {
        let t = i as f32 * DT;
        match trigger {
            // A character every 0.15 s.
            Some("typing") if i % 9 == 0 => fx.key(),
            // The pointer sweeping an arc across the figure.
            Some("hover") => {
                let a = t * 1.3;
                fx.pointer(Some([fig.center[0] + 260.0 * a.cos(), fig.center[1] + 260.0 * a.sin()]));
            }
            _ => {}
        }
        fx.advance(DT);
    }
    fx.write(uni);
    let [jx, jy] = fx.shake();
    uni.fit[1] += jx * uni.fit[0];
    uni.fit[2] += jy * uni.fit[0];
}
