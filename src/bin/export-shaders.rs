//! Write the fragment shader as GLSL for the Qt plugin's build to compile with `qsb`.
//!
//!     cargo run --bin export-shaders -- qml-plugin/shaders/sigil.frag

use std::process::ExitCode;

fn main() -> ExitCode {
    let out = std::env::args().nth(1).unwrap_or_else(|| "sigil.frag".into());
    let glsl = match imagespin::gpu::shader::fragment_glsl() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(e) = std::fs::write(&out, glsl) {
        eprintln!("error: cannot write {out}: {e}");
        return ExitCode::FAILURE;
    }
    println!("wrote {out}");
    ExitCode::SUCCESS
}
