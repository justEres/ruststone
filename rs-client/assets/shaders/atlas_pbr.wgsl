#import bevy_pbr::{
    pbr_types,
    pbr_functions,
    pbr_functions::alpha_discard,
    pbr_fragment::pbr_input_from_standard_material,
    decal::clustered::apply_decal_base_color,
}

#ifdef PREPASS_PIPELINE
#import bevy_pbr::{
    prepass_io::{VertexOutput, FragmentOutput},
    pbr_deferred_functions::deferred_output,
}
#else
#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
    pbr_types::STANDARD_MATERIAL_FLAGS_UNLIT_BIT,
}
#endif

#ifdef MESHLET_MESH_MATERIAL_PASS
#import bevy_pbr::meshlet_visibility_buffer_resolve::resolve_vertex_output
#endif

#ifdef OIT_ENABLED
#import bevy_core_pipeline::oit::oit_draw
#endif // OIT_ENABLED

#ifdef FORWARD_DECAL
#import bevy_pbr::decal::forward::get_forward_decal_info
#endif

const ATLAS_COLUMNS: f32 = 64.0;
const ATLAS_ROWS: f32 = 64.0;
const ATLAS_UV_INSET: f32 = 0.02;
const ATLAS_UV_PACK_SCALE: f32 = 1024.0;

// Rendering architecture notes:
// - This shader is shared by opaque/cutout/water materials.
// - `quality_and_water.w` selects pass type:
//   0 = opaque, 1 = transparent (water), 2 = cutout.
// - Cutout/opaque transparency is controlled with explicit `discard`, not mask state.
//   This keeps behavior stable across runtime quality and pipeline switches.

@group(2) @binding(100) var atlas_texture: texture_2d<f32>;
@group(2) @binding(101) var atlas_sampler: sampler;
@group(2) @binding(103) var reflection_texture: texture_2d<f32>;
@group(2) @binding(104) var reflection_sampler: sampler;

struct AtlasLightingUniform {
    sun_dir_and_strength: vec4<f32>,
    ambient_and_fog: vec4<f32>,
    quality_and_water: vec4<f32>,
    color_grading: vec4<f32>,
    water_effects: vec4<f32>,
    water_controls: vec4<f32>,
    water_extra: vec4<f32>,
    debug_flags: vec4<f32>,
    reflection_view_proj: mat4x4<f32>,
}

@group(2) @binding(102) var<uniform> lighting_uniform: AtlasLightingUniform;

fn atlas_uv_from_repeating(local_uv: vec2<f32>, tile_origin: vec2<f32>) -> vec2<f32> {
    let tile_size = vec2<f32>(1.0 / ATLAS_COLUMNS, 1.0 / ATLAS_ROWS);
    let inset = tile_size * ATLAS_UV_INSET;
    return tile_origin + inset + fract(local_uv) * (tile_size - inset * 2.0);
}

fn atlas_texel_from_repeating(local_uv: vec2<f32>, tile_origin: vec2<f32>) -> vec4<f32> {
    let uv = atlas_uv_from_repeating(local_uv, tile_origin);
    let tex_size_u = textureDimensions(atlas_texture, 0);
    let tex_size = vec2<f32>(f32(tex_size_u.x), f32(tex_size_u.y));
    let max_xy = vec2<f32>(tex_size.x - 1.0, tex_size.y - 1.0);
    let texel_f = clamp(floor(uv * tex_size), vec2<f32>(0.0, 0.0), max_xy);
    let texel = vec2<i32>(i32(texel_f.x), i32(texel_f.y));
    return textureLoad(atlas_texture, texel, 0);
}

// --- Math helpers -------------------------------------------------------------

fn safe_normalize(v: vec3<f32>, fallback: vec3<f32>) -> vec3<f32> {
    let len2 = dot(v, v);
    if len2 > 0.000001 {
        return v * inverseSqrt(len2);
    }
    return fallback;
}

