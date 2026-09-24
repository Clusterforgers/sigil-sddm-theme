use imagespin::effects::{Census, Drop, Effects};
use imagespin::gpu::{self, Gpu, Uniforms};
use imagespin::figure::{self, Figure, FigureError, Rendered};

use rand::rngs::SmallRng;
use rand::RngExt;

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::time::Instant;

use wgpu::CurrentSurfaceTexture;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

/// Which mip of the artwork to read, given how the window maps onto it.
///
/// One sub-sample covers `1 / (fit * ss)` source texels; when that exceeds one the disc is
/// being minified and level 0 is undersampled, which is what makes the thin gold lines
/// crawl. Below one there is nothing to gain, so the level floors at zero.
///
/// The bias is not a fudge. Trilinear filtering blends toward a full 2x2 box, which is a
/// wider filter than the footprint actually calls for, so the straight `-log2` over-blurs.
/// Fitted against a brute-force `ss=8` render: -0.35 beats both level 0 and an unbiased
/// level on closeness to that reference *and* on high-frequency energy, at 714x427 and at
/// 500x300 alike.
fn source_lod(fit_scale: f32, ss: u32, canvas_scale: f32) -> f32 {
    // `canvas_scale` is how much denser the texture is than the coordinate space, which
    // shifts the whole thing by one level per doubling.
    let rate = (fit_scale * ss as f32 / canvas_scale.max(1e-3)).max(1e-3);
    (-rate.log2() - 0.35).max(0.0)
}

/// The worst frame seen in a reporting interval, and what was on screen for it.
///
/// A vsync-locked viewer cannot be timed by asking the GPU politely — every frame appears
/// to take exactly one refresh until one of them misses, and then the gap jumps. So the
/// thing to watch is the longest gap between frames, together with what was live when it
/// happened. That is also precisely what "laggy" means to someone watching it.
#[derive(Default, Clone, Copy)]
struct Worst {
    dt: f32,
    live: Census,
}

/// How the layers turn, and how the viewer is set to draw them.
struct State {
    /// Revolutions per second, positive clockwise, from each layer's `turns`.
    speed: Vec<f32>,
    /// Each layer's current turn: radians, counter-clockwise, as the shader wants it.
    angle: Vec<f32>,
    /// Names of the layers.
    names: Vec<String>,
    /// Supersampling factor, 1-4.
    ss: u32,
    /// Paint each layer in its own colour, for telling which ring turns with which.
    tint: bool,
    /// Picks the layer an unassigned key blooms.
    rng: SmallRng,
}

impl State {
    /// The colours `t` paints the layers in, in the same order as `describe`. Matches
    /// `layer_tint` in `common.wgsl`, by hand.
    fn legend(&self) {
        const NAMES: [&str; 7] =
            ["red", "orange", "yellow", "green", "cyan", "blue", "magenta"];
        println!("\n  layer colours on — each ring is painted by the layer that turns it");
        for (i, name) in self.names.iter().enumerate() {
            println!(
                "    {}  {:<9}  {:<18}  {}",
                i + 1,
                NAMES[i.min(NAMES.len() - 1)],
                name,
                self.period(i)
            );
        }
        println!("  edit `turns` in the figure file; saving it updates the viewer\n");
    }

    fn describe(&self) {
        println!("\n  layers (edit the figure file to change them; saves are picked up live)");
        for (i, name) in self.names.iter().enumerate() {
            println!("    {}  {:<18}  {:>6.3} rev/s   {}", i + 1, name, self.speed[i], self.period(i));
        }
        println!("\n  press any key to bloom a random layer — 1-{} pick one, Space blooms all",
            self.names.len().min(9));
    }

    /// Seconds per revolution and which way.
    fn period(&self, i: usize) -> String {
        let s = self.speed[i];
        if s.abs() < 1e-4 {
            "static".to_string()
        } else {
            format!("{:.1}s/rev {}", 1.0 / s.abs(), if s > 0.0 { "CW" } else { "CCW" })
        }
    }

