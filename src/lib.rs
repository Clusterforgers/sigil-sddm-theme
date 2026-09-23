//! Spin each layer of a concentric sigil independently.
//!
//! The same layer geometry drives both the offline GIF renderer (`render`, `gifout`)
//! and the real-time viewer (`src/bin/live.rs`), which reimplements `geom::radius_at`
//! in WGSL. Keep the two in step.

pub mod config;
pub mod fit;
pub mod geom;
pub mod gifout;
pub mod gpu;
pub mod render;