fn apply_voxel_lighting(
    base: vec4<f32>,
    normal: vec3<f32>,
    view_z: f32,
    view_dir: vec3<f32>,
    water_scene_reflection: vec3<f32>,
    water_scene_reflection_valid: f32,
) -> vec4<f32> {
    let quality_mode = lighting_uniform.quality_and_water.x;
    let sun_dir = safe_normalize(lighting_uniform.sun_dir_and_strength.xyz, vec3(0.0, 1.0, 0.0));
    let sun_strength = lighting_uniform.sun_dir_and_strength.w;
    let ambient_strength = lighting_uniform.ambient_and_fog.x;

    let ndotl = max(dot(normal, sun_dir), 0.0);
    var shade = ambient_strength;
    if quality_mode >= 1.0 {
        shade += ndotl * sun_strength;
    }

    if quality_mode >= 2.0 {
        // Cheap hemispherical lift to avoid pure black undersides.
        let hemi = normal.y * 0.5 + 0.5;
        shade *= mix(0.84, 1.12, hemi);
    }

    if quality_mode >= 3.0 {
        // Slightly soften contrast in the highest preset.
        shade = pow(max(shade, 0.0), 0.92);
    }

    var rgb = base.rgb * shade;

    let pass_mode = lighting_uniform.quality_and_water.w;
    let transparent_pass = pass_mode > 0.5 && pass_mode < 1.5;
    if transparent_pass && quality_mode >= 2.0 {
        let absorption = lighting_uniform.quality_and_water.y;
        let fresnel_boost = lighting_uniform.quality_and_water.z;
        let fresnel = pow(1.0 - max(dot(normal, view_dir), 0.0), 5.0);
        let water_tint = vec3(0.50, 0.66, 0.93);
        rgb = mix(rgb * (1.0 - absorption), water_tint, fresnel * (0.24 + fresnel_boost));
        if lighting_uniform.water_effects.x > 0.5 {
            let user_strength = clamp(lighting_uniform.water_controls.x, 0.0, 3.0);
            let near_boost = clamp(lighting_uniform.water_controls.z, 0.0, 1.0);
            let blue_tint_strength = clamp(lighting_uniform.water_controls.w, 0.0, 1.0);
            let sun_dir_reflect = safe_normalize(-sun_dir, vec3(0.0, 1.0, 0.0));
            let reflected = reflect(-view_dir, normal);
            let sky_blend = clamp(reflected.y * 0.5 + 0.5, 0.0, 1.0);
            let sky = mix(vec3(0.64, 0.73, 0.86), vec3(0.39, 0.56, 0.88), sky_blend);
            let terrain_enabled = lighting_uniform.water_effects.x > 1.5;
            let terrain = vec3(0.32, 0.39, 0.27);
            let terrain_blend_val =
                clamp(pow(clamp(-reflected.y, 0.0, 1.0), 0.72) * 1.25, 0.0, 1.0);
            let terrain_blend = select(0.0, terrain_blend_val, terrain_enabled);
            let env_reflection_base = mix(sky, terrain, terrain_blend);
            let sky_fill = clamp(lighting_uniform.debug_flags.y, 0.0, 1.0);
            let env_reflection_fallback = mix(env_reflection_base, sky, sky_fill);
            let env_reflection = mix(
                env_reflection_fallback,
                water_scene_reflection,
                water_scene_reflection_valid * select(0.0, 1.0, terrain_enabled),
            );
            let sun_glint = pow(max(dot(reflected, sun_dir_reflect), 0.0), 96.0) * 0.65;
            let refl_strength = select(0.82, 0.94, terrain_enabled);
            let boost = 0.25 + user_strength * 1.45;
            let min_mirror = clamp((user_strength - 1.0) * 0.40 + near_boost * 0.65, 0.0, 0.85);
            let reflected_color = env_reflection + vec3(sun_glint);
            let tinted_reflection = mix(
                reflected_color,
                reflected_color * vec3(0.78, 0.90, 1.10),
                blue_tint_strength,
            );
            let refl_mix = clamp(max(fresnel * refl_strength * boost, min_mirror), 0.0, 0.995);
            rgb = mix(rgb, tinted_reflection, refl_mix);
        }
    }

    if quality_mode >= 2.0 {
        let fog_density = lighting_uniform.ambient_and_fog.y;
        let fog_start = lighting_uniform.ambient_and_fog.z;
        let fog_end = max(lighting_uniform.ambient_and_fog.w, fog_start + 1.0);
        let fog_color = vec3(0.66, 0.73, 0.87);
        let dist = max(view_z, 0.0);
        let fog_range = clamp((dist - fog_start) / (fog_end - fog_start), 0.0, 1.0);
        let fog_exp = 1.0 - exp(-dist * fog_density);
        let fog = max(fog_range, fog_exp);
        rgb = mix(rgb, fog_color, fog);
    }

    return vec4(rgb, base.a);
}

