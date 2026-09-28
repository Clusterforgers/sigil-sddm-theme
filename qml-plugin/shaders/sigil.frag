#version 440 core
struct GpuLayer {
    vec4 motion;
    vec4 extent;
    vec4 form;
};
struct GpuPulse {
    vec4 wave;
    vec4 splash;
};
struct GpuFlare {
    vec4 at;
};
struct GpuLimb {
    vec4 seg;
    vec4 style;
};
struct GpuWave {
    vec4 front;
    vec4 color;
};
struct GpuSwap {
    vec4 slot;
    vec4 source;
    vec4 map;
};
struct GpuThread {
    vec4 seg;
    vec4 style;
};
struct GpuRipple {
    vec4 at;
};
struct GpuMark {
    vec4 slot;
    vec4 glow;
};
struct GpuGlyph {
    vec4 slot;
    vec4 risen;
    vec4 turn;
};
struct Uniforms {
    vec2 center;
    vec2 host;
    vec4 frame;
    vec4 fit;
    vec4 quality;
    vec4 live;
    vec4 look;
    vec4 ambient;
    vec4 rhythm;
    vec4 live2_;
    vec4 pointer;
    vec4 gbox;
    vec4 gbox_p;
    vec4 background;
    vec4 pulses[32];
    vec4 flares[16];
    vec4 limbs[192];
    vec4 glyphs[96];
    vec4 waves[8];
    vec4 swaps[36];
    vec4 threads[32];
    vec4 ripples[8];
    vec4 marks[48];
    vec4 layers[48];
};
struct Swapped {
    vec4 ink;
    float hide;
};
struct Fields {
    vec4 blood;
    vec2 arc;
    float flare;
    vec4 tint;
    float sheen;
    float threads;
    float ripple;
    float near;
    float halo;
};
const float PI = 3.1415927;
const float TAU = 6.2831855;
const uint MAX_LAYERS = 16u;
const uint MAX_PULSES = 16u;
const uint MAX_FLARES = 16u;
const uint MAX_BOLT_SEGS = 96u;
const uint MAX_GLYPHS = 32u;
const uint MAX_WAVES = 4u;
const uint MAX_SWAPS = 12u;
const uint MAX_THREADS = 16u;
const uint MAX_RIPPLES = 8u;
const uint MAX_MARKS = 24u;
const vec3 LUMA = vec3(0.299, 0.587, 0.114);
const vec3 GLOW = vec3(1.0, 0.74, 0.3);
const vec3 BLOOD = vec3(0.42, 0.012, 0.012);
const vec3 BLOOD_HOT = vec3(0.9, 0.06, 0.03);
const float WARP_GAIN = 9.0;
const vec3 SPARK = vec3(0.22, 0.52, 1.0);
const vec3 SPARK_CORE = vec3(0.72, 0.88, 1.0);
const vec3 GLYPH_HOT = vec3(1.0, 0.92, 0.62);
const vec3 GLYPH_GONE = vec3(0.62, 0.3, 1.0);
const vec3 SHEEN = vec3(1.0, 0.92, 0.72);
const vec3 EMBER = vec3(1.0, 0.42, 0.08);
const vec3 PEN = vec3(1.0, 0.86, 0.55);
const vec3 THREAD = vec3(1.0, 0.95, 0.82);
const vec3 HOVER = vec3(1.0, 0.85, 0.55);
const vec3 MARK = vec3(1.0, 0.9, 0.62);
const float LAYER_REACH = 16.0;
const vec3 BLAST = vec3(1.0, 0.93, 0.8);

layout(std140, binding = 0) uniform Uniforms_block_0Fragment { Uniforms _group_0_binding_0_fs; };

layout(binding = 1) uniform sampler2DArray _group_0_binding_1_fs;

layout(binding = 3) uniform sampler2DArray _group_0_binding_3_fs;

layout(binding = 4) uniform sampler2DArray _group_0_binding_4_fs;

layout(location = 0) out vec4 _fs2p_location0;

GpuPulse uni_pulse(uint i_2) {
    vec4 _e6 = _group_0_binding_0_fs.pulses[(2u * i_2)];
    vec4 _e14 = _group_0_binding_0_fs.pulses[((2u * i_2) + 1u)];
    return GpuPulse(_e6, _e14);
}

GpuFlare uni_flare(uint i_3) {
    vec4 _e4 = _group_0_binding_0_fs.flares[i_3];
    return GpuFlare(_e4);
}

GpuLimb uni_limb(uint i_4) {
    vec4 _e6 = _group_0_binding_0_fs.limbs[(2u * i_4)];
    vec4 _e14 = _group_0_binding_0_fs.limbs[((2u * i_4) + 1u)];
    return GpuLimb(_e6, _e14);
}

GpuGlyph uni_glyph(uint i_5) {
    vec4 _e6 = _group_0_binding_0_fs.glyphs[(3u * i_5)];
    vec4 _e14 = _group_0_binding_0_fs.glyphs[((3u * i_5) + 1u)];
    vec4 _e22 = _group_0_binding_0_fs.glyphs[((3u * i_5) + 2u)];
    return GpuGlyph(_e6, _e14, _e22);
}

GpuWave uni_wave(uint i_6) {
    vec4 _e6 = _group_0_binding_0_fs.waves[(2u * i_6)];
    vec4 _e14 = _group_0_binding_0_fs.waves[((2u * i_6) + 1u)];
    return GpuWave(_e6, _e14);
}

GpuSwap uni_swap(uint i_7) {
    vec4 _e6 = _group_0_binding_0_fs.swaps[(3u * i_7)];
    vec4 _e14 = _group_0_binding_0_fs.swaps[((3u * i_7) + 1u)];
    vec4 _e22 = _group_0_binding_0_fs.swaps[((3u * i_7) + 2u)];
    return GpuSwap(_e6, _e14, _e22);
}

GpuThread uni_thread(uint i_8) {
    vec4 _e6 = _group_0_binding_0_fs.threads[(2u * i_8)];
    vec4 _e14 = _group_0_binding_0_fs.threads[((2u * i_8) + 1u)];
    return GpuThread(_e6, _e14);
}

GpuRipple uni_ripple(uint i_9) {
    vec4 _e4 = _group_0_binding_0_fs.ripples[i_9];
    return GpuRipple(_e4);
}

GpuMark uni_mark(uint i_10) {
    vec4 _e6 = _group_0_binding_0_fs.marks[(2u * i_10)];
    vec4 _e14 = _group_0_binding_0_fs.marks[((2u * i_10) + 1u)];
    return GpuMark(_e6, _e14);
}

