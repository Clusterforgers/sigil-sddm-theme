use imagespin::figure::{Figure, LAYER_TINT};

use clap::{Parser, Subcommand};
use image::{Rgb, RgbImage};
use std::f32::consts::TAU;
use std::process::ExitCode;

#[derive(Parser)]
#[command(about = "Check, draw and inspect a figure file")]
struct Cli {
    /// The figure to work on.
    #[arg(long, short, default_value = "figure.json5", global = true)]
    figure: String,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Validate the figure and list its layers.
    Check,
    /// Draw the figure as one flat PNG.
    Draw {
        #[arg(long, default_value = "sigil.png")]
        out: String,
        /// Pixels per canvas unit.
        #[arg(long, default_value_t = 1.0)]
        scale: f32,
        /// Paint each layer in its own colour, to see which element is in which layer.
        #[arg(long)]
        tint: bool,
    },
    /// Write each layer as its own transparent PNG.
    Layers {
        #[arg(long, default_value = "layers")]
        out: String,
        #[arg(long, default_value_t = 1.0)]
        scale: f32,
    },
    /// Ring every letter and symbol the viewer can light, coloured by layer.
    Glyphs {
        #[arg(long, default_value = "glyphs.png")]
        out: String,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let fig = match Figure::load(&cli.figure) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };

    let written = match cli.cmd {
        Cmd::Check => {
            check(&fig);
            return ExitCode::SUCCESS;
        }
        Cmd::Draw { out, scale, tint } => {
            if tint {
                legend(&fig);
            }
            save(&fig.composite(scale, tint), &out)
        }
        Cmd::Layers { out, scale } => write_layers(&fig, &out, scale),
        Cmd::Glyphs { out } => save(&glyph_map(&fig), &out),
    };
    match written {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Print the layer table and any warnings.
fn check(fig: &Figure) {
    let r = fig.render(1.0);
    println!("  bottom to top:");
    println!("  {:<3} {:<20} {:>4} {:>6} {:>9} {:>16}", "#", "layer", "z", "turns", "elements", "ink radius");
    for (i, (spec, layer)) in fig.spec().layers.iter().zip(&r.layers).enumerate() {
        println!(
            "  {:<3} {:<20} {:>4} {:>+6} {:>9} {:>7.1} – {:<7.1}",
            i + 1,
            spec.name,
            spec.z,
            spec.turns,
            spec.elements.len(),
            layer.extent[0],
            layer.extent[1]
        );
    }
    println!("\n  {} glyphs; layer textures cover {:.0} units square", r.glyphs.len(), r.frame.side);
    for w in &r.warnings {
        println!("  warning: {w}");
    }
    println!("  ok");
}

fn legend(fig: &Figure) {
    for (i, l) in fig.spec().layers.iter().enumerate() {
        let [r, g, b] = LAYER_TINT[i % LAYER_TINT.len()];
        println!("  \x1b[38;2;{r};{g};{b}m■\x1b[0m {}", l.name);
    }
}

fn save(img: &RgbImage, out: &str) -> Result<(), String> {
    img.save(out).map_err(|e| format!("cannot write {out}: {e}"))?;
    println!("wrote {out}  ({}x{})", img.width(), img.height());
    Ok(())
}

fn write_layers(fig: &Figure, dir: &str, scale: f32) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {dir}: {e}"))?;
    for (i, l) in fig.render(scale).layers.iter().enumerate() {
        let path = format!("{dir}/{:02}-{}.png", i + 1, l.name.replace(' ', "-"));
        // Un-premultiply, so the PNG shows the ink rather than a darkened fringe.
        let mut img = l.image.clone();
        for p in img.pixels_mut() {
            let a = p.0[3] as f32 / 255.0;
            if a > 0.0 {
                for c in &mut p.0[..3] {
                    *c = (*c as f32 / a).round().min(255.0) as u8;
                }
            }
        }
        img.save(&path).map_err(|e| format!("cannot write {path}: {e}"))?;
        println!("wrote {path}");
    }
    Ok(())
}

/// The figure, with a ring round every glyph in its layer's colour.
fn glyph_map(fig: &Figure) -> RgbImage {
    let mut img = fig.composite(1.0, false);
    let glyphs = fig.render(1.0).glyphs;
    for g in &glyphs {
        let col = Rgb(LAYER_TINT[g.layer % LAYER_TINT.len()]);
        for i in 0..64 {
            let a = TAU * i as f32 / 64.0;
            let (x, y) = (g.pos[0] + g.radius * a.cos(), g.pos[1] + g.radius * a.sin());
            if x >= 0.0 && y >= 0.0 && (x as u32) < img.width() && (y as u32) < img.height() {
                img.put_pixel(x as u32, y as u32, col);
            }
        }
    }
    println!("  {} glyphs", glyphs.len());
    img
}