fn apply_fancy_post_lighting(
    base: vec4<f32>,
    normal: vec3<f32>,
    view_z: f32,
    view_dir: vec3<f32>,
    water_scene_reflection: vec3<f32>,
    water_scene_reflection_valid: f32,
) -> vec4<f32> {
    let quality_mode = lighting_uniform.quality_and_water.x;
    var rgb = base.rgb;

    if quality_mode >= 3.0 {
        // Slightly soften hard contrasts after PBR shading in highest quality.
        rgb = pow(max(rgb, vec3(0.0)), vec3(0.96));
    }

    let pass_mode = lighting_uniform.quality_and_water.w;
    let transparent_pass = pass_mode > 0.5 && pass_mode < 1.5;
    if transparent_pass && quality_mode >= 2.0 {
        let absorption = lighting_uniform.quality_and_water.y;
        let fresnel_boost = lighting_uniform.quality_and_water.z;
        let fresnel = pow(1.0 - max(dot(normal, view_dir), 0.0), 5.0);
        let water_tint = vec3(0.50, 0.66, 0.93);
        rgb = mix(rgb * (1.0 - absorption), water_tint, fresnel * (0.22 + fresnel_boost));
        if lighting_uniform.water_effects.x > 0.5 {
            let user_strength = clamp(lighting_uniform.water_controls.x, 0.0, 3.0);
            let near_boost = clamp(lighting_uniform.water_controls.z, 0.0, 1.0);
            let blue_tint_strength = clamp(lighting_uniform.water_controls.w, 0.0, 1.0);
            let sun_dir = safe_normalize(lighting_uniform.sun_dir_and_strength.xyz, vec3(0.0, 1.0, 0.0));
            let reflected = reflect(-view_dir, normal);
            let sky_blend = clamp(reflected.y * 0.5 + 0.5, 0.0, 1.0);
            let sky = mix(vec3(0.64, 0.73, 0.86), vec3(0.39, 0.56, 0.88), sky_blend);
            let terrain_enabled = lighting_uniform.water_effects.x > 1.5;
            let terrain = vec3(0.32, 0.39, 0.27);
            let terrain_blend_val =
                clamp(pow(clamp(-reflected.y, 0.0, 1.0), 0.72) * 1.25, 0.0, 1.0);
            let terrain_blend = select(0.0, terrain_blend_val, terrain_enabled);
            let env_reflection_base = mix(sky, terrain, terrain_blend);
            let sky_fill = clamp(lighting_uniform.debug_flags.y, 0.0, 1.0);
            let env_reflection_fallback = mix(env_reflection_base, sky, sky_fill);
            let env_reflection = mix(
                env_reflection_fallback,
                water_scene_reflection,
                water_scene_reflection_valid * select(0.0, 1.0, terrain_enabled),
            );
            let sun_glint = pow(max(dot(reflected, safe_normalize(-sun_dir, vec3(0.0, 1.0, 0.0))), 0.0), 96.0) * 0.65;
            let refl_strength = select(0.82, 0.94, terrain_enabled);
            let boost = 0.25 + user_strength * 1.45;
            let min_mirror = clamp((user_strength - 1.0) * 0.40 + near_boost * 0.65, 0.0, 0.85);
            let reflected_color = env_reflection + vec3(sun_glint);
            let tinted_reflection = mix(
                reflected_color,
                reflected_color * vec3(0.78, 0.90, 1.10),
                blue_tint_strength,
            );
            let refl_mix = clamp(max(fresnel * refl_strength * boost, min_mirror), 0.0, 0.995);
            rgb = mix(rgb, tinted_reflection, refl_mix);
        }
    }

    if quality_mode >= 2.0 {
        let fog_density = lighting_uniform.ambient_and_fog.y;
        let fog_start = lighting_uniform.ambient_and_fog.z;
        let fog_end = max(lighting_uniform.ambient_and_fog.w, fog_start + 1.0);
        let fog_color = vec3(0.66, 0.73, 0.87);
        let dist = max(view_z, 0.0);
        let fog_range = clamp((dist - fog_start) / (fog_end - fog_start), 0.0, 1.0);
        let fog_exp = 1.0 - exp(-dist * fog_density);
        let fog = max(fog_range, fog_exp);
        rgb = mix(rgb, fog_color, fog);
    }

    return vec4(rgb, base.a);
}

