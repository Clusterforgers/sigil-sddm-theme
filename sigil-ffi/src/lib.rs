//! The engine behind a C interface, for the Qt plugin that draws the sigil on the login
//! screen. `include/sigil.h` declares the same calls; keep the two in step.
//!
//! Every call takes the engine made by `sigil_engine_new` and is safe to make from any one
//! thread at a time. Byte views it hands back stay valid until the next call of the same
//! kind, or until the engine is freed; the plugin copies them straight away.

use imagespin::engine::Engine;
use imagespin::figure::Figure;
use imagespin::gpu::{glow_pyramid, source_pyramid};

use std::ffi::{c_char, CStr, CString};
use std::ptr;

pub struct SigilEngine {
    engine: Engine,
    /// What the last byte-returning call handed out, kept alive for the caller to copy.
    art: Vec<u8>,
    glow: Vec<u8>,
    reveal: Vec<u8>,
    uniforms: Vec<u8>,
}

/// A view of bytes owned by the engine, and how many mip levels they hold.
#[repr(C)]
pub struct SigilBytes {
    pub data: *const u8,
    pub len: usize,
    pub levels: u32,
}

impl SigilBytes {
    fn of(v: &[u8], levels: u32) -> Self {
        SigilBytes { data: v.as_ptr(), len: v.len(), levels }
    }
}

/// Load the figure at `path`, with the JSON settings in `overrides` laid over it if that is
/// not null and the file exists, and draw its layers at `canvas_scale` texels per canvas
/// unit, supersampled `ss` times. Null on failure, with the reason in `*error` if `error` is not
/// null; free that with `sigil_string_free`.
///
/// # Safety
/// `path` is a NUL-terminated string, `overrides` null or one; `error` is null or points at
/// writable storage.
#[no_mangle]
pub unsafe extern "C" fn sigil_engine_new(
    path: *const c_char,
    overrides: *const c_char,
    canvas_scale: f32,
    ss: u32,
    error: *mut *mut c_char,
) -> *mut SigilEngine {
    let fail = |msg: String| {
        if !error.is_null() {
            *error = CString::new(msg).unwrap_or_default().into_raw();
        }
        ptr::null_mut()
    };
    if path.is_null() {
        return fail("no figure path".into());
    }
    let path = CStr::from_ptr(path).to_string_lossy().into_owned();
    let loaded = if overrides.is_null() {
        Figure::load(&path)
    } else {
        Figure::load_with(&path, CStr::from_ptr(overrides).to_string_lossy().as_ref())
    };
    let fig = match loaded {
        Ok(f) => f.render(canvas_scale),
        Err(e) => return fail(e.to_string()),
    };
    let engine = Engine::new(fig, ss.clamp(1, 4));
    Box::into_raw(Box::new(SigilEngine { engine, art: Vec::new(), glow: Vec::new(), reveal: Vec::new(), uniforms: Vec::new() }))
}

/// # Safety
/// `e` came from `sigil_engine_new` and is not used again.
#[no_mangle]
pub unsafe extern "C" fn sigil_engine_free(e: *mut SigilEngine) {
    if !e.is_null() {
        drop(Box::from_raw(e));
    }
}

/// # Safety
/// `s` came from this library and is not used again.
#[no_mangle]
pub unsafe extern "C" fn sigil_string_free(s: *mut c_char) {
    if !s.is_null() {
        drop(CString::from_raw(s));
    }
}

/// How many layers there are: the depth of each texture array.
#[no_mangle]
pub extern "C" fn sigil_layer_count(e: &SigilEngine) -> u32 {
    e.engine.fig.layers.len() as u32
}

/// The side of each layer's artwork and glow, in texels, at mip level 0.
#[no_mangle]
pub extern "C" fn sigil_layer_side(e: &SigilEngine) -> u32 {
    e.engine.fig.layers.first().map_or(0, |l| l.image.width())
}

/// The side of each layer's reveal map, in texels. It has no mips.
#[no_mangle]
pub extern "C" fn sigil_reveal_side(e: &SigilEngine) -> u32 {
    e.engine.fig.layers.first().map_or(0, |l| l.reveal_side)
}