GpuLayer uni_layer(uint i_11) {
    vec4 _e6 = _group_0_binding_0_fs.layers[(3u * i_11)];
    vec4 _e14 = _group_0_binding_0_fs.layers[((3u * i_11) + 1u)];
    vec4 _e22 = _group_0_binding_0_fs.layers[((3u * i_11) + 2u)];
    return GpuLayer(_e6, _e14, _e22);
}

vec2 art_uv(vec2 p) {
    vec4 _e3 = _group_0_binding_0_fs.frame;
    float _e9 = _group_0_binding_0_fs.frame.z;
    return ((p - _e3.xy) / vec2(_e9));
}

vec4 art(vec2 p_1, uint k, float lod) {
    vec2 _e5 = art_uv(p_1);
    vec4 _e6 = textureLod(_group_0_binding_1_fs, vec3(_e5, k), lod);
    return _e6;
}

vec3 layer_tint(uint k_1) {
    switch(k_1) {
        case 0u: {
            return vec3(1.0, 0.28, 0.28);
        }
        case 1u: {
            return vec3(1.0, 0.62, 0.16);
        }
        case 2u: {
            return vec3(0.95, 0.95, 0.22);
        }
        case 3u: {
            return vec3(0.32, 0.95, 0.38);
        }
        case 4u: {
            return vec3(0.26, 0.9, 0.96);
        }
        case 5u: {
            return vec3(0.48, 0.58, 1.0);
        }
        default: {
            return vec3(0.96, 0.42, 0.96);
        }
    }
}

float hash2_(vec2 p_2) {
    return fract((sin(dot(p_2, vec2(127.1, 311.7))) * 43758.547));
}

float noise2_(vec2 p_3) {
    vec2 i_23 = floor(p_3);
    vec2 f_3 = fract(p_3);
    vec2 u_1 = ((f_3 * f_3) * (vec2(3.0) - (2.0 * f_3)));
    float _e10 = hash2_(i_23);
    float _e15 = hash2_((i_23 + vec2(1.0, 0.0)));
    float _e20 = hash2_((i_23 + vec2(0.0, 1.0)));
    float _e25 = hash2_((i_23 + vec2(1.0, 1.0)));
    return mix(mix(_e10, _e15, u_1.x), mix(_e20, _e25, u_1.x), u_1.y);
}

vec3 spill(vec2 q, uint k_2, float amount, vec3 tint) {
    float acc_1 = 0.0;
    float radius = (1.5 + (5.5 * min(amount, 1.0)));
    float _e16 = _group_0_binding_0_fs.quality.z;
    float lod_1 = clamp(log2((radius * 0.5)), 0.0, _e16);
    vec2 _e19 = art_uv(q);
    float _e25 = _group_0_binding_0_fs.frame.z;
    float o = ((radius * 0.35) / _e25);
    vec4 _e31 = textureLod(_group_0_binding_3_fs, vec3((_e19 + vec2(o, o)), k_2), lod_1);
    acc_1 = _e31.x;
    float _e34 = acc_1;
    vec4 _e40 = textureLod(_group_0_binding_3_fs, vec3((_e19 + vec2(-(o), o)), k_2), lod_1);
    acc_1 = (_e34 + _e40.x);
    float _e43 = acc_1;
    vec4 _e49 = textureLod(_group_0_binding_3_fs, vec3((_e19 + vec2(o, -(o))), k_2), lod_1);
    acc_1 = (_e43 + _e49.x);
    float _e52 = acc_1;
    vec4 _e59 = textureLod(_group_0_binding_3_fs, vec3((_e19 + vec2(-(o), -(o))), k_2), lod_1);
    acc_1 = (_e52 + _e59.x);
    float _e62 = acc_1;
    return (((tint * (_e62 * 0.25)) * amount) * 1.7);
}

vec4 pulse_at(float r) {
    float body = 0.0;
    float edge = 0.0;
    float trough = 0.0;
    float warp = 0.0;
    uint i_12 = 0u;
    float env = 0.0;
    float _e12 = _group_0_binding_0_fs.live.x;
    uint n = uint(_e12);
    bool loop_init = true;
    while(true) {
        if (!loop_init) {
            uint _e122 = i_12;
            i_12 = (_e122 + 1u);
        }
        loop_init = false;
        uint _e16 = i_12;
        if ((_e16 < n)) {
        } else {
            break;
        }
        {
            uint _e18 = i_12;
            GpuPulse _e19 = uni_pulse(_e18);
            vec4 p_16 = _e19.wave;
            uint _e21 = i_12;
            GpuPulse _e22 = uni_pulse(_e21);
            vec4 x = _e22.splash;
            if ((x.x > 0.0)) {
                float t_1 = max((1.0 - ((r * r) / (x.y * x.y))), 0.0);
                float _e36 = body;
                body = (_e36 + ((x.x * t_1) * t_1));
                float _e41 = edge;
                edge = (_e41 + ((((x.x * t_1) * t_1) * t_1) * 0.8));
            }
            if ((abs((r - p_16.x)) > (p_16.z * 7.0))) {
                continue;
            }
            float s = ((x.z * (p_16.x - r)) / p_16.z);
            env = 0.0;
            if ((s < 0.0)) {
                float a_1 = (s * 2.6);
                env = exp((-(a_1) * a_1));
            } else {
                env = exp((-(s) * 1.8));
            }
            float ripple = abs(cos(((TAU * (p_16.x - r)) / p_16.w)));
            float _e83 = body;
            float _e85 = env;
            body = (_e83 + ((p_16.y * _e85) * mix(1.0, ripple, clamp((s * 0.6), 0.0, 0.85))));
            float e = (s * 2.8);
            float _e98 = edge;
            edge = (_e98 + (p_16.y * exp((-(e) * e))));
            float t_2 = ((s + 1.35) * 2.2);
            float _e109 = trough;
            trough = (_e109 + (p_16.y * exp((-(t_2) * t_2))));
            float _e116 = warp;
            float _e119 = env;
            warp = (_e116 + ((p_16.y * s) * _e119));
        }
    }
    float _e125 = body;
    float _e126 = edge;
    float _e127 = trough;
    float _e128 = warp;
    return vec4(_e125, _e126, _e127, _e128);
}

float flare_at(vec2 p_4) {
    float acc_2 = 0.0;
    uint i_13 = 0u;
    float _e6 = _group_0_binding_0_fs.live.y;
    uint n_1 = uint(_e6);
    bool loop_init_1 = true;
    while(true) {
        if (!loop_init_1) {
            uint _e30 = i_13;
            i_13 = (_e30 + 1u);
        }
        loop_init_1 = false;
        uint _e10 = i_13;
        if ((_e10 < n_1)) {
        } else {
            break;
        }
        {
            uint _e12 = i_13;
            GpuFlare _e13 = uni_flare(_e12);
            vec4 s_1 = _e13.at;
            vec2 d = ((p_4 - s_1.xy) / vec2(s_1.z));
            float t_3 = max((1.0 - dot(d, d)), 0.0);
            float _e25 = acc_2;
            acc_2 = (_e25 + ((s_1.w * t_3) * t_3));
        }
    }
    float _e33 = acc_2;
    return _e33;
}

