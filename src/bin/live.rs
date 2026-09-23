//! Real-time viewer: the sigil turns at a fixed rate; keys bloom parts of it.
//!
//! Renders with the same inverse map as the offline path, but in a fragment shader
//! (`shaders/spin.wgsl`), so the whole disc redraws every frame at display rate. The
//! pipeline and uniform layout live in `imagespin::gpu`, shared with `bench`.
//!
//! Layer speeds are fixed, taken from layers.json so the window matches what the GIF
//! renders. Keys do not steer the motion; they bloom a layer, which fades on its own.

use imagespin::config;
use imagespin::gpu::{self, Gpu, Uniforms, MAX_BOLT_SEGS, MAX_FLARES, MAX_PULSES};

use rand::rngs::SmallRng;
use rand::RngExt;

use std::sync::Arc;
use std::time::Instant;

use wgpu::CurrentSurfaceTexture;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

/// A drop landing in something thick, or the same thing running in reverse.
///
/// Carries the splash at the point of impact as well as the ring, because the two are
/// one event: the crest is what the splash turns into.
struct Pulse {
    age: f32,
    /// Crest radius in source pixels, and how fast it is still travelling.
    radius: f32,
    speed: f32,
    /// Envelope width and ripple wavelength, in source pixels.
    width: f32,
    wavelength: f32,
    strength: f32,
    /// The impact flash at the centre: how bright it still is, and how wide.
    splash: f32,
    splash_radius: f32,
    /// Converging on the centre rather than leaving it.
    imploding: bool,
}

/// What kind of disturbance to start.
enum Drop {
    /// The ambient one: something falls in the middle.
    Droplet,
    /// What Enter fires — a far heavier drop.
    Heavy,
    /// A ring that starts at the rim and closes on the centre, gathering as it goes.
    Implosion,
    /// What an implosion turns into when it lands, carrying its gathered strength.
    Rebound(f32),
}

/// One straight limb of a bolt, in source pixels.
struct Limb {
    a: [f32; 2],
    b: [f32; 2],
}

/// A bolt of lightning arcing across the diagram.
struct Bolt {
    /// The jagged path, plus any fork, flattened into one list.
    limbs: Vec<Limb>,
    age: f32,
    life: f32,
    strength: f32,
    /// Half-width of the hot core, in source pixels.
    width: f32,
    /// Flicker phase, so two bolts alight at once do not pulse in step.
    phase: f32,
}

/// The afterglow left behind where a bolt struck. Placed in un-rotated coordinates, so
/// it stays where it landed while the layers keep turning underneath it.
struct Flare {
    pos: [f32; 2],
    radius: f32,
    age: f32,
    life: f32,
    strength: f32,
}

/// Smoothstep, for envelopes that must not pop at either end.
fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Crest strength.
///
/// A ring spreads a fixed amount of energy over a growing circumference, so amplitude
/// goes as `1/sqrt(r)` — which is also why an implosion gets brighter the further in it
/// gets, rather than merely surviving. Viscosity damps the whole thing on top of that,
/// and the rim fade keeps a wave from ending abruptly against the outer ring.
fn pulse_amount(p: &Pulse, disc: f32) -> f32 {
    let r = p.radius.max(disc * 0.06);
    let spread = (disc * 0.34 / r).sqrt().clamp(0.4, 2.8);
    let damp = (-1.05 * p.age).exp();
    p.strength * spread * damp * (1.0 - smoothstep(0.88, 1.10, p.radius / disc))
}

/// Lightning is a flash that decays fast, with a flicker on top so it reads as
/// electricity rather than as a line quietly fading out.
fn bolt_amount(b: &Bolt) -> f32 {
    let u = (b.age / b.life.max(1e-3)).min(1.0);
    let decay = (1.0 - u) * (1.0 - u);
    b.strength * decay * (0.68 + 0.32 * (b.age * 57.0 + b.phase).sin())
}

/// Strike fast, ebb slowly — a discharge rather than a throb.
fn flare_amount(f: &Flare) -> f32 {
    let u = f.age / f.life.max(1e-3);
    f.strength * smoothstep(0.0, 0.12, u) * (1.0 - smoothstep(0.25, 1.0, u))
}


/// Bolts an implosion throws out of the centre when it lands.
const BURST_BOLTS: u32 = 9;