/// Layer `i`'s artwork and its mips, smallest last: sRGB-encoded premultiplied RGBA8.
#[no_mangle]
pub extern "C" fn sigil_layer_art(e: &mut SigilEngine, i: u32) -> SigilBytes {
    let Some(l) = e.engine.fig.layers.get(i as usize) else { return SigilBytes::of(&[], 0) };
    let (bytes, levels) = source_pyramid(&l.image);
    e.art = bytes;
    SigilBytes::of(&e.art, levels)
}

/// Layer `i`'s bloom highlight and its mips, smallest last: linear R8.
#[no_mangle]
pub extern "C" fn sigil_layer_glow(e: &mut SigilEngine, i: u32) -> SigilBytes {
    let Some(l) = e.engine.fig.layers.get(i as usize) else { return SigilBytes::of(&[], 0) };
    let (bytes, levels) = glow_pyramid(&l.image);
    e.glow = bytes;
    SigilBytes::of(&e.glow, levels)
}

/// Layer `i`'s reveal map: RG8, one level.
#[no_mangle]
pub extern "C" fn sigil_layer_reveal(e: &mut SigilEngine, i: u32) -> SigilBytes {
    let Some(l) = e.engine.fig.layers.get(i as usize) else { return SigilBytes::of(&[], 0) };
    e.reveal = l.reveal.clone();
    SigilBytes::of(&e.reveal, 1)
}

/// The size of the uniform block, in bytes.
#[no_mangle]
pub extern "C" fn sigil_uniforms_size() -> usize {
    std::mem::size_of::<imagespin::gpu::Uniforms>()
}

/// Advance by `dt` seconds and lay out a frame with the figure fitted into the rectangle
/// `(x, y, w, h)`, in the frame's pixels. `flip_height` is the frame's height if the
/// framebuffer runs y-up, else 0; `encode_srgb` is nonzero if the target does not encode
/// sRGB itself. The bytes are the uniform block for the frame.
#[no_mangle]
pub extern "C" fn sigil_frame(
    e: &mut SigilEngine,
    dt: f32,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    flip_height: f32,
    encode_srgb: i32,
) -> SigilBytes {
    let uni = e.engine.frame_in(dt, [x, y, w, h]);
    let mut uni = *uni;
    uni.host = [flip_height, if encode_srgb != 0 { 1.0 } else { 0.0 }];
    e.uniforms.clear();
    e.uniforms.extend_from_slice(bytemuck::bytes_of(&uni));
    SigilBytes::of(&e.uniforms, 1)
}

/// A character was typed.
#[no_mangle]
pub extern "C" fn sigil_key(e: &mut SigilEngine) {
    e.engine.fx.key();
}

/// Backspace.
#[no_mangle]
pub extern "C" fn sigil_backspace(e: &mut SigilEngine) {
    e.engine.fx.backspace();
}

/// Enter: start the surge. Nonzero if it started, zero if one was already running.
#[no_mangle]
pub extern "C" fn sigil_surge(e: &mut SigilEngine) -> i32 {
    e.engine.fx.surge() as i32
}

/// The password was refused.
#[no_mangle]
pub extern "C" fn sigil_fail(e: &mut SigilEngine) {
    e.engine.fx.fail();
}

/// Nonzero once for each explosion of the surge.
#[no_mangle]
pub extern "C" fn sigil_take_detonated(e: &mut SigilEngine) -> i32 {
    e.engine.fx.take_detonated() as i32
}

/// The pointer is at pixel `(x, y)` of the last frame, or has left if `present` is zero.
#[no_mangle]
pub extern "C" fn sigil_pointer(e: &mut SigilEngine, present: i32, x: f32, y: f32) {
    e.engine.pointer((present != 0).then_some([x, y]));
}

/// Nonzero if the figure file asks for the system panel on the login screen.
#[no_mangle]
pub extern "C" fn sigil_system_info(e: &SigilEngine) -> i32 {
    e.engine.fig.login.system_info as i32
}

/// Show the figure whole at once rather than drawing it in.
#[no_mangle]
pub extern "C" fn sigil_skip_build(e: &mut SigilEngine) {
    e.engine.fx.skip_build();
}