fn apply_color_grading(rgb_in: vec3<f32>) -> vec3<f32> {
    let saturation = max(lighting_uniform.color_grading.x, 0.0);
    let contrast = max(lighting_uniform.color_grading.y, 0.0);
    let brightness = lighting_uniform.color_grading.z;
    let gamma = max(lighting_uniform.color_grading.w, 0.001);

    let luma = dot(rgb_in, vec3(0.2126, 0.7152, 0.0722));
    var rgb = mix(vec3(luma), rgb_in, saturation);
    rgb = (rgb - vec3(0.5)) * contrast + vec3(0.5);
    rgb = rgb + vec3(brightness);
    rgb = pow(max(rgb, vec3(0.0)), vec3(1.0 / gamma));
    return clamp(rgb, vec3(0.0), vec3(1.0));
}

@fragment
fn fragment(
#ifdef MESHLET_MESH_MATERIAL_PASS
    @builtin(position) frag_coord: vec4<f32>,
#else
    vertex_output: VertexOutput,
    @builtin(front_facing) is_front: bool,
#endif
) -> FragmentOutput {
#ifdef MESHLET_MESH_MATERIAL_PASS
    let vertex_output = resolve_vertex_output(frag_coord);
    let is_front = true;
#endif

    var in = vertex_output;

    // If we're in the crossfade section of a visibility range, conditionally
    // discard the fragment according to the visibility pattern.
#ifdef VISIBILITY_RANGE_DITHER
    pbr_functions::visibility_range_dither(in.position, in.visibility_range_dither);
#endif

#ifdef FORWARD_DECAL
    let forward_decal_info = get_forward_decal_info(in);
    in.world_position = forward_decal_info.world_position;
    in.uv = forward_decal_info.uv;
#endif

    // Stage 1: Base material and atlas sampling.
    // Generate a PbrInput struct from the StandardMaterial bindings.
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    let vertex_tint_rgb = pbr_input.material.base_color.rgb;
    let pass_mode = lighting_uniform.quality_and_water.w;
    let pass_mode_valid = pass_mode == pass_mode;
    let transparent_pass = pass_mode_valid && pass_mode > 0.5 && pass_mode < 1.5;
    let cutout_pass = pass_mode_valid && pass_mode > 1.5;
    let cutout_debug_mode = i32(round(lighting_uniform.debug_flags.x));
    let cutout_blend_enabled = lighting_uniform.debug_flags.z > 0.5;

#ifdef VERTEX_UVS_A
    let packed_uv = in.uv;
    let tile_cell = floor(packed_uv / vec2<f32>(ATLAS_UV_PACK_SCALE, ATLAS_UV_PACK_SCALE));
    var uv_local = packed_uv - tile_cell * vec2<f32>(ATLAS_UV_PACK_SCALE, ATLAS_UV_PACK_SCALE);
    var tile_origin = tile_cell / vec2<f32>(ATLAS_COLUMNS, ATLAS_ROWS);
    var water_wave = vec2<f32>(0.0, 0.0);
    if transparent_pass {
        let t = lighting_uniform.water_effects.w * lighting_uniform.water_effects.z;
        let amp = lighting_uniform.water_effects.y;
        let detail_amp = lighting_uniform.water_extra.x;
        let detail_scale = lighting_uniform.water_extra.y;
        let detail_speed = lighting_uniform.water_extra.z;
        let wp = in.world_position.xyz;
        let base_wave = vec2<f32>(
            sin((wp.x + wp.z) * 0.42 + t * 2.3)
                + 0.55 * sin(wp.x * 0.95 - t * 1.7),
            cos((wp.x - wp.z) * 0.31 - t * 1.6)
                + 0.5 * cos(wp.z * 1.05 + t * 1.4),
        );
        let detail_wave = vec2<f32>(
            sin((wp.x * detail_scale + wp.z * detail_scale * 1.37) * 0.83 + t * (2.7 + detail_speed)),
            cos((wp.x * detail_scale * 1.21 - wp.z * detail_scale) * 0.71 - t * (2.2 + detail_speed)),
        );
        let micro_wave = vec2<f32>(
            sin((wp.x * detail_scale * 2.5 + wp.z * detail_scale * 3.2) * 1.31 + t * (3.7 + detail_speed * 1.7)),
            cos((wp.z * detail_scale * 2.9 - wp.x * detail_scale * 2.1) * 1.19 - t * (3.2 + detail_speed * 1.5)),
        );
        water_wave = base_wave + detail_wave * detail_amp + micro_wave * (detail_amp * 0.42);
        let wave_scale = amp * 0.11;
        uv_local += water_wave * wave_scale;
    }
    // Manual texel fetch keeps alpha-cutout stable across quality pipeline switches.
    let atlas_sample = atlas_texel_from_repeating(uv_local, tile_origin);
    let atlas_rgb = atlas_sample.rgb;
    pbr_input.material.base_color *= atlas_sample;
    let atlas_alpha = atlas_sample.a;
#else
    let atlas_rgb = pbr_input.material.base_color.rgb;
    let atlas_alpha = pbr_input.material.base_color.a;
#endif

    if transparent_pass {
        // Keep water/lava blend, but still punch fully transparent texels.
        if !(atlas_alpha > 0.001) {
            discard;
        }
    } else {
        // Hard cutout for foliage/glass/cross meshes and opaque block passes.
        // Use a NaN-safe comparison: invalid alpha should be discarded too.
        if !(atlas_alpha >= 0.5) {
            discard;
        }
    }

    // Stage 2: Pass-specific alpha handling.
    // We do our own alpha cutout above. Keep cutout passes fully opaque after discard
    // to avoid backend/material-mode differences on alpha-mask handling.
    if transparent_pass || (cutout_pass && cutout_blend_enabled) {
        pbr_input.material.base_color.a = atlas_alpha;
        // Keep std alpha handling for blended passes (water/cutout blend debug).
        pbr_input.material.base_color =
            alpha_discard(pbr_input.material, pbr_input.material.base_color);
    } else {
        pbr_input.material.base_color.a = 1.0;
    }

    // Clustered decals.
    pbr_input.material.base_color = apply_decal_base_color(
        in.world_position.xyz,
        in.position.xy,
        pbr_input.material.base_color
    );

#ifdef PREPASS_PIPELINE
    // Write the gbuffer, lighting pass id, and optionally normal and motion_vector textures.
    let out = deferred_output(in, pbr_input);
#else
    // Stage 3: Lighting.
    // Hybrid path:
    // - Fast/Standard: cheap custom voxel lighting (no dynamic shadow sampling).
    // - Fancy presets: Bevy PBR lighting + CSM shadows + lightweight post stylization.
    var out: FragmentOutput;
    var water_scene_reflection = vec3<f32>(0.0, 0.0, 0.0);
    var water_scene_reflection_valid = 0.0;
    if transparent_pass && lighting_uniform.water_effects.x > 1.5 {
        // Reflection camera is already mirrored around the water plane.
        // Project the current world position into reflection clip space.
        let clip = lighting_uniform.reflection_view_proj * vec4<f32>(in.world_position.xyz, 1.0);
        if clip.w > 0.0001 {
            let ndc = clip.xy / clip.w;
            let uv_raw = vec2<f32>(ndc.x * 0.5 + 0.5, 1.0 - (ndc.y * 0.5 + 0.5))
                + water_wave * (lighting_uniform.water_effects.y * 0.025);
            let uv = clamp(uv_raw, vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 1.0));
            water_scene_reflection = textureSample(reflection_texture, reflection_sampler, uv).rgb;

            let edge = min(min(uv.x, uv.y), min(1.0 - uv.x, 1.0 - uv.y));
            let edge_fade = clamp(lighting_uniform.water_extra.w, 0.01, 0.5);
            let edge_valid = smoothstep(0.0, edge_fade, edge);

            let outside = max(vec2<f32>(0.0, 0.0), max(-uv_raw, uv_raw - vec2<f32>(1.0, 1.0)));
            let outside_dist = max(outside.x, outside.y);
            let oob_valid = 1.0 - smoothstep(0.0, edge_fade * 1.7, outside_dist);

            water_scene_reflection_valid = edge_valid * oob_valid;
        }
    }
    let base_normal = safe_normalize(pbr_input.N, vec3(0.0, 1.0, 0.0));
    var normal = base_normal;
    if transparent_pass {
        let amp = lighting_uniform.water_effects.y;
        normal = safe_normalize(
            base_normal + vec3<f32>(water_wave.x * amp * 0.28, 0.0, water_wave.y * amp * 0.28),
            base_normal,
        );
    }
    let view_dir = safe_normalize(pbr_input.V, vec3(0.0, 0.0, 1.0));
    let quality_mode = lighting_uniform.quality_and_water.x;
    if cutout_debug_mode == 4 {
        let pass_rgb = select(
            vec3<f32>(0.16, 0.16, 0.16), // opaque-ish
            vec3<f32>(0.2, 0.45, 1.0),   // transparent
            transparent_pass,
        );
        out.color = vec4(
            select(pass_rgb, vec3<f32>(0.2, 1.0, 0.35), cutout_pass), // cutout
            1.0,
        );
    } else if cutout_pass && cutout_debug_mode != 0 {
        // Diagnostic view for cutout pipeline debugging:
        // 1 = raw atlas rgb, 2 = raw vertex tint rgb, 3 = atlas alpha.
        var debug_rgb = vec3<f32>(atlas_alpha, atlas_alpha, atlas_alpha);
        if cutout_debug_mode == 1 {
            debug_rgb = atlas_rgb;
        } else if cutout_debug_mode == 2 {
            debug_rgb = vertex_tint_rgb;
        }
        out.color = vec4(debug_rgb, 1.0);
    } else if quality_mode >= 2.0 && (pbr_input.material.flags & STANDARD_MATERIAL_FLAGS_UNLIT_BIT) == 0u {
        out.color = apply_pbr_lighting(pbr_input);
        out.color = apply_fancy_post_lighting(
            out.color,
            normal,
            abs(in.position.w),
            view_dir,
            water_scene_reflection,
            water_scene_reflection_valid,
        );
    } else {
        out.color = apply_voxel_lighting(
            pbr_input.material.base_color,
            normal,
            abs(in.position.w),
            view_dir,
            water_scene_reflection,
            water_scene_reflection_valid,
        );
    }
    out.color = vec4(apply_color_grading(out.color.rgb), out.color.a);

    // Apply in-shader post processing (fog, alpha-premultiply, and optional tonemapping/debanding).
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
#endif

#ifdef OIT_ENABLED
    let alpha_mode = pbr_input.material.flags & pbr_types::STANDARD_MATERIAL_FLAGS_ALPHA_MODE_RESERVED_BITS;
    if alpha_mode != pbr_types::STANDARD_MATERIAL_FLAGS_ALPHA_MODE_OPAQUE {
        // The fragments will only be drawn during the oit resolve pass.
        oit_draw(in.position, out.color);
        discard;
    }
#endif // OIT_ENABLED

#ifdef FORWARD_DECAL
    out.color.a = min(forward_decal_info.alpha, out.color.a);
#endif

    return out;
}