/// Bolts that can be in the air together. Has to clear a whole burst, or the fan would
/// evict its own first arms before the last ones were out.
const MAX_BOLTS: usize = 14;

/// Seconds for a bloom to fade to ~2% of its peak.
const BLOOM_DECAY: f32 = 0.55;

struct State {
    /// Revolutions per second, signed, fixed for the whole session.
    speed: Vec<f32>,
    angle: Vec<f32>,
    /// Per-layer glow, 1.0 on a keypress, decaying toward 0.
    bloom: Vec<f32>,
    /// Names of the layers.
    names: Vec<String>,
    /// Supersampling factor, 1-4.
    ss: u32,
    /// Picks the layer an unassigned key blooms, and everything the energy does.
    rng: SmallRng,
    /// Rings travelling out from the centre, oldest first.
    pulses: Vec<Pulse>,
    /// Lightning currently in the air.
    bolts: Vec<Bolt>,
    /// Afterglows where lightning struck.
    flares: Vec<Flare>,
    /// Seconds until the next ambient pulse and the next strike.
    next_pulse: f32,
    next_bolt: f32,
    /// Where the last bolt landed. The next one leaves from here, so the lightning
    /// walks the diagram instead of teleporting around it.
    spark_at: [f32; 2],
    /// The fan an implosion left to release: how many arms are still to come, when the
    /// next one is due, how hard they hit, and the angle the fan is built around.
    burst_left: u32,
    burst_timer: f32,
    burst_power: f32,
    burst_base: f32,
    /// How brightly the whole disc is still answering the last collapse.
    flash: f32,
    /// Disc centre and outermost radius, in source pixels.
    center: [f32; 2],
    disc: f32,
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
        println!("\n  press any key to bloom a random layer — 1-{} pick one, Space blooms all",
            self.names.len().min(9));
    }

    /// Light up layer `i`.
    fn flash(&mut self, i: usize) {
        if let Some(b) = self.bloom.get_mut(i) {
            *b = 1.0;
        }
    }


    /// Start a disturbance. The four kinds differ only in how hard they hit and which
    /// way the ring runs.
    fn add_pulse(&mut self, drop: Drop) {
        if self.pulses.len() >= MAX_PULSES {
            self.pulses.remove(0);
        }
        let d = self.disc;
        let base = Pulse {
            age: 0.0,
            radius: 0.0,
            speed: d * 0.92,
            width: d * 0.09,
            wavelength: d * 0.21,
            strength: 0.95,
            splash: 1.2,
            splash_radius: d * 0.16,
            imploding: false,
        };
        self.pulses.push(match drop {
            Drop::Droplet => base,
            Drop::Heavy => Pulse {
                speed: d * 0.72,
                width: d * 0.16,
                wavelength: d * 0.30,
                strength: 1.8,
                splash: 2.4,
                splash_radius: d * 0.28,
                ..base
            },
            // Starts at the rim with nothing at the centre: there has been no impact
            // yet, and the whole point is that it is on its way to one.
            Drop::Implosion => Pulse {
                radius: d * 1.02,
                speed: d * 0.45,
                width: d * 0.10,
                wavelength: d * 0.22,
                strength: 0.85,
                splash: 0.0,
                imploding: true,
                ..base
            },
            // Everything the implosion gathered, coming back out.
            Drop::Rebound(gathered) => Pulse {
                speed: d * 1.10,
                width: d * 0.14,
                wavelength: d * 0.24,
                strength: gathered * 2.1,
                splash: gathered * 3.4,
                splash_radius: d * 0.24,
                ..base
            },
        });
    }

    /// Pull a point back inside the disc, so a bolt and its halo never cross the rim.
    fn inside_disc(&self, p: [f32; 2], frac: f32) -> [f32; 2] {
        let (dx, dy) = (p[0] - self.center[0], p[1] - self.center[1]);
        let r = (dx * dx + dy * dy).sqrt();
        let max = self.disc * frac;
        if r <= max || r < 1e-3 {
            p
        } else {
            [self.center[0] + dx * max / r, self.center[1] + dy * max / r]
        }
    }

    /// A jagged path from `a` to `b`.
    ///
    /// The kinks are largest in the middle and pinched to nothing at both ends, so the
    /// bolt actually meets the two points it is supposed to connect instead of fraying
    /// past them.
    fn jag(&mut self, a: [f32; 2], b: [f32; 2], steps: usize, spread: f32) -> Vec<Limb> {
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let len = (dx * dx + dy * dy).sqrt().max(1.0);
        // Unit normal to the run: the direction the kinks push in.
        let (nx, ny) = (-dy / len, dx / len);
        let mut pts = Vec::with_capacity(steps + 1);
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            // sin is zero at both ends and one in the middle — taut, not frayed.
            let taper = (t * std::f32::consts::PI).sin();
            let off: f32 = self.rng.random_range(-1.0..1.0) * spread * len * taper;
            pts.push([a[0] + dx * t + nx * off, a[1] + dy * t + ny * off]);
        }
        pts.windows(2).map(|w| Limb { a: w[0], b: w[1] }).collect()
    }

    /// Strike from wherever the last bolt landed to somewhere new.
    ///
    /// Chaining is what makes it read as power moving through the formation rather than
    /// as unrelated flashes. Most jumps are short, so the lightning works a region over;
    /// about one in six is a long throw right across the disc, and being rare is what
    /// makes those land.
    fn add_bolt(&mut self) {
        if self.bolts.len() >= MAX_BOLTS {
            self.bolts.remove(0);
        }
        let d = self.disc;
        let long = self.rng.random_range(0.0..1.0) < 0.22;
        let reach = if long {
            d * self.rng.random_range(0.75..1.55)
        } else {
            d * self.rng.random_range(0.22..0.62)
        };
        let dir: f32 = self.rng.random_range(0.0..std::f32::consts::TAU);
        let from = self.spark_at;
        let to = self.inside_disc(
            [from[0] + reach * dir.cos(), from[1] + reach * dir.sin()],
            0.90,
        );

        let steps = if long { 9 } else { 6 };
        let mut limbs = self.jag(from, to, steps, 0.085);

        // A fork now and then. It costs three limbs and does more for the look than
        // anything else here.
        if limbs.len() > 2 && self.rng.random_range(0.0..1.0) < 0.45 {
            let at = self.rng.random_range(1..limbs.len() - 1);
            let root = limbs[at].a;
            let ang: f32 = self.rng.random_range(0.0..std::f32::consts::TAU);
            let flen = reach * self.rng.random_range(0.20..0.45);
            let tip =
                self.inside_disc([root[0] + flen * ang.cos(), root[1] + flen * ang.sin()], 0.90);
            let fork = self.jag(root, tip, 3, 0.11);
            limbs.extend(fork);
        }

        self.bolts.push(Bolt {
            limbs,
            age: 0.0,
            life: self.rng.random_range(0.18..0.38),
            strength: if long {
                self.rng.random_range(1.10..1.60)
            } else {
                self.rng.random_range(0.75..1.15)
            },
            width: self.rng.random_range(2.6..4.0),
            phase: self.rng.random_range(0.0..std::f32::consts::TAU),
        });

        // Something has to light up where it landed, or the strike has no consequence.
        if self.flares.len() >= MAX_FLARES {
            self.flares.remove(0);
        }
        self.flares.push(Flare {
            pos: to,
            radius: self.rng.random_range(40.0..85.0),
            age: 0.0,
            life: self.rng.random_range(0.35..0.70),
            strength: self.rng.random_range(0.5..0.9),
        });

        self.spark_at = to;
    }


    /// One arm of the fan an implosion throws out when it lands.
    ///
    /// Unlike a wandering strike this has a fixed origin and a given direction, and it
    /// deliberately leaves `spark_at` alone — the ordinary lightning must not be dragged
    /// to the centre just because the disc collapsed.
    fn add_burst_bolt(&mut self, angle: f32, power: f32) {
        if self.bolts.len() >= MAX_BOLTS {
            self.bolts.remove(0);
        }
        let d = self.disc;
        let from = self.center;
        let reach = d * self.rng.random_range(0.72..0.95);
        let to = self.inside_disc(
            [from[0] + reach * angle.cos(), from[1] + reach * angle.sin()],
            0.92,
        );

        // Taut and nearly straight: this is energy thrown outward, not something picking
        // its way across the diagram.
        let mut limbs = self.jag(from, to, 6, 0.055);
        if limbs.len() > 2 && self.rng.random_range(0.0..1.0) < 0.18 {
            let at = self.rng.random_range(1..limbs.len() - 1);
            let root = limbs[at].a;
            let ang: f32 = angle + self.rng.random_range(-1.1..1.1);
            let flen = reach * self.rng.random_range(0.18..0.38);
            let tip =
                self.inside_disc([root[0] + flen * ang.cos(), root[1] + flen * ang.sin()], 0.92);
            let fork = self.jag(root, tip, 3, 0.09);
            limbs.extend(fork);
        }

        self.bolts.push(Bolt {
            limbs,
            age: 0.0,
            // Shorter than a wandering strike: this is a flash, and it also keeps the
            // number alight at once — and so the cost — down.
            life: self.rng.random_range(0.12..0.24),
            strength: power * self.rng.random_range(1.2..1.8),
            width: self.rng.random_range(2.8..4.2),
            phase: self.rng.random_range(0.0..std::f32::consts::TAU),
        });

        if self.flares.len() >= MAX_FLARES {
            self.flares.remove(0);
        }
        self.flares.push(Flare {
            pos: to,
            radius: self.rng.random_range(40.0..80.0),
            age: 0.0,
            life: self.rng.random_range(0.3..0.6),
            strength: power * self.rng.random_range(0.5..0.9),
        });
    }

    /// Release the fan a bolt at a time.
    ///
    /// Spread over a fifth of a second rather than fired at once, which reads as energy
    /// rushing out instead of a single flash — and caps how many bolts are alight
    /// together, which is what keeps the cost spike in hand.
    fn advance_burst(&mut self, dt: f32) {
        if self.burst_left == 0 {
            return;
        }
        self.burst_timer -= dt;
        while self.burst_left > 0 && self.burst_timer <= 0.0 {
            let i = (BURST_BOLTS - self.burst_left) as f32;
            // Evenly spaced so it is a fan, jittered so it is not a diagram.
            let spread = std::f32::consts::TAU / BURST_BOLTS as f32;
            let jitter: f32 = self.rng.random_range(-0.35..0.35);
            let angle = self.burst_base + spread * i + jitter * spread;
            let power = self.burst_power;
            self.add_burst_bolt(angle, power);
            self.burst_left -= 1;
            // Spread wider than a bolt lives, so the fan is never all alight at once.
            // That is as much a cost decision as a look one.
            self.burst_timer += self.rng.random_range(0.022..0.042);
        }
    }

    /// Move every disturbance on by `dt`, and land any implosion that has arrived.
    fn advance_pulses(&mut self, dt: f32) {
        let disc = self.disc;
        for p in self.pulses.iter_mut() {
            p.age += dt;
            if p.imploding {
                // Converging: the same energy crowds into an ever shorter circumference,
                // so it runs in faster the closer it gets.
                p.radius -= p.speed * dt;
                p.speed *= 1.0 + 1.5 * dt;
            } else {
                p.radius += p.speed * dt;
                // Thick liquid drags the crest down as it spreads.
                p.speed *= (-1.35 * dt).exp();
            }
            // However the ring behaves, the impact flash is brief.
            p.splash *= (-9.0 * dt).exp();
        }

        // An implosion that reaches the middle does not just stop; it lands.
        let mut landed: Vec<f32> = Vec::new();
        self.pulses.retain(|p| {
            if p.imploding && p.radius <= disc * 0.05 {
                landed.push(p.strength * (-1.05 * p.age).exp());
                false
            } else {
                p.radius - p.width * 6.0 < disc && p.age < 12.0
            }
        });
        for gathered in landed {
            self.add_pulse(Drop::Rebound(gathered));

            // It throws off energy as well as blood. The fan is queued rather than fired
            // here so it comes out as a rush over the next fifth of a second.
            self.burst_left = BURST_BOLTS;
            self.burst_timer = 0.0;
            self.burst_power = gathered.clamp(0.5, 1.5);
            self.burst_base = self.rng.random_range(0.0..std::f32::consts::TAU);
            // ...and the whole formation answers.
            self.flash = 1.0;

            if self.flares.len() >= MAX_FLARES {
                self.flares.remove(0);
            }
            self.flares.push(Flare {
                pos: self.center,
                radius: self.disc * 0.3,
                age: 0.0,
                life: 0.5,
                strength: self.burst_power * 1.2,
            });
        }
    }

    /// Advance everything by `dt`, drop whatever has finished, and spawn whatever is
    /// due. None of this is driven by the keyboard.
    fn advance_energy(&mut self, dt: f32) {
        self.advance_pulses(dt);
        self.advance_burst(dt);
        self.flash *= (-6.0 * dt).exp();

        for b in self.bolts.iter_mut() {
            b.age += dt;
        }
        self.bolts.retain(|b| b.age < b.life);

        for f in self.flares.iter_mut() {
            f.age += dt;
        }
        self.flares.retain(|f| f.age < f.life);

        self.next_pulse -= dt;
        if self.next_pulse <= 0.0 {
            // Now and then the disc draws a ring in from the rim instead of throwing one
            // out. It is worth waiting a little longer for.
            if self.rng.random_range(0.0..1.0) < 0.22 {
                self.add_pulse(Drop::Implosion);
                self.next_pulse = self.rng.random_range(4.5..7.0);
            } else {
                self.add_pulse(Drop::Droplet);
                self.next_pulse = self.rng.random_range(2.6..4.8);
            }
        }
        self.next_bolt -= dt;
        if self.next_bolt <= 0.0 {
            self.add_bolt();
            // Mostly a quick rattle of strikes, with the occasional lull so the bursts
            // stand out against something.
            self.next_bolt = if self.rng.random_range(0.0..1.0) < 0.22 {
                self.rng.random_range(0.7..1.6)
            } else {
                self.rng.random_range(0.08..0.35)
            };
        }
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
    fn new(elwt: &ActiveEventLoop, uni: &Uniforms, src: &image::RgbImage) -> Self {
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

        let gpu = Gpu::new(&device, &queue, format, src, uni);
        Gfx { window, surface, device, queue, surf_cfg, gpu }
    }
}

