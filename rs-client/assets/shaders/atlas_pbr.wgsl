#import bevy_pbr::{
    pbr_types,
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
    pbr_functions,
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

fn atlas_uv_from_repeating(local_uv: vec2<f32>, tile_origin: vec2<f32>) -> vec2<f32> {
    let tile_size = vec2<f32>(1.0 / ATLAS_COLUMNS, 1.0 / ATLAS_ROWS);
    let inset = tile_size * ATLAS_UV_INSET;
    return tile_origin + inset + fract(local_uv) * (tile_size - inset * 2.0);
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
    // In forward mode, calculate the lit color now and apply post-lighting effects.
    var out: FragmentOutput;
    if (pbr_input.material.flags & STANDARD_MATERIAL_FLAGS_UNLIT_BIT) == 0u {
        out.color = apply_pbr_lighting(pbr_input);
    } else {
        out.color = pbr_input.material.base_color;
    }

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
