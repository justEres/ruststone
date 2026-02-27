#import bevy_pbr::{
    pbr_types,
    pbr_functions,
    pbr_functions::alpha_discard,
    decal::clustered::apply_decal_base_color,
}
#ifndef PREPASS_PIPELINE
#import bevy_pbr::pbr_fragment::pbr_input_from_standard_material
#endif
#import bevy_pbr::{
    mesh_view_bindings as view_bindings,
    prepass_utils,
    view_transformations::{
        depth_ndc_to_view_z,
        ndc_to_uv,
        position_world_to_ndc,
        position_world_to_view,
    },
}

#ifdef PREPASS_PIPELINE
#import bevy_pbr::{
    prepass_io::{VertexOutput, FragmentOutput},
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

#ifdef TONEMAP_IN_SHADER
#import bevy_core_pipeline::tonemapping::approximate_inverse_tone_mapping
#endif

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
@group(2) @binding(103) var skybox_texture: texture_cube<f32>;
@group(2) @binding(104) var skybox_sampler: sampler;

struct AtlasLightingUniform {
    sun_dir_and_strength: vec4<f32>,
    ambient_and_fog: vec4<f32>,
    quality_and_water: vec4<f32>,
    color_grading: vec4<f32>,
    water_effects: vec4<f32>,
    water_controls: vec4<f32>,
    water_extra: vec4<f32>,
    ssr_params: vec4<f32>,
    debug_flags: vec4<f32>,
    grass_overlay_info: vec4<f32>,
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

fn sample_skybox_reflection(reflect_dir: vec3<f32>) -> vec3<f32> {
    let dir = safe_normalize(reflect_dir, vec3<f32>(0.0, 1.0, 0.0));
    return textureSampleLevel(skybox_texture, skybox_sampler, dir, 0.0).rgb;
}

fn sample_scene_reflection(uv: vec2<f32>) -> vec3<f32> {
    var sampled = textureSampleLevel(
        view_bindings::view_transmission_texture,
        view_bindings::view_transmission_sampler,
        uv,
        0.0,
    );
    sampled = vec4<f32>(sampled.rgb / max(view_bindings::view.exposure, 0.0001), sampled.a);
#ifdef TONEMAP_IN_SHADER
    sampled = approximate_inverse_tone_mapping(sampled, view_bindings::view.color_grading);
#endif
    return sampled.rgb;
}

const WATER_SSR_MAX_STEPS: u32 = 64u;

#ifdef DEPTH_PREPASS
fn water_ssr_raymarch(
    origin_world: vec3<f32>,
    reflect_world: vec3<f32>,
    wave: vec2<f32>,
) -> vec4<f32> {
    let steps = u32(clamp(round(lighting_uniform.ssr_params.x), 4.0, f32(WATER_SSR_MAX_STEPS)));
    let thickness = clamp(lighting_uniform.ssr_params.y, 0.02, 2.0);
    let max_distance = clamp(lighting_uniform.ssr_params.z, 4.0, 400.0);
    let stride = clamp(lighting_uniform.ssr_params.w, 0.2, 8.0);
    let ray_dir = safe_normalize(reflect_world, vec3<f32>(0.0, 1.0, 0.0));

    var prev_uv = vec2<f32>(0.0, 0.0);
    var prev_diff = 0.0;
    var has_prev = false;

    let jitter = fract(sin(dot(origin_world.xz + wave * 0.33, vec2<f32>(12.9898, 78.233))) * 43758.5453);
    let t0 = 0.35 + jitter * stride;
    for (var i = 0u; i < WATER_SSR_MAX_STEPS; i = i + 1u) {
        if i >= steps {
            break;
        }
        let t = t0 + f32(i) * stride;
        if t > max_distance {
            break;
        }

        let sample_world = origin_world + ray_dir * t;
        let sample_ndc = position_world_to_ndc(sample_world);
        if sample_ndc.z <= 0.0001 || sample_ndc.z >= 0.9999 {
            continue;
        }

        let uv = ndc_to_uv(sample_ndc.xy);
        if any(uv < vec2<f32>(0.0, 0.0)) || any(uv > vec2<f32>(1.0, 1.0)) {
            break;
        }

        let frag_xy = uv * view_bindings::view.viewport.zw + view_bindings::view.viewport.xy;
        let scene_ndc_depth = prepass_utils::prepass_depth(vec4<f32>(frag_xy, 0.0, 0.0), 0u);
        if scene_ndc_depth <= 0.0001 {
            continue;
        }

        let scene_view_z = depth_ndc_to_view_z(scene_ndc_depth);
        let ray_view_z = position_world_to_view(sample_world).z;
        let diff = ray_view_z - scene_view_z;

        if has_prev && prev_diff > thickness && diff <= thickness {
            let denom = max(prev_diff - diff, 0.0001);
            let a = clamp((prev_diff - thickness) / denom, 0.0, 1.0);
            let hit_uv = mix(prev_uv, uv, a);
            let hit_color = sample_scene_reflection(hit_uv);
            return vec4<f32>(hit_color, 1.0);
        }

        prev_uv = uv;
        prev_diff = diff;
        has_prev = true;
    }
    return vec4<f32>(0.0, 0.0, 0.0, 0.0);
}
#endif

fn apply_voxel_lighting(
    base: vec4<f32>,
    normal: vec3<f32>,
    view_z: f32,
    view_dir: vec3<f32>,
    is_water_surface: bool,
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
    if transparent_pass && is_water_surface && quality_mode >= 2.0 {
        let absorption = lighting_uniform.quality_and_water.y;
        let fresnel_boost = lighting_uniform.quality_and_water.z;
        let fresnel = pow(1.0 - max(dot(normal, view_dir), 0.0), 5.0);
        let water_tint = vec3(0.50, 0.66, 0.93);
        rgb = mix(rgb * (1.0 - absorption), water_tint, fresnel * (0.24 + fresnel_boost));
        if lighting_uniform.water_effects.x > 0.5 {
            let user_strength = clamp(lighting_uniform.water_controls.x, 0.0, 3.0);
            let blue_tint_strength = clamp(lighting_uniform.water_controls.w, 0.0, 1.0);
            let sun_dir_reflect = safe_normalize(-sun_dir, vec3(0.0, 1.0, 0.0));
            let reflected = reflect(-view_dir, normal);
            let sky = sample_skybox_reflection(reflected);
            let terrain_enabled = lighting_uniform.water_effects.x > 1.5;
            let terrain = vec3(0.32, 0.39, 0.27);
            let terrain_blend_val =
                clamp(pow(clamp(-reflected.y, 0.0, 1.0), 0.72) * 1.25, 0.0, 1.0);
            let terrain_blend = select(0.0, terrain_blend_val, terrain_enabled);
            let env_reflection_base = mix(sky, terrain, terrain_blend);
            let sky_fill = clamp(lighting_uniform.debug_flags.y, 0.0, 1.0);
            let env_reflection_fallback = mix(env_reflection_base, sky, sky_fill);
            let env_reflection = mix(env_reflection_fallback, water_scene_reflection, water_scene_reflection_valid);
            let sun_glint = pow(max(dot(reflected, sun_dir_reflect), 0.0), 96.0) * 0.65;
            let refl_strength = select(0.82, 0.94, terrain_enabled);
            let boost = 0.25 + user_strength * 1.45;
            let min_mirror = clamp((user_strength - 1.0) * 0.40, 0.0, 0.85);
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
    is_water_surface: bool,
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
    if transparent_pass && is_water_surface && quality_mode >= 2.0 {
        let absorption = lighting_uniform.quality_and_water.y;
        let fresnel_boost = lighting_uniform.quality_and_water.z;
        let fresnel = pow(1.0 - max(dot(normal, view_dir), 0.0), 5.0);
        let water_tint = vec3(0.50, 0.66, 0.93);
        rgb = mix(rgb * (1.0 - absorption), water_tint, fresnel * (0.22 + fresnel_boost));
        if lighting_uniform.water_effects.x > 0.5 {
            let user_strength = clamp(lighting_uniform.water_controls.x, 0.0, 3.0);
            let blue_tint_strength = clamp(lighting_uniform.water_controls.w, 0.0, 1.0);
            let sun_dir = safe_normalize(lighting_uniform.sun_dir_and_strength.xyz, vec3(0.0, 1.0, 0.0));
            let reflected = reflect(-view_dir, normal);
            let sky = sample_skybox_reflection(reflected);
            let terrain_enabled = lighting_uniform.water_effects.x > 1.5;
            let terrain = vec3(0.32, 0.39, 0.27);
            let terrain_blend_val =
                clamp(pow(clamp(-reflected.y, 0.0, 1.0), 0.72) * 1.25, 0.0, 1.0);
            let terrain_blend = select(0.0, terrain_blend_val, terrain_enabled);
            let env_reflection_base = mix(sky, terrain, terrain_blend);
            let sky_fill = clamp(lighting_uniform.debug_flags.y, 0.0, 1.0);
            let env_reflection_fallback = mix(env_reflection_base, sky, sky_fill);
            let env_reflection = mix(env_reflection_fallback, water_scene_reflection, water_scene_reflection_valid);
            let sun_glint = pow(max(dot(reflected, safe_normalize(-sun_dir, vec3(0.0, 1.0, 0.0))), 0.0), 96.0) * 0.65;
            let refl_strength = select(0.82, 0.94, terrain_enabled);
            let boost = 0.25 + user_strength * 1.45;
            let min_mirror = clamp((user_strength - 1.0) * 0.40, 0.0, 0.85);
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
#ifdef PREPASS_PIPELINE
#ifdef PREPASS_FRAGMENT
) -> FragmentOutput {
#else
) {
#endif
#else
) -> FragmentOutput {
#endif
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
    let pass_mode = lighting_uniform.quality_and_water.w;
    let pass_mode_valid = pass_mode == pass_mode;
    let transparent_pass = pass_mode_valid && pass_mode > 0.5 && pass_mode < 1.5;
    let cutout_pass = pass_mode_valid && pass_mode > 1.5;
    var is_water_surface = false;
#ifndef PREPASS_PIPELINE
    // Generate a PbrInput struct from the StandardMaterial bindings.
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    let vertex_tint_rgb = pbr_input.material.base_color.rgb;
    let vertex_tint_alpha = pbr_input.material.base_color.a;
    is_water_surface = transparent_pass && vertex_tint_alpha < 0.75;
    let shader_debug_view = i32(round(lighting_uniform.debug_flags.x));
    let fixed_debug_state = lighting_uniform.debug_flags.w > 0.5;
#endif

    let packed_uv = in.uv;
    let tile_cell = floor(packed_uv / vec2<f32>(ATLAS_UV_PACK_SCALE, ATLAS_UV_PACK_SCALE));
    var uv_local = packed_uv - tile_cell * vec2<f32>(ATLAS_UV_PACK_SCALE, ATLAS_UV_PACK_SCALE);
    var tile_origin = tile_cell / vec2<f32>(ATLAS_COLUMNS, ATLAS_ROWS);
    var water_wave = vec2<f32>(0.0, 0.0);
    if is_water_surface {
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
    let atlas_alpha = atlas_sample.a;
    let grass_base_origin = lighting_uniform.grass_overlay_info.xy;
    let grass_overlay_origin = lighting_uniform.grass_overlay_info.zw;
    let grass_info_valid =
        grass_base_origin.x == grass_base_origin.x
        && grass_base_origin.y == grass_base_origin.y
        && grass_overlay_origin.x == grass_overlay_origin.x
        && grass_overlay_origin.y == grass_overlay_origin.y;
    let origin_eps = vec2<f32>(0.5 / ATLAS_COLUMNS, 0.5 / ATLAS_ROWS);
    let is_grass_side =
        grass_info_valid && all(abs(tile_origin - grass_base_origin) <= origin_eps);
#ifndef PREPASS_PIPELINE
    if is_grass_side {
        let overlay_sample = atlas_texel_from_repeating(uv_local, grass_overlay_origin);
        let overlay_mask = overlay_sample.a;
        let shade = clamp(pbr_input.material.base_color.a, 0.0, 1.0);
        let tinted = mix(vec3<f32>(1.0, 1.0, 1.0), clamp(vertex_tint_rgb, vec3<f32>(0.0), vec3<f32>(1.0)), overlay_mask);
        pbr_input.material.base_color = vec4(atlas_rgb * tinted * shade, atlas_alpha);
    } else {
        pbr_input.material.base_color *= atlas_sample;
    }
#endif

    if transparent_pass {
        // Keep water/lava blend, but still punch fully transparent texels.
        if !(atlas_alpha > 0.001) {
            discard;
        }
    } else {
        // For all non-transparent passes force binary cutout.
        // This keeps foliage/cutout stable even if runtime pass flags or material state
        // are temporarily out of sync after quality switches.
        if !(atlas_alpha >= 0.5) {
            discard;
        }
    }

#ifndef PREPASS_PIPELINE
    if shader_debug_view != 0 {
        var debug_rgb = vec3<f32>(1.0, 0.0, 1.0);
        if shader_debug_view == 1 {
            debug_rgb = select(
                vec3<f32>(0.18, 0.18, 0.18), // opaque
                vec3<f32>(0.20, 0.45, 1.0),  // transparent
                transparent_pass,
            );
            debug_rgb = select(debug_rgb, vec3<f32>(0.2, 1.0, 0.35), cutout_pass); // cutout
        } else if shader_debug_view == 2 {
            debug_rgb = atlas_rgb;
        } else if shader_debug_view == 3 {
            debug_rgb = vec3<f32>(atlas_alpha, atlas_alpha, atlas_alpha);
        } else if shader_debug_view == 4 {
            debug_rgb = vertex_tint_rgb;
        } else if shader_debug_view == 5 {
            let depth_norm = clamp(abs(in.position.w) / max(lighting_uniform.ambient_and_fog.w, 1.0), 0.0, 1.0);
            debug_rgb = vec3<f32>(depth_norm, depth_norm, depth_norm);
        } else if shader_debug_view == 6 {
            // Pass flag diagnostics:
            // R = cutout, G = transparent, B = opaque fallback
            let opaque_flag = select(0.0, 1.0, !(transparent_pass || cutout_pass));
            debug_rgb = vec3<f32>(
                select(0.0, 1.0, cutout_pass),
                select(0.0, 1.0, transparent_pass),
                opaque_flag,
            );
        } else if shader_debug_view == 7 {
            // Alpha diagnostics:
            // R = strict cutout keep bit (alpha >= 0.5),
            // G = raw sampled alpha,
            // B = pass_mode (normalized).
            let keep = select(0.0, 1.0, atlas_alpha >= 0.5);
            let pass_n = clamp(pass_mode * 0.5, 0.0, 1.0);
            debug_rgb = vec3<f32>(keep, atlas_alpha, pass_n);
        } else if shader_debug_view == 8 {
            // Cutout lighting path diagnostics:
            // Green = cutout + lit (PBR-eligible),
            // Red = cutout + unlit flag set,
            // Blue = non-cutout pass.
            let cutout_lit =
                cutout_pass
                && (pbr_input.material.flags & STANDARD_MATERIAL_FLAGS_UNLIT_BIT) == 0u;
            debug_rgb = vec3<f32>(0.12, 0.32, 1.0);
            if cutout_pass {
                debug_rgb = select(
                    vec3<f32>(1.0, 0.15, 0.15),
                    vec3<f32>(0.15, 1.0, 0.25),
                    cutout_lit,
                );
            }
        }
        var debug_out: FragmentOutput;
        debug_out.color = vec4(debug_rgb, 1.0);
        return debug_out;
    }
#endif

#ifdef PREPASS_PIPELINE
    // Depth/shadow prepass path: alpha discard above controls coverage.
#ifdef PREPASS_FRAGMENT
    var out: FragmentOutput;
#ifdef UNCLIPPED_DEPTH_ORTHO_EMULATION
    out.frag_depth = in.unclipped_depth;
#endif
#endif
#else
    // Stage 2: Pass-specific alpha handling.
    // We do our own alpha cutout above. Keep cutout passes fully opaque after discard
    // to avoid backend/material-mode differences on alpha-mask handling.
    if transparent_pass {
        pbr_input.material.base_color.a = atlas_alpha;
        // Keep std alpha handling for transparent passes.
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

    // Stage 3: Lighting.
    // Hybrid path:
    // - Fast/Standard: cheap custom voxel lighting (no dynamic shadow sampling).
    // - Fancy presets: Bevy PBR lighting + CSM shadows + lightweight post stylization.
    var out: FragmentOutput;
    var water_scene_reflection = vec3<f32>(0.0, 0.0, 0.0);
    var water_scene_reflection_valid = 0.0;
    var reflection_normal = safe_normalize(pbr_input.N, vec3(0.0, 1.0, 0.0));
    if is_water_surface {
        let amp = lighting_uniform.water_effects.y;
        reflection_normal = safe_normalize(
            reflection_normal + vec3<f32>(water_wave.x * amp * 0.28, 0.0, water_wave.y * amp * 0.28),
            reflection_normal,
        );
    }
    if is_water_surface && lighting_uniform.water_effects.x > 3.5 {
#ifdef DEPTH_PREPASS
            let reflect_dir = reflect(-safe_normalize(pbr_input.V, vec3<f32>(0.0, 0.0, 1.0)), reflection_normal);
            let ssr_hit = water_ssr_raymarch(
                in.world_position.xyz + reflection_normal * 0.08,
                reflect_dir,
                water_wave,
            );
            water_scene_reflection = ssr_hit.rgb;
            water_scene_reflection_valid = ssr_hit.a;
#endif
    }
    let base_normal = safe_normalize(pbr_input.N, vec3(0.0, 1.0, 0.0));
    var normal = base_normal;
    if is_water_surface {
        let amp = lighting_uniform.water_effects.y;
        normal = safe_normalize(
            base_normal + vec3<f32>(water_wave.x * amp * 0.28, 0.0, water_wave.y * amp * 0.28),
            base_normal,
        );
    }
    let view_dir = safe_normalize(pbr_input.V, vec3(0.0, 0.0, 1.0));
    let quality_mode = lighting_uniform.quality_and_water.x;
    if quality_mode >= 2.0 && (pbr_input.material.flags & STANDARD_MATERIAL_FLAGS_UNLIT_BIT) == 0u {
        out.color = apply_pbr_lighting(pbr_input);
        out.color = apply_fancy_post_lighting(
            out.color,
            normal,
            abs(in.position.w),
            view_dir,
            is_water_surface,
            water_scene_reflection,
            water_scene_reflection_valid,
        );
    } else {
        out.color = apply_voxel_lighting(
            pbr_input.material.base_color,
            normal,
            abs(in.position.w),
            view_dir,
            is_water_surface,
            water_scene_reflection,
            water_scene_reflection_valid,
        );
    }
    if !fixed_debug_state {
        out.color = vec4(apply_color_grading(out.color.rgb), out.color.a);
    }

    // Apply in-shader post processing (fog, alpha-premultiply, and optional tonemapping/debanding).
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
#endif

#ifdef OIT_ENABLED
#ifndef PREPASS_PIPELINE
    let alpha_mode = pbr_input.material.flags & pbr_types::STANDARD_MATERIAL_FLAGS_ALPHA_MODE_RESERVED_BITS;
    if alpha_mode != pbr_types::STANDARD_MATERIAL_FLAGS_ALPHA_MODE_OPAQUE {
        // The fragments will only be drawn during the oit resolve pass.
        oit_draw(in.position, out.color);
        discard;
    }
#endif
#endif // OIT_ENABLED

#ifdef FORWARD_DECAL
#ifndef PREPASS_PIPELINE
    out.color.a = min(forward_decal_info.alpha, out.color.a);
#endif
#endif

#ifdef PREPASS_PIPELINE
#ifdef PREPASS_FRAGMENT
    return out;
#else
    return;
#endif
#else
    return out;
#endif
}