float seg_dist2_(vec2 p_5, vec2 a, vec2 b) {
    vec2 ab = (b - a);
    float t_4 = clamp((dot((p_5 - a), ab) / max(dot(ab, ab), 1e-6)), 0.0, 1.0);
    vec2 d_1 = (p_5 - (a + (ab * t_4)));
    return dot(d_1, d_1);
}

vec2 bolt_at(vec2 p_6) {
    float core = 0.0;
    float halo = 0.0;
    uint i_14 = 0u;
    bool local = false;
    bool local_1 = false;
    bool local_2 = false;
    float _e8 = _group_0_binding_0_fs.live.z;
    uint n_2 = uint(_e8);
    bool loop_init_2 = true;
    while(true) {
        if (!loop_init_2) {
            uint _e86 = i_14;
            i_14 = (_e86 + 1u);
        }
        loop_init_2 = false;
        uint _e12 = i_14;
        if ((_e12 < n_2)) {
        } else {
            break;
        }
        {
            uint _e14 = i_14;
            GpuLimb _e15 = uni_limb(_e14);
            vec4 s_2 = _e15.seg;
            uint _e17 = i_14;
            GpuLimb _e18 = uni_limb(_e17);
            vec4 w_1 = _e18.style;
            float reach = (w_1.y * 12.0);
            vec2 lo = (min(s_2.xy, s_2.zw) - vec2(reach));
            vec2 hi = (max(s_2.xy, s_2.zw) + vec2(reach));
            if (!((p_6.x < lo.x))) {
                local = (p_6.y < lo.y);
            } else {
                local = true;
            }
            bool _e43 = local;
            if (!(_e43)) {
                local_1 = (p_6.x > hi.x);
            } else {
                local_1 = true;
            }
            bool _e51 = local_1;
            if (!(_e51)) {
                local_2 = (p_6.y > hi.y);
            } else {
                local_2 = true;
            }
            bool _e59 = local_2;
            if (_e59) {
                continue;
            }
            float _e62 = seg_dist2_(p_6, s_2.xy, s_2.zw);
            float w2_ = (w_1.y * w_1.y);
            float f_4 = (w2_ / (_e62 + w2_));
            float _e68 = core;
            core = (_e68 + ((w_1.x * f_4) * f_4));
            float t_5 = max((1.0 - (_e62 / (reach * reach))), 0.0);
            float _e79 = halo;
            halo = (_e79 + (((w_1.x * t_5) * t_5) * 1.1));
        }
    }
    float _e89 = core;
    float _e90 = halo;
    return vec2(_e89, _e90);
}

vec3 glyph_colour(float t) {
    float k_12 = clamp((t / 0.55), 0.0, 1.0);
    if ((k_12 < 0.45)) {
        return mix(GLYPH_HOT, vec3(1.0), (k_12 / 0.45));
    }
    return mix(vec3(1.0), GLYPH_GONE, ((k_12 - 0.45) / 0.55));
}

float glyph_hide(vec2 p_7, uint k_3) {
    float acc_3 = 0.0;
    uint i_15 = 0u;
    float _e7 = _group_0_binding_0_fs.live.w;
    uint n_3 = uint(_e7);
    bool loop_init_3 = true;
    while(true) {
        if (!loop_init_3) {
            uint _e50 = i_15;
            i_15 = (_e50 + 1u);
        }
        loop_init_3 = false;
        uint _e11 = i_15;
        if ((_e11 < n_3)) {
        } else {
            break;
        }
        {
            uint _e13 = i_15;
            GpuGlyph _e14 = uni_glyph(_e13);
            if ((uint(_e14.turn.z) != k_3)) {
                continue;
            }
            uint _e19 = i_15;
            GpuGlyph _e20 = uni_glyph(_e19);
            vec4 a_2 = _e20.risen;
            uint _e22 = i_15;
            GpuGlyph _e23 = uni_glyph(_e22);
            vec4 g = _e23.slot;
            vec2 d_2 = ((p_7 - g.xy) / g.zw);
            float t_6 = max((1.0 - dot(d_2, d_2)), 0.0);
            if ((t_6 <= 0.0)) {
                continue;
            }
            float hide = (smoothstep(0.0, 0.12, a_2.w) * (1.0 - smoothstep(0.72, 1.0, a_2.w)));
            float _e47 = acc_3;
            acc_3 = (_e47 + (hide * t_6));
        }
    }
    float _e53 = acc_3;
    return min(_e53, 1.0);
}

vec3 glyph_ghost(vec2 p_8) {
    vec3 acc_4 = vec3(0.0);
    uint i_16 = 0u;
    float _e7 = _group_0_binding_0_fs.live.w;
    uint n_4 = uint(_e7);
    bool loop_init_4 = true;
    while(true) {
        if (!loop_init_4) {
            uint _e86 = i_16;
            i_16 = (_e86 + 1u);
        }
        loop_init_4 = false;
        uint _e11 = i_16;
        if ((_e11 < n_4)) {
        } else {
            break;
        }
        {
            uint _e13 = i_16;
            GpuGlyph _e14 = uni_glyph(_e13);
            vec4 a_3 = _e14.risen;
            if ((a_3.z <= 0.002)) {
                continue;
            }
            uint _e19 = i_16;
            GpuGlyph _e20 = uni_glyph(_e19);
            vec4 g_1 = _e20.slot;
            float grow = (1.0 + (1.6 * a_3.w));
            vec2 ext = (g_1.zw * grow);
            vec2 local_18 = (p_8 - a_3.xy);
            vec2 d_3 = (local_18 / ext);
            float t_7 = max((1.0 - dot(d_3, d_3)), 0.0);
            if ((t_7 <= 0.0)) {
                continue;
            }
            uint _e39 = i_16;
            GpuGlyph _e40 = uni_glyph(_e39);
            vec4 rot = _e40.turn;
            vec2 back = (vec2(((local_18.x * rot.y) - (local_18.y * rot.x)), ((local_18.y * rot.y) + (local_18.x * rot.x))) / vec2(grow));
            float _e66 = _group_0_binding_0_fs.quality.w;
            vec4 _e71 = art((g_1.xy + back), uint(rot.z), (_e66 + (a_3.w * 2.5)));
            vec3 ink_2 = _e71.xyz;
            float lum = dot(ink_2, LUMA);
            vec3 _e75 = acc_4;
            vec3 _e77 = glyph_colour(a_3.w);
            acc_4 = (_e75 + (_e77 * ((((lum * a_3.z) * t_7) * t_7) * 1.6)));
        }
    }
    vec3 _e89 = acc_4;
    return _e89;
}