    /// Take on the layers of `fig`. A layer that was there before, by name, carries on
    /// from the angle it had reached, so saving an edit does not make everything jump.
    fn adopt(&mut self, fig: &Rendered) {
        let loop_secs = fig.loop_secs.max(1e-3);
        self.angle = fig
            .layers
            .iter()
            .map(|l| self.names.iter().position(|n| *n == l.name).map_or(0.0, |i| self.angle[i]))
            .collect();
        self.speed = fig.layers.iter().map(|l| l.turns as f32 / loop_secs).collect();
        self.names = fig.layers.iter().map(|l| l.name.clone()).collect();
    }

    /// Which layer an unassigned key lights up: a fresh draw on every press, so
    /// holding the same key keeps moving the bloom around.
    fn random_layer(&mut self) -> usize {
        self.rng.random_range(0..self.names.len().max(1))
    }
}

fn help() {
    println!(
        r#"
  imagespin live — the sigil turns at a fixed rate; keys make it bloom and pulse

    any key      bloom a random layer
    1-9          bloom that specific layer
    Space        bloom every layer at once
    Enter        a heavy drop into the middle
    t            colour each layer differently, to see which ring is which
    - / =        supersampling down / up (antialiasing vs framerate)
    F1           this help
    Esc          quit

  Blood drops into the middle on its own — and now and then implodes instead — while
  blue lightning arcs from one part of the formation to the next. Neither needs a key.
"#
    );
}

/// Everything that only exists once winit has handed us a window.
struct Gfx {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surf_cfg: wgpu::SurfaceConfiguration,
    gpu: Gpu,
}

impl Gfx {
    fn new(elwt: &ActiveEventLoop, uni: &Uniforms, fig: &Rendered) -> Self {
        let window = Arc::new(
            elwt.create_window(
                Window::default_attributes()
                    .with_title("imagespin — live")
                    .with_inner_size(PhysicalSize::new(1280, 720)),
            )
            .unwrap(),
        );

        let instance = wgpu::Instance::default();
        let surface = instance.create_surface(window.clone()).unwrap();
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
            ..Default::default()
        }))
        .expect("no suitable GPU adapter");
        println!("  GPU: {}\n", adapter.get_info().name);

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: None,
            required_features: wgpu::Features::empty(),
            // downlevel_defaults caps textures at 2048, which a HiDPI surface exceeds.
            required_limits: wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits()),
            ..Default::default()
        }))
        .unwrap();

        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(caps.formats[0]);
        let surf_cfg = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            // Let the backend pick, which is sRGB for the sRGB format chosen above.
            color_space: Default::default(),
            width: window.inner_size().width.max(1),
            height: window.inner_size().height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surf_cfg);

        let gpu = Gpu::new(&device, &queue, format, fig, uni);
        Gfx { window, surface, device, queue, surf_cfg, gpu }
    }

    /// Upload a new figure. The pipeline is cheap to build, so it is simply rebuilt.
    fn load(&mut self, fig: &Rendered, uni: &Uniforms) {
        self.gpu = Gpu::new(&self.device, &self.queue, self.surf_cfg.format, fig, uni);
    }
}

struct App {
    st: State,
    fx: Effects,
    uni: Uniforms,
    /// The figure on screen.
    fig: Rendered,
    /// New versions of it, each time the file is saved.
    reloads: Receiver<Result<Rendered, FigureError>>,
    /// `None` until winit first resumes us and a window can be created.
    gfx: Option<Gfx>,
    last: Instant,
    fps_t: Instant,
    fps_n: u32,
    worst: Worst,
}

impl App {
    /// Swap in a newly saved figure, keeping everything that is still meaningful: layer
    /// angles by name, the effects in flight, and the viewer's settings.
    fn load(&mut self, fig: Rendered) {
        for w in &fig.warnings {
            println!("  warning: {w}");
        }
        self.st.adopt(&fig);
        self.uni = gpu::uniforms(&fig, self.st.ss);
        self.fx.reload(&fig);
        if let Some(gfx) = self.gfx.as_mut() {
            gfx.load(&fig, &self.uni);
        }
        println!("  reloaded: {} layers, {} glyphs", fig.layers.len(), fig.glyphs.len());
        self.fig = fig;
    }