struct App {
    st: State,
    uni: Uniforms,
    src: image::RgbImage,
    /// `None` until winit first resumes us and a window can be created.
    gfx: Option<Gfx>,
    last: Instant,
    fps_t: Instant,
    fps_n: u32,
}

impl App {
    fn redraw(&mut self) {
        let Some(gfx) = self.gfx.as_mut() else { return };

        let now = Instant::now();
        let dt = (now - self.last).as_secs_f32();
        self.last = now;

        // Constant rate, always.
        for (a, s) in self.st.angle.iter_mut().zip(&self.st.speed) {
            *a += s * dt * std::f32::consts::TAU;
        }
        self.st.advance_energy(dt);

        // Exponential fade, so a bloom decays at the same rate whatever the framerate.
        let fade = (-dt / (BLOOM_DECAY / 4.0)).exp();
        for b in self.st.bloom.iter_mut() {
            *b *= fade;
            if *b < 0.002 {
                *b = 0.0;
            }
        }

        // Fit the source into the window, preserving aspect.
        let (sw, sh) = (self.src.width() as f32, self.src.height() as f32);
        let (w, h) = (gfx.surf_cfg.width as f32, gfx.surf_cfg.height as f32);
        let scale = (w / sw).min(h / sh);
        let n = self.st.names.len();
        self.uni.fit = [scale, (w - sw * scale) * 0.5, (h - sh * scale) * 0.5, n as f32];
        self.uni.params[0] = self.st.ss as f32;
        for i in 0..n {
            let a = self.st.angle[i];
            // The shader rotates by this angle rather than un-rotating an arctangent,
            // so hand it the sin/cos instead of making every pixel recompute them.
            self.uni.layers[i].motion = [a, self.st.bloom[i], a.sin(), a.cos()];
        }
        self.uni.counts[0] = self.st.pulses.len() as f32;
        self.uni.counts[1] = self.st.flares.len() as f32;
        for (i, p) in self.st.pulses.iter().enumerate() {
            let amount = pulse_amount(p, self.st.disc);
            self.uni.pulses[i] = [p.radius, amount, p.width, p.wavelength];
            // A converging ring leaves its wake behind it, which is outward.
            let wake = if p.imploding { -1.0 } else { 1.0 };
            self.uni.pulse_x[i] = [p.splash, p.splash_radius, wake, 0.0];
        }
        for (i, f) in self.st.flares.iter().enumerate() {
            self.uni.flares[i] = [f.pos[0], f.pos[1], f.radius, flare_amount(f)];
        }
        // Every live bolt's limbs go into one flat list, which is all the shader wants.
        let mut seg = 0usize;
        for b in self.st.bolts.iter() {
            let amount = bolt_amount(b);
            for l in b.limbs.iter() {
                if seg >= MAX_BOLT_SEGS {
                    break;
                }
                self.uni.bolts[seg] = [l.a[0], l.a[1], l.b[0], l.b[1]];
                self.uni.bolt_w[seg] = [amount, b.width, 0.0, 0.0];
                seg += 1;
            }
        }
        self.uni.counts[2] = seg as f32;
        self.uni.counts[3] = self.st.flash;
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

        self.fps_n += 1;
        if self.fps_t.elapsed().as_secs_f32() >= 2.0 {
            gfx.window.set_title(&format!(
                "imagespin — live   {:.0} fps   {}x{}   {}x{} ss",
                self.fps_n as f32 / self.fps_t.elapsed().as_secs_f32(),
                gfx.surf_cfg.width,
                gfx.surf_cfg.height,
                self.st.ss,
                self.st.ss
            ));
            self.fps_n = 0;
            self.fps_t = Instant::now();
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, elwt: &ActiveEventLoop) {
        elwt.set_control_flow(ControlFlow::Poll);
        if self.gfx.is_none() {
            self.gfx = Some(Gfx::new(elwt, &self.uni, &self.src));
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
                let n = self.st.names.len();
                match logical_key.as_ref() {
                    Key::Named(NamedKey::Escape) => elwt.exit(),
                    Key::Named(NamedKey::F1) => help(),

                    // The one key that drives the energy rather than the bloom.
                    Key::Named(NamedKey::Enter) => self.st.add_pulse(Drop::Heavy),

                    // Supersampling is a render-quality knob, not a motion one.
                    Key::Character("-") => self.st.ss = self.st.ss.saturating_sub(1).max(1),
                    Key::Character("=") | Key::Character("+") => {
                        self.st.ss = (self.st.ss + 1).min(4)
                    }

                    Key::Named(NamedKey::Space) => {
                        for i in 0..n {
                            self.st.flash(i);
                        }
                    }

                    // 1-9 name a layer; every other key draws one at random.
                    Key::Character(s) => {
                        for c in s.chars() {
                            match c.to_digit(10) {
                                Some(d) if d >= 1 => self.st.flash(d as usize - 1),
                                _ => {
                                    let i = self.st.random_layer();
                                    self.st.flash(i);
                                }
                            }
                        }
                    }

                    // Every remaining key still blooms something.
                    _ => {
                        let i = self.st.random_layer();
                        self.st.flash(i);
                    }
                }
            }

            WindowEvent::RedrawRequested => self.redraw(),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _elwt: &ActiveEventLoop) {
        if let Some(gfx) = self.gfx.as_ref() {
            gfx.window.request_redraw();
        }
    }
}

fn main() {
    env_logger::init();
    let cfg_path = std::env::args().nth(1).unwrap_or_else(|| "layers.json".into());
    let (cfg, src) = config::load(&cfg_path);
    let layers = gpu::ordered_layers(&cfg);

    // Fixed for the session: the same speeds the GIF renders at.
    let fps = cfg.fps.max(1) as f32;
    let loop_secs = cfg.frames as f32 / fps;
    let disc = layers.iter().map(|l| l.outer.max_radius()).fold(0.0f32, f32::max);
    let ss = 2;
    let st = State {
        speed: layers.iter().map(|l| l.turns as f32 / loop_secs).collect(),
        angle: vec![0.0; layers.len()],
        bloom: vec![0.0; layers.len()],
        names: layers.iter().map(|l| l.name.clone()).collect(),
        ss,
        rng: rand::make_rng(),
        pulses: Vec::new(),
        bolts: Vec::new(),
        flares: Vec::new(),
        // Do not open on a pulse; let the diagram turn for a moment first.
        next_pulse: 1.4,
        next_bolt: 0.6,
        // The first bolt has nowhere to come from, so start it at the centre.
        spark_at: cfg.center,
        burst_left: 0,
        burst_timer: 0.0,
        burst_power: 0.0,
        burst_base: 0.0,
        flash: 0.0,
        center: cfg.center,
        disc,
    };

    help();
    st.describe();

    let uni = gpu::uniforms(&cfg, &layers, &src, ss);
    let mut app = App {
        st,
        uni,
        src,
        gfx: None,
        last: Instant::now(),
        fps_t: Instant::now(),
        fps_n: 0,
    };

    EventLoop::new().unwrap().run_app(&mut app).unwrap();
}