float sheen_at(vec2 p_9) {
    float strength = _group_0_binding_0_fs.ambient.x;
    if ((strength <= 0.0)) {
        return 0.0;
    }
    vec2 _e10 = _group_0_binding_0_fs.center;
    vec2 d_4 = (p_9 - _e10);
    float alpha = atan(-(d_4.x), -(d_4.y));
    float _e20 = _group_0_binding_0_fs.look.w;
    float _e24 = _group_0_binding_0_fs.ambient.y;
    float phase = ((_e20 * _e24) * TAU);
    float band = (0.5 + (0.5 * cos(((2.0 * (alpha + phase)) + (length(d_4) * 0.012)))));
    return (strength * pow(band, 18.0));
}

vec2 haze_at(vec2 q_1, uint k_4) {
    float amount_2 = _group_0_binding_0_fs.ambient.z;
    if ((amount_2 <= 0.0)) {
        return vec2(0.0);
    }
    float t_8 = _group_0_binding_0_fs.look.w;
    float o_1 = (float(k_4) * 1.7);
    float _e27 = noise2_(((q_1 * 0.035) + vec2(((t_8 * 0.4) + o_1), (-(t_8) * 0.3))));
    float _e41 = noise2_((((q_1 * 0.035) + vec2((-(t_8) * 0.35), ((t_8 * 0.45) + o_1))) + vec2(31.7)));
    return (((vec2(_e27, _e41) - vec2(0.5)) * 2.0) * amount_2);
}

float breath(uint k_5) {
    float strength_1 = _group_0_binding_0_fs.rhythm.y;
    if ((strength_1 <= 0.0)) {
        return 1.0;
    }
    float _e11 = _group_0_binding_0_fs.look.w;
    float _e15 = _group_0_binding_0_fs.rhythm.z;
    float phase_1 = (((_e11 / max(_e15, 0.5)) * TAU) - (float(k_5) * 0.45));
    return (1.0 + (strength_1 * sin(phase_1)));
}

vec2 burn(vec2 q_2, uint k_6, float amount_1) {
    if ((amount_1 <= 0.0)) {
        return vec2(1.0, 0.0);
    }
    vec2 p_17 = ((q_2 * 0.03) + vec2((float(k_6) * 13.1), (float(k_6) * 7.3)));
    float _e18 = noise2_(p_17);
    float _e26 = noise2_(((p_17 * 2.7) + vec2(5.2)));
    float n_5 = ((0.65 * _e18) + (0.35 * _e26));
    float edge_1 = (n_5 - amount_1);
    float left = smoothstep(0.0, 0.03, edge_1);
    float glow_1 = ((left * (1.0 - smoothstep(0.03, 0.12, edge_1))) * min((amount_1 * 6.0), 1.0));
    return vec2(left, glow_1);
}

vec4 tint_at(float r_1) {
    vec4 acc_5 = vec4(0.0);
    uint i_17 = 0u;
    float env_1 = 0.0;
    float _e7 = _group_0_binding_0_fs.ambient.w;
    uint n_6 = uint(_e7);
    bool loop_init_5 = true;
    while(true) {
        if (!loop_init_5) {
            uint _e50 = i_17;
            i_17 = (_e50 + 1u);
        }
        loop_init_5 = false;
        uint _e11 = i_17;
        if ((_e11 < n_6)) {
        } else {
            break;
        }
        {
            uint _e13 = i_17;
            GpuWave _e14 = uni_wave(_e13);
            float s_3 = ((_e14.front.w * (_e14.front.x - r_1)) / _e14.front.y);
            env_1 = exp(((-(s_3) * s_3) * 2.0));
            if ((s_3 > 0.0)) {
                float _e32 = env_1;
                env_1 = max(_e32, (0.6 * exp((-(s_3) * 1.2))));
            }
            float _e42 = env_1;
            float k_13 = (_e14.front.z * _e42);
            vec4 _e44 = acc_5;
            acc_5 = (_e44 + vec4((_e14.color.xyz * k_13), k_13));
        }
    }
    vec4 _e53 = acc_5;
    return _e53;
}

Swapped swapped_at(vec2 q_3, uint k_7) {
    Swapped out_ = Swapped(vec4(0.0), 0.0);
    uint i_18 = 0u;
    bool local_3 = false;
    out_.ink = vec4(0.0);
    out_.hide = 0.0;
    float _e11 = _group_0_binding_0_fs.rhythm.x;
    uint n_7 = uint(_e11);
    bool loop_init_6 = true;
    while(true) {
        if (!loop_init_6) {
            uint _e90 = i_18;
            i_18 = (_e90 + 1u);
        }
        loop_init_6 = false;
        uint _e15 = i_18;
        if ((_e15 < n_7)) {
        } else {
            break;
        }
        {
            uint _e17 = i_18;
            GpuSwap _e18 = uni_swap(_e17);
            if (!((uint(_e18.slot.w) != k_7))) {
                local_3 = (_e18.source.w <= 0.0);
            } else {
                local_3 = true;
            }
            bool _e31 = local_3;
            if (_e31) {
                continue;
            }
            vec2 off = (q_3 - _e18.slot.xy);
            float d2_ = (dot(off, off) / (_e18.slot.z * _e18.slot.z));
            if ((d2_ >= 0.8)) {
                continue;
            }
            float t_9 = ((1.0 - smoothstep(0.35, 0.8, d2_)) * _e18.source.w);
            vec4 m = _e18.map;
            vec2 src = (_e18.source.xy + (vec2(((off.x * m.x) - (off.y * m.y)), ((off.x * m.y) + (off.y * m.x))) * m.z));
            vec4 _e75 = out_.ink;
            float _e82 = _group_0_binding_0_fs.quality.w;
            vec4 _e83 = art(src, uint(_e18.source.z), _e82);
            out_.ink = (_e75 + (_e83 * t_9));
            float _e88 = out_.hide;
            out_.hide = max(_e88, t_9);
        }
    }
    Swapped _e93 = out_;
    return _e93;
}

