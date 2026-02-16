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
    pbr_functions::main_pass_post_lighting_processing,
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

struct AtlasLightingUniform {
    sun_dir_and_strength: vec4<f32>,
    ambient_and_fog: vec4<f32>,
    quality_and_water: vec4<f32>,
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

fn apply_voxel_lighting(base: vec4<f32>, normal: vec3<f32>, view_z: f32, view_dir: vec3<f32>) -> vec4<f32> {
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

#ifdef VERTEX_UVS_A
    var tile_origin = vec2<f32>(0.0, 0.0);
#ifdef VERTEX_UVS_B
    tile_origin = in.uv_b;
#endif
    let atlas_uv = atlas_uv_from_repeating(in.uv, tile_origin);
    pbr_input.material.base_color *= textureSample(atlas_texture, atlas_sampler, atlas_uv);
#endif

    // Alpha discard.
    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);

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
    // Custom voxel lighting path (fast/fancy quality presets). This avoids per-pixel
    // clustered light evaluation for chunk materials.
    var out: FragmentOutput;
    let voxel_lit = apply_voxel_lighting(
        pbr_input.material.base_color,
        safe_normalize(pbr_input.N, vec3(0.0, 1.0, 0.0)),
        abs(in.position.w),
        safe_normalize(pbr_input.V, vec3(0.0, 0.0, 1.0)),
    );
    out.color = voxel_lit;

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
