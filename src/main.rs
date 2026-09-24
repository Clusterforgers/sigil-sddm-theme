use imagespin::{config, fit, gifout, glyphs, render, sigil};
use config::{parse_hex, Config};

use clap::{Parser, Subcommand};
use imagespin::geom::Boundary;
use image::{Rgb, RgbImage};
use render::Renderer;
use std::f32::consts::TAU;

#[derive(Parser)]
#[command(about = "Spin each layer of a mandala/sigil image independently")]
struct Cli {
    #[arg(long, short, default_value = "layers.json", global = true)]
    config: String,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Draw the layer boundaries over the source image, for tuning layers.json.
    Preview {
        #[arg(long, default_value = "preview.png")]
        out: String,
    },
    /// Score every boundary in the config: low == sits in an empty gap.
    Check {},

    /// Outline brightness across a radius range, for locating gaps.
    Gaps {
        #[arg(long, default_value_t = 7.0)]
        sides: f32,
        #[arg(long, default_value_t = 0.0)]
        phase: f32,
        #[arg(long)]
        rmin: f32,
        #[arg(long)]
        rmax: f32,
    },

    /// Scan for polygon boundaries that match the drawn artwork.
    Fit {
        #[arg(long, default_value_t = 7)]
        sides: u32,
        #[arg(long, default_value_t = 60.0)]
        rmin: f32,
        #[arg(long, default_value_t = 400.0)]
        rmax: f32,
        #[arg(long, default_value_t = 14)]
        top: usize,
    },

    /// Find the individual letters and symbols, and mark what was found.
    Glyphs {
        #[arg(long, default_value = "glyphs.png")]
        out: String,
    },
    /// Draw the sigil procedurally instead of reading the source image.
    Draw {
        #[arg(long, default_value = "sigil.png")]
        out: String,
        /// Canvas multiple of the coordinates layers.json is written in.
        #[arg(long, default_value_t = 1.0)]
        scale: f32,
        /// Blend the drawing over the source image, to check the two line up.
        #[arg(long)]
        over_source: bool,
        /// Draw the linework on top of the photograph, rather than on its own.
        #[arg(long)]
        composite: bool,
    },
    /// Render the animated GIF.
    Render {
        #[arg(long, default_value = "spin.gif")]
        out: String,
        #[arg(long)]
        frames: Option<u32>,
        #[arg(long)]
        colors: Option<usize>,
        /// Supersampling factor per axis.
        #[arg(long, default_value_t = 3)]
        ss: u32,
        /// Also dump every frame as a PNG into this directory.
        #[arg(long)]
        png_frames: Option<String>,
        /// Delta-encoding tolerance (squared RGB distance). 0 == lossless delta;
        /// higher trades a little ghosting in the antialiasing for a smaller file.
        #[arg(long, default_value_t = 200)]
        tol: i32,
        /// Render one extra frame (== frame 0) to verify the loop is seamless.
        #[arg(long)]
        loop_check: bool,
        /// Paint each layer in its own colour, for telling which ring turns with which.
        #[arg(long)]
        tint: bool,
    },
}

/// Stroke each layer boundary over the source so the radii/phases can be eyeballed.
fn preview(cfg: &Config, src: &RgbImage, out: &str) {
    let mut img = src.clone();
    let (cx, cy) = (cfg.center[0], cfg.center[1]);
    let colors: [[u8; 3]; 6] = [
        [255, 0, 0],     // red
        [0, 255, 255],   // cyan
        [0, 255, 0],     // green
        [255, 0, 255],   // magenta
        [80, 140, 255],  // blue
        [255, 255, 255], // white
    ];

    let mut plot = |b: Boundary, col: [u8; 3]| {
        // Dense angular sweep so polygon edges come out solid.
        let steps = 20000;
        for i in 0..steps {
            let a = TAU * i as f32 / steps as f32;
            let r = b.radius_at(a);
            if r <= 0.0 {
                continue;
            }
            let x = cx - r * a.sin();
            let y = cy - r * a.cos();
            for (dx, dy) in [(0, 0), (1, 0), (0, 1)] {
                let (px, py) = (x as i32 + dx, y as i32 + dy);
                if px >= 0 && py >= 0 && (px as u32) < img.width() && (py as u32) < img.height() {
                    img.put_pixel(px as u32, py as u32, Rgb(col));
                }
            }
        }
    };

    for (i, l) in cfg.layers.iter().enumerate() {
        let c = colors[i % colors.len()];
        plot(l.outer, c);
        plot(l.inner, c);
        println!(
            "  {} layer {} '{}'  turns={:+}",
            ["red", "cyan", "green", "magenta", "blue", "white"][i % 6],
            i,
            l.name,
            l.turns
        );
    }
    img.save(out).unwrap();
    println!("wrote {out}");
}