float reveal_at(vec2 q_4, uint k_8) {
    vec2 size = vec2(uvec2(textureSize(_group_0_binding_4_fs, 0).xy));
    vec2 _e5 = art_uv(q_4);
    ivec2 texel = ivec2(clamp((_e5 * size), vec2(0.0), (size - vec2(1.0))));
    vec4 _e16 = texelFetch(_group_0_binding_4_fs, ivec3(texel, k_8), 0);
    vec2 rg = _e16.xy;
    return ((((rg.x * 255.0) * 256.0) + (rg.y * 255.0)) / 65535.0);
}

vec4 built(vec4 ink, vec2 q_5, uint k_9) {
    bool local_4 = false;
    float progress = _group_0_binding_0_fs.rhythm.w;
    if (!((progress >= 1.0))) {
        local_4 = (ink.w <= 0.0);
    } else {
        local_4 = true;
    }
    bool _e16 = local_4;
    if (_e16) {
        return ink;
    }
    float _e17 = reveal_at(q_5, k_9);
    float since = (progress - _e17);
    float shown = clamp((since / 0.004), 0.0, 1.0);
    float tip = (shown * (1.0 - clamp((since / 0.035), 0.0, 1.0)));
    return vec4(((ink.xyz * shown) + (PEN * ((tip * ink.w) * 2.4))), (ink.w * shown));
}

float threads_at(vec2 p_10) {
    float acc_6 = 0.0;
    uint i_19 = 0u;
    bool local_5 = false;
    bool local_6 = false;
    bool local_7 = false;
    float _e6 = _group_0_binding_0_fs.live2_.x;
    uint n_8 = uint(_e6);
    bool loop_init_7 = true;
    while(true) {
        if (!loop_init_7) {
            uint _e105 = i_19;
            i_19 = (_e105 + 1u);
        }
        loop_init_7 = false;
        uint _e10 = i_19;
        if ((_e10 < n_8)) {
        } else {
            break;
        }
        {
            uint _e12 = i_19;
            GpuThread _e13 = uni_thread(_e12);
            vec2 a_4 = _e13.seg.xy;
            vec2 b_1 = _e13.seg.zw;
            vec2 lo_1 = (min(a_4, b_1) - vec2(14.0));
            vec2 hi_1 = (max(a_4, b_1) + vec2(14.0));
            if (!((p_10.x < lo_1.x))) {
                local_5 = (p_10.y < lo_1.y);
            } else {
                local_5 = true;
            }
            bool _e36 = local_5;
            if (!(_e36)) {
                local_6 = (p_10.x > hi_1.x);
            } else {
                local_6 = true;
            }
            bool _e44 = local_6;
            if (!(_e44)) {
                local_7 = (p_10.y > hi_1.y);
            } else {
                local_7 = true;
            }
            bool _e52 = local_7;
            if (_e52) {
                continue;
            }
            float _e53 = seg_dist2_(p_10, a_4, b_1);
            float line = ((exp((-(_e53) / 1.4)) * 0.9) + (exp((-(_e53) / 40.0)) * 0.22));
            vec2 spark = (a_4 + ((b_1 - a_4) * _e13.style.y));
            float ds = dot((p_10 - spark), (p_10 - spark));
            float stars = (exp((-(dot((p_10 - a_4), (p_10 - a_4))) / 30.0)) + exp((-(dot((p_10 - b_1), (p_10 - b_1))) / 30.0)));
            float _e90 = acc_6;
            acc_6 = (_e90 + (_e13.style.x * ((line + (exp((-(ds) / 14.0)) * 1.6)) + (stars * 0.7))));
        }
    }
    float _e108 = acc_6;
    return _e108;
}

float ripples_at(vec2 p_11) {
    float acc_7 = 0.0;
    uint i_20 = 0u;
    float _e6 = _group_0_binding_0_fs.live2_.y;
    uint n_9 = uint(_e6);
    bool loop_init_8 = true;
    while(true) {
        if (!loop_init_8) {
            uint _e29 = i_20;
            i_20 = (_e29 + 1u);
        }
        loop_init_8 = false;
        uint _e10 = i_20;
        if ((_e10 < n_9)) {
        } else {
            break;
        }
        {
            uint _e12 = i_20;
            GpuRipple _e13 = uni_ripple(_e12);
            vec4 r_2 = _e13.at;
            float d_5 = ((length((p_11 - r_2.xy)) - r_2.z) / 7.0);
            float _e22 = acc_7;
            acc_7 = (_e22 + (r_2.w * exp((-(d_5) * d_5))));
        }
    }
    float _e32 = acc_7;
    return _e32;
}

float near_at(vec2 p_12) {
    float presence = _group_0_binding_0_fs.pointer.z;
    if ((presence <= 0.0)) {
        return 0.0;
    }
    vec4 _e10 = _group_0_binding_0_fs.pointer;
    vec2 d_6 = (p_12 - _e10.xy);
    return (presence * exp((-(dot(d_6, d_6)) / 12100.0)));
}

float marks_at(vec2 q_6, uint k_10) {
    float acc_8 = 0.0;
    uint i_21 = 0u;
    float _e7 = _group_0_binding_0_fs.live2_.z;
    uint n_10 = uint(_e7);
    bool loop_init_9 = true;
    while(true) {
        if (!loop_init_9) {
            uint _e49 = i_21;
            i_21 = (_e49 + 1u);
        }
        loop_init_9 = false;
        uint _e11 = i_21;
        if ((_e11 < n_10)) {
        } else {
            break;
        }
        {
            uint _e13 = i_21;
            GpuMark _e14 = uni_mark(_e13);
            if ((uint(_e14.slot.w) != k_10)) {
                continue;
            }
            float d2_1 = (dot((q_6 - _e14.slot.xy), (q_6 - _e14.slot.xy)) / (_e14.slot.z * _e14.slot.z));
            float t_10 = (1.0 - smoothstep(0.3, 1.0, d2_1));
            float _e37 = acc_8;
            acc_8 = (_e37 + (t_10 * ((_e14.glow.x * 0.9) + (_e14.glow.y * 2.2))));
        }
    }
    float _e52 = acc_8;
    return _e52;
}

float marks_halo(vec2 p_13) {
    float acc_9 = 0.0;
    uint i_22 = 0u;
    float _e6 = _group_0_binding_0_fs.live2_.z;
    uint n_11 = uint(_e6);
    bool loop_init_10 = true;
    while(true) {
        if (!loop_init_10) {
            uint _e38 = i_22;
            i_22 = (_e38 + 1u);
        }
        loop_init_10 = false;
        uint _e10 = i_22;
        if ((_e10 < n_11)) {
        } else {
            break;
        }
        {
            uint _e12 = i_22;
            GpuMark _e13 = uni_mark(_e12);
            float reach_1 = (_e13.slot.z * 3.0);
            vec2 d_7 = (p_13 - _e13.glow.zw);
            float _e21 = acc_9;
            acc_9 = (_e21 + (((_e13.glow.x * 0.12) + (_e13.glow.y * 0.5)) * exp((-(dot(d_7, d_7)) / (reach_1 * reach_1)))));
        }
    }
    float _e41 = acc_9;
    return _e41;
}

