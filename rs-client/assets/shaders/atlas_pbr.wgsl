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
    reflection_view_proj: mat4x4<f32>,
}

@group(2) @binding(102) var<uniform> lighting_uniform: AtlasLightingUniform;

fn atlas_uv_from_repeating(local_uv: vec2<f32>, tile_origin: vec2<f32>) -> vec2<f32> {
    let tile_size = vec2<f32>(1.0 / ATLAS_COLUMNS, 1.0 / ATLAS_ROWS);
    let inset = tile_size * ATLAS_UV_INSET;
    return tile_origin + inset + fract(local_uv) * (tile_size - inset * 2.0);
}

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

    let transparent_pass = lighting_uniform.quality_and_water.w > 0.5;
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
            let env_reflection = mix(
                env_reflection_base,
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

    let transparent_pass = lighting_uniform.quality_and_water.w > 0.5;
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
            let env_reflection = mix(
                env_reflection_base,
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

    // Generate a PbrInput struct from the StandardMaterial bindings.
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    let transparent_pass = lighting_uniform.quality_and_water.w > 0.5;

#ifdef VERTEX_UVS_A
    var tile_origin = vec2<f32>(0.0, 0.0);
#ifdef VERTEX_UVS_B
    tile_origin = in.uv_b;
#endif
    var uv_local = in.uv;
    var water_wave = vec2<f32>(0.0, 0.0);
    if transparent_pass {
        let t = lighting_uniform.water_effects.w * lighting_uniform.water_effects.z;
        let amp = lighting_uniform.water_effects.y;
        let wp = in.world_position.xyz;
        water_wave = vec2<f32>(
            sin((wp.x + wp.z) * 0.42 + t * 2.3)
                + 0.55 * sin(wp.x * 0.95 - t * 1.7),
            cos((wp.x - wp.z) * 0.31 - t * 1.6)
                + 0.5 * cos(wp.z * 1.05 + t * 1.4),
        );
        let wave_scale = amp * 0.11;
        uv_local += water_wave * wave_scale;
    }
    let atlas_uv = atlas_uv_from_repeating(uv_local, tile_origin);
    let atlas_sample = textureSample(atlas_texture, atlas_sampler, atlas_uv);
    pbr_input.material.base_color *= atlas_sample;
    let atlas_alpha = atlas_sample.a;
#else
    let atlas_alpha = pbr_input.material.base_color.a;
#endif

    if transparent_pass {
        // Keep water/lava blend, but still punch fully transparent texels.
        if atlas_alpha <= 0.001 {
            discard;
        }
    } else {
        // Hard cutout for foliage/glass/cross meshes.
        if atlas_alpha < 0.5 {
            discard;
        }
    }

    // We do our own alpha cutout above. Keep cutout passes fully opaque after discard
    // to avoid backend/material-mode differences on alpha-mask handling.
    if transparent_pass {
        pbr_input.material.base_color.a = atlas_alpha;
        // Keep std alpha handling for blended passes (water).
        pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);
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
            let uv = vec2<f32>(ndc.x * 0.5 + 0.5, 1.0 - (ndc.y * 0.5 + 0.5))
                + water_wave * (lighting_uniform.water_effects.y * 0.025);
            if all(uv >= vec2<f32>(0.001, 0.001)) && all(uv <= vec2<f32>(0.999, 0.999)) {
                water_scene_reflection = textureSample(reflection_texture, reflection_sampler, uv).rgb;
                water_scene_reflection_valid = 1.0;
            }
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
    if quality_mode >= 2.0 && (pbr_input.material.flags & STANDARD_MATERIAL_FLAGS_UNLIT_BIT) == 0u {
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
