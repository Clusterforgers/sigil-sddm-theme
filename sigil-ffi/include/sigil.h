// The sigil engine's C interface. Mirrors sigil-ffi/src/lib.rs; keep the two in step.
//
// One engine, used from one thread at a time. Byte views stay valid until the next call of
// the same kind or until the engine is freed, so copy them straight away.

#pragma once

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct SigilEngine SigilEngine;

typedef struct {
    const uint8_t *data;
    size_t len;
    uint32_t levels;
} SigilBytes;

// Null on failure, with the reason in *error (free it with sigil_string_free). overrides,
// if not null, names a JSON file of settings laid over the figure; it may not exist.
SigilEngine *sigil_engine_new(const char *path, const char *overrides, float canvas_scale, uint32_t ss,
                              char **error);
void sigil_engine_free(SigilEngine *e);
void sigil_string_free(char *s);

uint32_t sigil_layer_count(const SigilEngine *e);
uint32_t sigil_layer_side(const SigilEngine *e);
uint32_t sigil_reveal_side(const SigilEngine *e);

// Each layer's mip chain, level 0 first, each level tightly packed.
SigilBytes sigil_layer_art(SigilEngine *e, uint32_t layer);    // sRGB RGBA8, premultiplied
SigilBytes sigil_layer_glow(SigilEngine *e, uint32_t layer);   // linear R8
SigilBytes sigil_layer_reveal(SigilEngine *e, uint32_t layer); // RG8, one level

size_t sigil_uniforms_size(void);
// Advance by dt seconds and lay out a frame with the figure fitted into the pixel
// rectangle (x, y, w, h); returns the uniform block.
// flip_height: the frame height if the framebuffer is y-up, else 0.
// encode_srgb: nonzero if the target stores values as given rather than encoding sRGB.
SigilBytes sigil_frame(SigilEngine *e, float dt, float x, float y, float w, float h, float flip_height,
                       int32_t encode_srgb);

void sigil_key(SigilEngine *e);
void sigil_backspace(SigilEngine *e);
int32_t sigil_surge(SigilEngine *e);          // nonzero if it started
void sigil_fail(SigilEngine *e);
int32_t sigil_take_detonated(SigilEngine *e); // nonzero once per explosion
void sigil_pointer(SigilEngine *e, int32_t present, float x, float y);
void sigil_skip_build(SigilEngine *e);
int32_t sigil_system_info(const SigilEngine *e); // nonzero if the figure wants the system panel

#ifdef __cplusplus
}
#endif