Fields fields_at(vec2 p_14) {
    Fields f_1 = Fields(vec4(0.0), vec2(0.0), 0.0, vec4(0.0), 0.0, 0.0, 0.0, 0.0, 0.0);
    vec2 _e3 = _group_0_binding_0_fs.center;
    vec2 d_8 = (p_14 - _e3);
    vec4 _e8 = pulse_at(length(d_8));
    f_1.blood = _e8;
    vec2 _e10 = bolt_at(p_14);
    f_1.arc = _e10;
    float _e12 = flare_at(p_14);
    f_1.flare = _e12;
    vec4 _e15 = tint_at(length(d_8));
    f_1.tint = _e15;
    float _e17 = sheen_at(p_14);
    f_1.sheen = _e17;
    float _e19 = threads_at(p_14);
    f_1.threads = _e19;
    float _e21 = ripples_at(p_14);
    f_1.ripple = _e21;
    float _e23 = near_at(p_14);
    f_1.near = _e23;
    float _e25 = marks_halo(p_14);
    f_1.halo = _e25;
    Fields _e26 = f_1;
    return _e26;
}

vec3 shade(vec2 p_15, Fields f_2) {
    vec3 col_1 = vec3(0.0);
    vec3 glow = vec3(0.0);
    uint k_11 = 0u;
    bool local_8 = false;
    vec4 ink_1 = vec4(0.0);
    bool local_9 = false;
    float ec = 0.0;
    float es = 0.0;
    float w = 0.0;
    int j_1 = 0;
    bool local_10 = false;
    bool local_11 = false;
    bool local_12 = false;
    bool local_13 = false;
    vec3 risen = vec3(0.0);
    bool local_14 = false;
    bool local_15 = false;
    bool local_16 = false;
    bool local_17 = false;
    vec2 _e4 = _group_0_binding_0_fs.center;
    vec2 d_9 = (p_15 - _e4);
    float r2_ = dot(d_9, d_9);
    float _e10 = _group_0_binding_0_fs.quality.y;
    float _e16 = _group_0_binding_0_fs.quality.y;
    if ((r2_ >= ((_e10 + LAYER_REACH) * (_e16 + LAYER_REACH)))) {
        vec4 _e23 = _group_0_binding_0_fs.background;
        return _e23.xyz;
    }
    float r_3 = sqrt(r2_);
    float _e29 = _group_0_binding_0_fs.quality.y;
    float _e33 = _group_0_binding_0_fs.quality.y;
    float on_plate = (1.0 - smoothstep(_e29, (_e33 + LAYER_REACH), r_3));
    float red = (f_2.blood.x * on_plate);
    float edge_2 = (f_2.blood.y * on_plate);
    float trough_1 = (f_2.blood.z * on_plate);
    float warp_1 = f_2.blood.w;
    vec2 arc = f_2.arc;
    float blue = (arc.y + f_2.flare);
    vec4 _e56 = _group_0_binding_0_fs.background;
    col_1 = _e56.xyz;
    float _e65 = _group_0_binding_0_fs.fit.w;
    uint n_12 = uint(_e65);
    bool loop_init_11 = true;
    while(true) {
        if (!loop_init_11) {
            uint _e533 = k_11;
            k_11 = (_e533 + 1u);
        }
        loop_init_11 = false;
        uint _e69 = k_11;
        if ((_e69 < n_12)) {
        } else {
            break;
        }
        {
            uint _e71 = k_11;
            GpuLayer _e72 = uni_layer(_e71);
            if ((_e72.form.y <= 0.002)) {
                continue;
            }
            vec2 dl = (d_9 / vec2(_e72.form.x));
            float rl = (r_3 / _e72.form.x);
            if (!((rl < (_e72.extent.x - LAYER_REACH)))) {
                local_8 = (rl > (_e72.extent.y + LAYER_REACH));
            } else {
                local_8 = true;
            }
            bool _e98 = local_8;
            if (_e98) {
                continue;
            }
            float s_4 = _e72.motion.z;
            float c_1 = _e72.motion.w;
            vec2 _e105 = _group_0_binding_0_fs.center;
            vec2 q_7 = (_e105 + vec2(((dl.x * c_1) - (dl.y * s_4)), ((dl.y * c_1) + (dl.x * s_4))));
            uint _e118 = k_11;
            vec2 _e119 = haze_at(q_7, _e118);
            vec2 qh = (q_7 + _e119);
            vec2 _e123 = _group_0_binding_0_fs.center;
            float qr = length((qh - _e123));
            vec2 _e128 = _group_0_binding_0_fs.center;
            vec2 _e131 = _group_0_binding_0_fs.center;
            vec2 qw = (_e128 + ((qh - _e131) * ((qr + (warp_1 * WARP_GAIN)) / max(qr, 0.001))));
            uint _e141 = k_11;
            float _e145 = _group_0_binding_0_fs.quality.w;
            vec4 _e146 = art(qw, _e141, _e145);
            ink_1 = _e146;
            vec4 _e148 = ink_1;
            uint _e149 = k_11;
            vec4 _e150 = built(_e148, qh, _e149);
            ink_1 = _e150;
            float lag = _e72.form.w;
            float _e156 = _group_0_binding_0_fs.live2_.w;
            if ((_e156 > 0.0)) {
                local_9 = (abs(lag) > 0.08);
            } else {
                local_9 = false;
            }
            bool _e165 = local_9;
            if (_e165) {
                float cl = cos(lag);
                float sl = sin(lag);
                ec = c_1;
                es = s_4;
                float _e173 = _group_0_binding_0_fs.live2_.w;
                w = ((0.5 * _e173) * smoothstep(0.08, 0.35, abs(lag)));
                j_1 = 0;
                bool loop_init_12 = true;
                while(true) {
                    if (!loop_init_12) {
                        int _e250 = j_1;
                        j_1 = (_e250 + 1);
                    }
                    loop_init_12 = false;
                    int _e184 = j_1;
                    if ((_e184 < 3)) {
                    } else {
                        break;
                    }
                    {
                        float _e187 = ec;
                        float _e189 = es;
                        float nc = ((_e187 * cl) + (_e189 * sl));
                        float _e192 = es;
                        float _e194 = ec;
                        es = ((_e192 * cl) - (_e194 * sl));
                        ec = nc;
                        vec2 _e199 = _group_0_binding_0_fs.center;
                        float _e201 = ec;
                        float _e204 = es;
                        float _e208 = ec;
                        float _e211 = es;
                        vec2 qe = (_e199 + vec2(((dl.x * _e201) - (dl.y * _e204)), ((dl.y * _e208) + (dl.x * _e211))));
                        uint _e216 = k_11;
                        float _e220 = _group_0_binding_0_fs.quality.w;
                        vec4 _e223 = art(qe, _e216, (_e220 + 0.8));
                        uint _e224 = k_11;
                        vec4 _e225 = built(_e223, qe, _e224);
                        float _e226 = w;
                        vec4 e_1 = (_e225 * _e226);
                        vec4 _e228 = ink_1;
                        float _e232 = ink_1.w;
                        float _e238 = ink_1.w;
                        float _e241 = ink_1.w;
                        ink_1 = vec4((_e228.xyz + (e_1.xyz * (1.0 - _e232))), (_e238 + (e_1.w * (1.0 - _e241))));
                        float _e247 = w;
                        w = (_e247 * 0.55);
                    }
                }
            }
            float _e256 = _group_0_binding_0_fs.live.w;
            float _e260 = _group_0_binding_0_fs.rhythm.x;
            float _e265 = _group_0_binding_0_fs.live2_.z;
            if ((((_e256 + _e260) + _e265) > 0.0)) {
                float _e275 = _group_0_binding_0_fs.gbox.x;
                local_10 = (q_7.x >= _e275);
            } else {
                local_10 = false;
            }
            bool _e278 = local_10;
            if (_e278) {
                float _e285 = _group_0_binding_0_fs.gbox.y;
                local_11 = (q_7.y >= _e285);
            } else {
                local_11 = false;
            }
            bool _e288 = local_11;
            if (_e288) {
                float _e295 = _group_0_binding_0_fs.gbox.z;
                local_12 = (q_7.x <= _e295);
            } else {
                local_12 = false;
            }
            bool _e298 = local_12;
            if (_e298) {
                float _e305 = _group_0_binding_0_fs.gbox.w;
                local_13 = (q_7.y <= _e305);
            } else {
                local_13 = false;
            }
            bool _e308 = local_13;
            if (_e308) {
                vec4 _e309 = ink_1;
                uint _e310 = k_11;
                float _e311 = glyph_hide(q_7, _e310);
                ink_1 = (_e309 * (1.0 - _e311));
                float _e318 = _group_0_binding_0_fs.rhythm.x;
                if ((_e318 > 0.0)) {
                    uint _e321 = k_11;
                    Swapped _e322 = swapped_at(q_7, _e321);
                    vec4 _e323 = ink_1;
                    ink_1 = ((_e323 * (1.0 - _e322.hide)) + _e322.ink);
                }
                float _e333 = _group_0_binding_0_fs.live2_.z;
                if ((_e333 > 0.0)) {
                    vec4 _e336 = ink_1;
                    vec4 _e338 = ink_1;
                    uint _e342 = k_11;
                    float _e343 = marks_at(q_7, _e342);
                    float _e347 = ink_1.w;
                    ink_1 = vec4((_e336.xyz + ((_e338.xyz * MARK) * _e343)), _e347);
                }
            }
            vec4 _e349 = ink_1;
            ink_1 = (_e349 * _e72.form.y);
            if ((_e72.form.z > 0.0)) {
                uint _e357 = k_11;
                vec2 _e360 = burn(q_7, _e357, _e72.form.z);
                float a_5 = ink_1.w;
                vec4 _e363 = ink_1;
                ink_1 = (_e363 * _e360.x);
                vec4 _e366 = ink_1;
                float _e376 = ink_1.w;
                ink_1 = vec4((_e366.xyz + (EMBER * ((_e360.y * a_5) * 2.5))), _e376);
            }
            if ((f_2.tint.w > 0.002)) {
                vec4 _e382 = ink_1;
                float lum_1 = dot(_e382.xyz, LUMA);
                vec3 c_2 = (f_2.tint.xyz / vec3(f_2.tint.w));
                vec4 _e392 = ink_1;
                float _e405 = ink_1.w;
                ink_1 = vec4(mix(_e392.xyz, ((c_2 * lum_1) * 1.8), (min(f_2.tint.w, 1.0) * 0.85)), _e405);
            }
            vec4 _e407 = ink_1;
            vec4 _e410 = ink_1;
            float _e421 = ink_1.w;
            ink_1 = vec4((_e407.xyz + (SHEEN * ((dot(_e410.xyz, LUMA) * f_2.sheen) * 1.6))), _e421);
            vec4 _e423 = ink_1;
            uint _e425 = k_11;
            float _e426 = breath(_e425);
            float _e429 = ink_1.w;
            ink_1 = vec4((_e423.xyz * _e426), _e429);
            vec4 _e431 = ink_1;
            vec4 _e433 = ink_1;
            float _e447 = ink_1.w;
            ink_1 = vec4((_e431.xyz + ((_e433.xyz * HOVER) * ((f_2.near * 0.9) + (f_2.ripple * 1.4)))), _e447);
            float _e452 = _group_0_binding_0_fs.look.y;
            if ((_e452 > 0.0)) {
                vec4 _e455 = ink_1;
                float lum_2 = dot(_e455.xyz, LUMA);
                vec4 _e459 = ink_1;
                uint _e461 = k_11;
                vec3 _e462 = layer_tint(_e461);
                float _e469 = _group_0_binding_0_fs.look.y;
                float _e472 = ink_1.w;
                ink_1 = vec4(mix(_e459.xyz, ((_e462 * lum_2) * 1.35), _e469), _e472);
            }
            float b_2 = _e72.motion.y;
            vec4 _e476 = ink_1;
            vec4 _e478 = ink_1;
            vec4 _e484 = ink_1;
            float _e493 = ink_1.w;
            ink_1 = vec4(((_e476.xyz + (_e478.xyz * (b_2 * 1.3))) + ((_e484.xyz * SPARK) * (blue * 2.0))), _e493);
            vec4 _e495 = ink_1;
            vec3 _e497 = col_1;
            float _e499 = ink_1.w;
            col_1 = (_e495.xyz + (_e497 * (1.0 - _e499)));
            float amt = (((b_2 + red) + blue) + (edge_2 * 0.6));
            if ((amt > 0.002)) {
                vec3 tint_1 = (((((GLOW * b_2) + (BLOOD * red)) + (SPARK * blue)) + (BLOOD_HOT * (edge_2 * 0.6))) / vec3(amt));
                vec3 _e526 = glow;
                uint _e527 = k_11;
                vec3 _e528 = spill(q_7, _e527, amt, tint_1);
                glow = (_e526 + (_e528 * _e72.form.y));
            }
        }
    }
    vec3 _e536 = col_1;
    col_1 = (_e536 * (1.0 - (min(trough_1, 1.0) * 0.72)));
    if ((red > 0.002)) {
        vec3 _e546 = col_1;
        float lum_3 = dot(_e546, LUMA);
        vec3 _e549 = col_1;
        col_1 = mix(_e549, ((BLOOD * lum_3) * 1.5), min(red, 1.0));
    }
    vec3 _e557 = col_1;
    vec3 _e558 = glow;
    col_1 = (((_e557 + _e558) + (BLOOD * (red * 0.05))) + (SPARK * (blue * 0.12)));
    vec3 _e570 = col_1;
    vec3 _e571 = col_1;
    col_1 = ((_e570 + ((_e571 * BLOOD_HOT) * (edge_2 * 0.85))) + (BLOOD_HOT * (edge_2 * 0.12)));
    vec3 _e583 = col_1;
    vec3 _e584 = col_1;
    float _e588 = _group_0_binding_0_fs.look.x;
    float _e597 = _group_0_binding_0_fs.look.x;
    col_1 = ((_e583 + (_e584 * (_e588 * 0.9))) + (BLOOD_HOT * (_e597 * 0.05)));
    float _e608 = _group_0_binding_0_fs.live.w;
    if ((_e608 > 0.0)) {
        float _e617 = _group_0_binding_0_fs.gbox_p.x;
        local_14 = (p_15.x >= _e617);
    } else {
        local_14 = false;
    }
    bool _e620 = local_14;
    if (_e620) {
        float _e627 = _group_0_binding_0_fs.gbox_p.y;
        local_15 = (p_15.y >= _e627);
    } else {
        local_15 = false;
    }
    bool _e630 = local_15;
    if (_e630) {
        float _e637 = _group_0_binding_0_fs.gbox_p.z;
        local_16 = (p_15.x <= _e637);
    } else {
        local_16 = false;
    }
    bool _e640 = local_16;
    if (_e640) {
        float _e647 = _group_0_binding_0_fs.gbox_p.w;
        local_17 = (p_15.y <= _e647);
    } else {
        local_17 = false;
    }
    bool _e650 = local_17;
    if (_e650) {
        vec3 _e651 = glyph_ghost(p_15);
        risen = _e651;
    }
    vec3 answer = (((THREAD * f_2.threads) + (MARK * f_2.halo)) + (HOVER * ((f_2.ripple * 0.08) + (f_2.near * 0.02))));
    vec3 _e669 = col_1;
    vec3 _e674 = risen;
    return (((_e669 + (SPARK_CORE * arc.x)) + _e674) + answer);
}