    fn redraw(&mut self) {
        let Some(gfx) = self.gfx.as_mut() else { return };

        let now = Instant::now();
        let dt = (now - self.last).as_secs_f32();
        self.last = now;

        // Constant rate, always. Speeds are clockwise; the angle the shader wants runs the
        // other way.
        for (a, s) in self.st.angle.iter_mut().zip(&self.st.speed) {
            *a -= s * dt * std::f32::consts::TAU;
        }
        self.fx.advance(dt);

        // Fit the canvas into the window, preserving aspect.
        let [sw, sh] = self.fig.canvas;
        let (w, h) = (gfx.surf_cfg.width as f32, gfx.surf_cfg.height as f32);
        let scale = (w / sw).min(h / sh);
        let n = self.st.names.len();
        self.uni.fit = [scale, (w - sw * scale) * 0.5, (h - sh * scale) * 0.5, n as f32];
        self.uni.quality[0] = self.st.ss as f32;
        self.uni.quality[3] = source_lod(scale, self.st.ss, self.fig.scale);
        self.uni.look[1] = if self.st.tint { 1.0 } else { 0.0 };
        for (l, &a) in self.uni.layers.iter_mut().zip(&self.st.angle) {
            // The shader rotates by this angle rather than un-rotating an arctangent, so
            // hand it the sin/cos instead of making every pixel recompute them. Slot 1 is
            // the bloom, which `Effects::write` fills in.
            let (s, c) = a.sin_cos();
            l.motion[0] = a;
            l.motion[2] = s;
            l.motion[3] = c;
        }
        self.fx.write(&mut self.uni, &self.st.angle);

        gfx.queue.write_buffer(&gfx.gpu.ubuf, 0, bytemuck::bytes_of(&self.uni));

        let frame = match gfx.surface.get_current_texture() {
            CurrentSurfaceTexture::Success(f) | CurrentSurfaceTexture::Suboptimal(f) => f,
            CurrentSurfaceTexture::Outdated | CurrentSurfaceTexture::Lost => {
                gfx.surface.configure(&gfx.device, &gfx.surf_cfg);
                return;
            }
            // Timed out, hidden, or a validation error: skip this frame and retry.
            _ => return,
        };
        let v = frame.texture.create_view(&Default::default());
        let mut enc = gfx.device.create_command_encoder(&Default::default());
        gfx.gpu.draw(&mut enc, &v);
        gfx.queue.submit(Some(enc.finish()));
        gfx.queue.present(frame);

        // Stalls only show up as a long gap, so keep the worst one and what caused it.
        // Anything past half a second is the compositor hiding us, not a slow frame.
        if dt > self.worst.dt && dt < 0.5 {
            self.worst = Worst { dt, live: self.fx.census() };
        }

        self.fps_n += 1;
        if self.fps_t.elapsed().as_secs_f32() >= 2.0 {
            let fps = self.fps_n as f32 / self.fps_t.elapsed().as_secs_f32();
            gfx.window.set_title(&format!(
                "imagespin — live   {:.0} fps   {}x{}   {}x{} ss",
                fps, gfx.surf_cfg.width, gfx.surf_cfg.height, self.st.ss, self.st.ss
            ));
            let Worst { dt, live: c } = self.worst;
            println!(
                "  {:5.1} fps   worst frame {:5.2} ms  [pulses {}  limbs {}  flares {}  glyphs {}  bloom {:.2}]",
                fps,
                dt * 1000.0,
                c.pulses, c.limbs, c.flares, c.glyphs, c.bloom,
            );
            self.worst = Worst::default();
            self.fps_n = 0;
            self.fps_t = Instant::now();
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, elwt: &ActiveEventLoop) {
        elwt.set_control_flow(ControlFlow::Poll);
        if self.gfx.is_none() {
            self.gfx = Some(Gfx::new(elwt, &self.uni, &self.fig));
            // Timestamps from before the window existed would make the first frame jump.
            self.last = Instant::now();
            self.fps_t = Instant::now();
        }
    }

    fn window_event(&mut self, elwt: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => elwt.exit(),

            WindowEvent::Resized(size) => {
                if let Some(gfx) = self.gfx.as_mut() {
                    gfx.surf_cfg.width = size.width.max(1);
                    gfx.surf_cfg.height = size.height.max(1);
                    gfx.surface.configure(&gfx.device, &gfx.surf_cfg);
                }
            }

            WindowEvent::KeyboardInput {
                event: KeyEvent { logical_key, state: ElementState::Pressed, .. },
                ..
            } => {
                // Nothing here changes how fast anything turns — keys only light things up.
                match logical_key.as_ref() {
                    Key::Named(NamedKey::Escape) => elwt.exit(),
                    Key::Named(NamedKey::F1) => help(),

                    // The one key that drives the energy rather than the bloom.
                    Key::Named(NamedKey::Enter) => self.fx.drop(Drop::Heavy),

                    // Before the catch-all below, which blooms on any other character.
                    Key::Character("t") | Key::Character("T") => {
                        self.st.tint = !self.st.tint;
                        if self.st.tint {
                            self.st.legend();
                        } else {
                            println!("  layer colours off");
                        }
                    }

                    // Supersampling is a render-quality knob, not a motion one.
                    Key::Character("-") => self.st.ss = self.st.ss.saturating_sub(1).max(1),
                    Key::Character("=") | Key::Character("+") => {
                        self.st.ss = (self.st.ss + 1).min(4)
                    }

                    Key::Named(NamedKey::Space) => self.fx.bloom.light_all(),

                    // 1-9 name a layer; every other key draws one at random.
                    Key::Character(s) => {
                        for c in s.chars() {
                            match c.to_digit(10) {
                                Some(d) if d >= 1 => self.fx.bloom.light(d as usize - 1),
                                _ => {
                                    let i = self.st.random_layer();
                                    self.fx.bloom.light(i);
                                }
                            }
                        }
                    }

                    // Every remaining key still blooms something.
                    _ => {
                        let i = self.st.random_layer();
                        self.fx.bloom.light(i);
                    }
                }
            }

            WindowEvent::RedrawRequested => self.redraw(),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _elwt: &ActiveEventLoop) {
        while let Ok(result) = self.reloads.try_recv() {
            match result {
                Ok(fig) => self.load(fig),
                Err(e) => eprintln!("\n{e}\n  (still showing the last figure that loaded)\n"),
            }
        }
        if let Some(gfx) = self.gfx.as_ref() {
            gfx.window.request_redraw();
        }
    }
}

/// Texture pixels per canvas unit. Twice, so the thin lines stay crisp when the window is
/// larger than the canvas; it is also the knob for how much GPU memory the layers take.
const CANVAS_SCALE: f32 = 2.0;

fn main() -> ExitCode {
    env_logger::init();
    let path = PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| "figure.json5".into()));
    let fig = match Figure::load(&path) {
        Ok(f) => f.render(CANVAS_SCALE),
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };
    let side = fig.layers[0].image.width();
    println!("  {} layers of {side}x{side}, {} glyphs", fig.layers.len(), fig.glyphs.len());
    for w in &fig.warnings {
        println!("  warning: {w}");
    }

    let ss = 2;
    let mut st = State {
        speed: Vec::new(),
        angle: Vec::new(),
        names: Vec::new(),
        ss,
        tint: false,
        rng: rand::make_rng(),
    };
    st.adopt(&fig);

    help();
    st.describe();
    println!("\n  watching {} for changes", path.display());

    let uni = gpu::uniforms(&fig, ss);
    let fx = Effects::new(&fig);
    let mut app = App {
        st,
        fx,
        uni,
        reloads: figure::watch(path, CANVAS_SCALE),
        fig,
        gfx: None,
        last: Instant::now(),
        fps_t: Instant::now(),
        fps_n: 0,
        worst: Worst::default(),
    };

    EventLoop::new().unwrap().run_app(&mut app).unwrap();
    ExitCode::SUCCESS
}