fn main() {
    let cli = Cli::parse();
    let (cfg, src) = config::load(&cli.config);

    match cli.cmd {
        Cmd::Preview { out } => preview(&cfg, &src, &out),

        Cmd::Check {} => {
            fit::check(&src, (cfg.center[0], cfg.center[1]), &cfg.layers, 14.0);
        }

        Cmd::Gaps { sides, phase, rmin, rmax } => {
            fit::gap_profile(&src, (cfg.center[0], cfg.center[1]), sides, phase, rmin, rmax);
        }

        Cmd::Fit { sides, rmin, rmax, top } => {
            fit::scan(&src, (cfg.center[0], cfg.center[1]), sides, rmin, rmax, top);
        }

        Cmd::Glyphs { out } => {
            let disc = cfg.layers.iter().map(|l| l.outer.max_radius()).fold(0.0f32, f32::max);
            let found = glyphs::find(&src, cfg.center, disc);
            println!("  found {} glyphs", found.len());
            let mut img = src.clone();
            for g in &found {
                // Ring each one, so what was picked up and what was missed is obvious.
                let steps = 64;
                for i in 0..steps {
                    let a = TAU * i as f32 / steps as f32;
                    let x = g.pos[0] + g.radius * a.cos();
                    let y = g.pos[1] + g.radius * a.sin();
                    if x >= 0.0 && y >= 0.0 && x < img.width() as f32 && y < img.height() as f32 {
                        img.put_pixel(x as u32, y as u32, Rgb([0, 255, 255]));
                    }
                }
            }
            img.save(&out).unwrap_or_else(|e| panic!("cannot write {out}: {e}"));
            println!("  wrote {out}");
        }
        Cmd::Draw { out, scale, over_source, composite } => {
            let fig = sigil::Figure::default();
            let mut img = if composite {
                sigil::over(&cfg, &fig, &src, scale)
            } else {
                sigil::draw(&cfg, &fig, scale)
            };
            if over_source {
                // Half the source, half the drawing: anything misplaced shows up as a
                // doubled line rather than having to be spotted by memory.
                let s = image::imageops::resize(
                    &src,
                    img.width(),
                    img.height(),
                    image::imageops::FilterType::Lanczos3,
                );
                for (p, q) in img.pixels_mut().zip(s.pixels()) {
                    // Drawing in red, source in green, so overlap reads as yellow.
                    let drawn = p.0[0].max(p.0[1]);
                    let orig = q.0[0].max(q.0[1]);
                    p.0 = [drawn, orig, 0];
                }
            }
            img.save(&out).unwrap_or_else(|e| panic!("cannot write {out}: {e}"));
            println!("wrote {out}  ({}x{})", img.width(), img.height());
        }
        Cmd::Render { out, frames, colors, ss, png_frames, tol, loop_check, tint } => {
            let n = frames.unwrap_or(cfg.frames);
            let ncol = colors.unwrap_or(cfg.colors);
            let bg = parse_hex(&cfg.background);
            let (ow, oh) = (cfg.out_size[0], cfg.out_size[1]);

            // The same canvas the viewer turns. The cuts belong to the drawing now, so the
            // bare scan run through them would shear. At the scan's own size: `Renderer`
            // takes its radii in scan pixels and derives its scale from the image width.
            let canvas = sigil::over(&cfg, &sigil::Figure::default(), &src, 1.0);

            let mut r = Renderer::new(
                canvas,
                (cfg.center[0], cfg.center[1]),
                ow,
                oh,
                bg,
                cfg.layers.clone(),
                ss,
            );
            r.tint = tint;
            if tint {
                println!("  layer colours, outermost first:");
                const NAMES: [&str; 7] =
                    ["red", "orange", "yellow", "green", "cyan", "blue", "magenta"];
                for (i, l) in r.layers.iter().enumerate() {
                    println!(
                        "    {}  {:<9}  {:<18}  turns {}",
                        i + 1,
                        NAMES[i.min(NAMES.len() - 1)],
                        l.name,
                        l.turns
                    );
                }
            }

            println!("building {ncol}-colour global palette...");
            let pal = gifout::Palette::build(src.as_raw(), ncol, bg);

            let rect = r.disc_rect();
            println!(
                "output {ow}x{oh}, {n} frames @ {}fps, moving rect {}x{} at ({},{})",
                cfg.fps, rect.2, rect.3, rect.0, rect.1
            );

            if let Some(d) = &png_frames {
                std::fs::create_dir_all(d).unwrap();
            }

            let mut gif = gifout::GifWriter::new(&out, ow as u16, oh as u16, &pal, cfg.fps).unwrap();
            let total = if loop_check { n + 1 } else { n };
            let mut frame0: Option<Vec<u8>> = None;
            let mut shown: Option<Vec<u8>> = None;
            let mut opaque_sum = 0.0f32;

            for i in 0..total {
                let buf = r.frame(i, n);

                if let Some(d) = &png_frames {
                    RgbImage::from_raw(ow, oh, buf.clone())
                        .unwrap()
                        .save(format!("{d}/frame_{i:04}.png"))
                        .unwrap();
                }

                if i == 0 {
                    frame0 = Some(buf.clone());
                    // Frame 0 is full-size and opaque, so the ground outside the disc is
                    // painted once and every later frame can draw over just the disc.
                    gif.write(pal.map_rect(&buf, ow, (0, 0, ow, oh)), (0, 0, ow, oh), None)
                        .unwrap();
                    shown = Some(pal.map_rect(&buf, ow, rect));
                } else if i < n {
                    let mut idx = pal.map_rect(&buf, ow, rect);
                    let opaque = pal.delta(&mut idx, shown.as_mut().unwrap(), tol);
                    opaque_sum += opaque;
                    gif.write(idx, rect, Some(pal.transparent)).unwrap();
                }

                if loop_check && i == n {
                    let same = frame0.as_ref().map(|f| *f == buf).unwrap_or(false);
                    println!(
                        "loop check: frame {n} vs frame 0 -> {}",
                        if same { "IDENTICAL (seamless)" } else { "DIFFERENT" }
                    );
                }
                print!("\rframe {}/{}", i + 1, total);
                use std::io::Write;
                std::io::stdout().flush().ok();
            }
            println!(
                "\nwrote {out}  (mean {:.0}% of the disc redrawn per frame)",
                100.0 * opaque_sum / (n - 1).max(1) as f32
            );
        }
    }
}