vec3 encode_srgb(vec3 c) {
    vec3 x_1 = clamp(c, vec3(0.0), vec3(1.0));
    vec3 lo_2 = (x_1 * 12.92);
    vec3 hi_2 = ((1.055 * pow(x_1, vec3(0.41666666))) - vec3(0.055));
    return mix(hi_2, lo_2, lessThanEqual(x_1, vec3(0.0031308)));
}

void main() {
    vec4 frag = gl_FragCoord;
    vec4 pos = vec4(0.0);
    vec3 acc = vec3(0.0);
    Fields f = Fields(vec4(0.0), vec2(0.0), 0.0, vec4(0.0), 0.0, 0.0, 0.0, 0.0, 0.0);
    int j = 0;
    int i_1 = 0;
    vec3 col = vec3(0.0);
    pos = frag;
    float _e5 = _group_0_binding_0_fs.host.x;
    if ((_e5 > 0.0)) {
        float _e12 = _group_0_binding_0_fs.host.x;
        float _e14 = pos.y;
        pos.y = (_e12 - _e14);
    }
    float _e19 = _group_0_binding_0_fs.quality.x;
    int ss = int(_e19);
    vec4 _e21 = pos;
    vec2 base = floor(_e21.xy);
    vec4 _e32 = _group_0_binding_0_fs.fit;
    float _e38 = _group_0_binding_0_fs.fit.x;
    vec2 centre = (((base + vec2(0.5)) - _e32.yz) / vec2(_e38));
    vec2 _e43 = _group_0_binding_0_fs.center;
    vec2 cd = (centre - _e43);
    float _e49 = _group_0_binding_0_fs.quality.y;
    float reach_2 = (_e49 + LAYER_REACH);
    if ((dot(cd, cd) < (reach_2 * reach_2))) {
        Fields _e55 = fields_at(centre);
        f = _e55;
    }
    bool loop_init_13 = true;
    while(true) {
        if (!loop_init_13) {
            int _e94 = j;
            j = (_e94 + 1);
        }
        loop_init_13 = false;
        int _e58 = j;
        if ((_e58 < ss)) {
        } else {
            break;
        }
        {
            i_1 = 0;
            bool loop_init_14 = true;
            while(true) {
                if (!loop_init_14) {
                    int _e91 = i_1;
                    i_1 = (_e91 + 1);
                }
                loop_init_14 = false;
                int _e62 = i_1;
                if ((_e62 < ss)) {
                } else {
                    break;
                }
                {
                    int _e64 = i_1;
                    int _e66 = j;
                    vec2 off_1 = ((vec2(float(_e64), float(_e66)) + vec2(0.5)) / vec2(float(ss)));
                    vec4 _e78 = _group_0_binding_0_fs.fit;
                    float _e84 = _group_0_binding_0_fs.fit.x;
                    vec2 p_18 = (((base + off_1) - _e78.yz) / vec2(_e84));
                    vec3 _e87 = acc;
                    Fields _e88 = f;
                    vec3 _e89 = shade(p_18, _e88);
                    acc = (_e87 + _e89);
                }
            }
        }
    }
    vec3 _e97 = acc;
    col = (_e97 / vec3(float((ss * ss))));
    float _e106 = _group_0_binding_0_fs.look.z;
    if ((_e106 > 0.0)) {
        float _e114 = _group_0_binding_0_fs.quality.y;
        float _e118 = _group_0_binding_0_fs.quality.y;
        float fall = exp(((-(dot(cd, cd)) / (_e114 * _e118)) * 0.6));
        vec3 _e124 = col;
        float _e129 = _group_0_binding_0_fs.look.z;
        col = (_e124 + (BLAST * (_e129 * (0.3 + (1.5 * fall)))));
    }
    float _e140 = _group_0_binding_0_fs.host.y;
    if ((_e140 > 0.0)) {
        vec3 _e143 = col;
        vec3 _e144 = encode_srgb(_e143);
        col = _e144;
    }
    vec3 _e145 = col;
    _fs2p_location0 = vec4(_e145, 1.0);
    return;
}

