use super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ClientOptionsFile {
    pub fov_deg: f32,
    pub render_distance_chunks: i32,
    pub infinite_render_distance: bool,
    pub mesh_enqueue_budget_per_frame: u32,
    pub mesh_apply_budget_per_frame: u32,
    pub mesh_max_in_flight: u32,
    pub shadows_enabled: bool,
    pub shadow_distance_scale: f32,
    pub aa_mode: String,
    pub occlusion_cull_enabled: bool,
    pub occlusion_anchor_player: bool,
    pub cull_guard_chunk_radius: i32,
    pub frustum_fov_debug: bool,
    pub frustum_fov_deg: f32,
    pub vsync_enabled: bool,
    pub use_greedy_meshing: bool,
    pub wireframe_enabled: bool,
    pub render_held_items: bool,
    pub render_first_person_arms: bool,
    pub render_self_model: bool,
    pub show_chunk_borders: bool,
    pub shading_model: String,
    pub shader_quality_mode: u8,
    pub enable_pbr_terrain_lighting: bool,
    pub vanilla_sky_light_strength: f32,
    pub vanilla_block_light_strength: f32,
    pub vanilla_face_shading_strength: f32,
    pub vanilla_ambient_floor: f32,
    pub vanilla_light_curve: f32,
    pub vanilla_foliage_tint_strength: f32,
    pub vanilla_block_shadow_mode: String,
    pub vanilla_block_shadow_strength: f32,
    pub vanilla_sun_trace_samples: u8,
    pub vanilla_sun_trace_distance: f32,
    pub vanilla_top_face_sun_bias: f32,
    pub vanilla_ao_shadow_blend: f32,
    pub sync_sun_with_time: bool,
    pub render_sun_sprite: bool,
    pub sun_azimuth_deg: f32,
    pub sun_elevation_deg: f32,
    pub sun_strength: f32,
    pub sun_warmth: f32,
    pub shadow_opacity: f32,
    pub player_shadow_opacity: f32,
    pub ambient_strength: f32,
    pub ambient_brightness: f32,
    pub sun_illuminance: f32,
    pub fill_illuminance: f32,
    pub fog_enabled: bool,
    pub fog_intensity: f32,
    pub fog_density: f32,
    pub fog_start: f32,
    pub fog_end: f32,
    pub water_absorption: f32,
    pub water_fresnel: f32,
    pub shadow_map_size: u32,
    pub shadow_cascades: u8,
    pub shadow_max_distance: f32,
    pub shadow_first_cascade_far_bound: f32,
    pub shadow_depth_bias: f32,
    pub shadow_normal_bias: f32,
    pub color_saturation: f32,
    pub color_contrast: f32,
    pub color_brightness: f32,
    pub color_gamma: f32,
    pub voxel_ao_enabled: bool,
    pub voxel_ao_strength: f32,
    pub voxel_ao_cutout: bool,
    pub water_reflections_enabled: bool,
    pub water_reflection_screen_space: bool,
    pub water_reflection_strength: f32,
    pub water_reflection_near_boost: f32,
    pub water_reflection_blue_tint: bool,
    pub water_reflection_tint_strength: f32,
    pub water_wave_strength: f32,
    pub water_wave_speed: f32,
    pub water_wave_detail_strength: f32,
    pub water_wave_detail_scale: f32,
    pub water_wave_detail_speed: f32,
    pub water_reflection_edge_fade: f32,
    pub water_reflection_sky_fill: f32,
    pub water_ssr_steps: u8,
    pub water_ssr_thickness: f32,
    pub water_ssr_max_distance: f32,
    pub water_ssr_stride: f32,
    pub sound_master: f32,
    pub sound_music: f32,
    pub sound_record: f32,
    pub sound_weather: f32,
    pub sound_block: f32,
    pub sound_hostile: f32,
    pub sound_neutral: f32,
    pub sound_player: f32,
    pub sound_ambient: f32,
    pub chat_background_opacity: f32,
    pub chat_font_size: f32,
    pub scoreboard_background_opacity: f32,
    pub scoreboard_font_size: f32,
    pub title_background_opacity: f32,
    pub title_font_size: f32,
    pub flight_speed_boost_enabled: bool,
    pub flight_speed_boost_multiplier: f32,
    pub cutout_debug_mode: u8,
    pub show_layer_entities: bool,
    pub show_layer_chunks_opaque: bool,
    pub show_layer_chunks_cutout: bool,
    pub show_layer_chunks_transparent: bool,
}

impl Default for ClientOptionsFile {
    fn default() -> Self {
        let render = RenderDebugSettings::default();
        Self {
            fov_deg: render.fov_deg,
            render_distance_chunks: render.render_distance_chunks,
            infinite_render_distance: render.infinite_render_distance,
            mesh_enqueue_budget_per_frame: render.mesh_enqueue_budget_per_frame,
            mesh_apply_budget_per_frame: render.mesh_apply_budget_per_frame,
            mesh_max_in_flight: render.mesh_max_in_flight,
            shadows_enabled: render.shadows_enabled,
            shadow_distance_scale: render.shadow_distance_scale,
            aa_mode: render.aa_mode.as_options_value().to_string(),
            occlusion_cull_enabled: render.occlusion_cull_enabled,
            occlusion_anchor_player: render.occlusion_anchor_player,
            cull_guard_chunk_radius: render.cull_guard_chunk_radius,
            frustum_fov_debug: render.frustum_fov_debug,
            frustum_fov_deg: render.frustum_fov_deg,
            vsync_enabled: false,
            use_greedy_meshing: render.use_greedy_meshing,
            wireframe_enabled: render.wireframe_enabled,
            render_held_items: render.render_held_items,
            render_first_person_arms: render.render_first_person_arms,
            render_self_model: render.render_self_model,
            show_chunk_borders: render.show_chunk_borders,
            shading_model: render.shading_model.as_options_value().to_string(),
            shader_quality_mode: render.shader_quality_mode,
            enable_pbr_terrain_lighting: render.enable_pbr_terrain_lighting,
            vanilla_sky_light_strength: render.vanilla_sky_light_strength,
            vanilla_block_light_strength: render.vanilla_block_light_strength,
            vanilla_face_shading_strength: render.vanilla_face_shading_strength,
            vanilla_ambient_floor: render.vanilla_ambient_floor,
            vanilla_light_curve: render.vanilla_light_curve,
            vanilla_foliage_tint_strength: render.vanilla_foliage_tint_strength,
            vanilla_block_shadow_mode: render.vanilla_block_shadow_mode.as_options_value().to_string(),
            vanilla_block_shadow_strength: render.vanilla_block_shadow_strength,
            vanilla_sun_trace_samples: render.vanilla_sun_trace_samples,
            vanilla_sun_trace_distance: render.vanilla_sun_trace_distance,
            vanilla_top_face_sun_bias: render.vanilla_top_face_sun_bias,
            vanilla_ao_shadow_blend: render.vanilla_ao_shadow_blend,
            sync_sun_with_time: render.sync_sun_with_time,
            render_sun_sprite: render.render_sun_sprite,
            sun_azimuth_deg: render.sun_azimuth_deg,
            sun_elevation_deg: render.sun_elevation_deg,
            sun_strength: render.sun_strength,
            sun_warmth: render.sun_warmth,
            shadow_opacity: render.shadow_opacity,
            player_shadow_opacity: render.player_shadow_opacity,
            ambient_strength: render.ambient_strength,
            ambient_brightness: render.ambient_brightness,
            sun_illuminance: render.sun_illuminance,
            fill_illuminance: render.fill_illuminance,
            fog_enabled: render.fog_enabled,
            fog_intensity: render.fog_intensity,
            fog_density: render.fog_density,
            fog_start: render.fog_start,
            fog_end: render.fog_end,
            water_absorption: render.water_absorption,
            water_fresnel: render.water_fresnel,
            shadow_map_size: render.shadow_map_size,
            shadow_cascades: render.shadow_cascades,
            shadow_max_distance: render.shadow_max_distance,
            shadow_first_cascade_far_bound: render.shadow_first_cascade_far_bound,
            shadow_depth_bias: render.shadow_depth_bias,
            shadow_normal_bias: render.shadow_normal_bias,
            color_saturation: render.color_saturation,
            color_contrast: render.color_contrast,
            color_brightness: render.color_brightness,
            color_gamma: render.color_gamma,
            voxel_ao_enabled: render.voxel_ao_enabled,
            voxel_ao_strength: render.voxel_ao_strength,
            voxel_ao_cutout: render.voxel_ao_cutout,
            water_reflections_enabled: render.water_reflections_enabled,
            water_reflection_screen_space: render.water_reflection_screen_space,
            water_reflection_strength: render.water_reflection_strength,
            water_reflection_near_boost: render.water_reflection_near_boost,
            water_reflection_blue_tint: render.water_reflection_blue_tint,
            water_reflection_tint_strength: render.water_reflection_tint_strength,
            water_wave_strength: render.water_wave_strength,
            water_wave_speed: render.water_wave_speed,
            water_wave_detail_strength: render.water_wave_detail_strength,
            water_wave_detail_scale: render.water_wave_detail_scale,
            water_wave_detail_speed: render.water_wave_detail_speed,
            water_reflection_edge_fade: render.water_reflection_edge_fade,
            water_reflection_sky_fill: render.water_reflection_sky_fill,
            water_ssr_steps: render.water_ssr_steps,
            water_ssr_thickness: render.water_ssr_thickness,
            water_ssr_max_distance: render.water_ssr_max_distance,
            water_ssr_stride: render.water_ssr_stride,
            sound_master: 1.0,
            sound_music: 1.0,
            sound_record: 1.0,
            sound_weather: 1.0,
            sound_block: 1.0,
            sound_hostile: 1.0,
            sound_neutral: 1.0,
            sound_player: 1.0,
            sound_ambient: 1.0,
            chat_background_opacity: 96.0,
            chat_font_size: 15.0,
            scoreboard_background_opacity: 112.0,
            scoreboard_font_size: 15.5,
            title_background_opacity: 80.0,
            title_font_size: 34.0,
            flight_speed_boost_enabled: render.flight_speed_boost_enabled,
            flight_speed_boost_multiplier: render.flight_speed_boost_multiplier,
            cutout_debug_mode: render.cutout_debug_mode,
            show_layer_entities: render.show_layer_entities,
            show_layer_chunks_opaque: render.show_layer_chunks_opaque,
            show_layer_chunks_cutout: render.show_layer_chunks_cutout,
            show_layer_chunks_transparent: render.show_layer_chunks_transparent,
        }
    }
}

pub(crate) fn options_to_file(
    state: &ConnectUiState,
    render: &RenderDebugSettings,
    sound: &SoundSettings,
) -> ClientOptionsFile {
    ClientOptionsFile {
        fov_deg: render.fov_deg,
        render_distance_chunks: render.render_distance_chunks,
        infinite_render_distance: render.infinite_render_distance,
        mesh_enqueue_budget_per_frame: render.mesh_enqueue_budget_per_frame,
        mesh_apply_budget_per_frame: render.mesh_apply_budget_per_frame,
        mesh_max_in_flight: render.mesh_max_in_flight,
        shadows_enabled: render.shadows_enabled,
        shadow_distance_scale: render.shadow_distance_scale,
        aa_mode: render.aa_mode.as_options_value().to_string(),
        occlusion_cull_enabled: render.occlusion_cull_enabled,
        occlusion_anchor_player: render.occlusion_anchor_player,
        cull_guard_chunk_radius: render.cull_guard_chunk_radius,
        frustum_fov_debug: render.frustum_fov_debug,
        frustum_fov_deg: render.frustum_fov_deg,
        vsync_enabled: state.vsync_enabled,
        use_greedy_meshing: render.use_greedy_meshing,
        wireframe_enabled: render.wireframe_enabled,
        render_held_items: render.render_held_items,
        render_first_person_arms: render.render_first_person_arms,
        render_self_model: render.render_self_model,
        show_chunk_borders: render.show_chunk_borders,
        shading_model: render.shading_model.as_options_value().to_string(),
        shader_quality_mode: render.shader_quality_mode,
        enable_pbr_terrain_lighting: render.enable_pbr_terrain_lighting,
        vanilla_sky_light_strength: render.vanilla_sky_light_strength,
        vanilla_block_light_strength: render.vanilla_block_light_strength,
        vanilla_face_shading_strength: render.vanilla_face_shading_strength,
        vanilla_ambient_floor: render.vanilla_ambient_floor,
        vanilla_light_curve: render.vanilla_light_curve,
        vanilla_foliage_tint_strength: render.vanilla_foliage_tint_strength,
        vanilla_block_shadow_mode: render.vanilla_block_shadow_mode.as_options_value().to_string(),
        vanilla_block_shadow_strength: render.vanilla_block_shadow_strength,
        vanilla_sun_trace_samples: render.vanilla_sun_trace_samples,
        vanilla_sun_trace_distance: render.vanilla_sun_trace_distance,
        vanilla_top_face_sun_bias: render.vanilla_top_face_sun_bias,
        vanilla_ao_shadow_blend: render.vanilla_ao_shadow_blend,
        sync_sun_with_time: render.sync_sun_with_time,
        render_sun_sprite: render.render_sun_sprite,
        sun_azimuth_deg: render.sun_azimuth_deg,
        sun_elevation_deg: render.sun_elevation_deg,
        sun_strength: render.sun_strength,
        sun_warmth: render.sun_warmth,
        shadow_opacity: render.shadow_opacity,
        player_shadow_opacity: render.player_shadow_opacity,
        ambient_strength: render.ambient_strength,
        ambient_brightness: render.ambient_brightness,
        sun_illuminance: render.sun_illuminance,
        fill_illuminance: render.fill_illuminance,
        fog_enabled: render.fog_enabled,
        fog_intensity: render.fog_intensity,
        fog_density: render.fog_density,
        fog_start: render.fog_start,
        fog_end: render.fog_end,
        water_absorption: render.water_absorption,
        water_fresnel: render.water_fresnel,
        shadow_map_size: render.shadow_map_size,
        shadow_cascades: render.shadow_cascades,
        shadow_max_distance: render.shadow_max_distance,
        shadow_first_cascade_far_bound: render.shadow_first_cascade_far_bound,
        shadow_depth_bias: render.shadow_depth_bias,
        shadow_normal_bias: render.shadow_normal_bias,
        color_saturation: render.color_saturation,
        color_contrast: render.color_contrast,
        color_brightness: render.color_brightness,
        color_gamma: render.color_gamma,
        voxel_ao_enabled: render.voxel_ao_enabled,
        voxel_ao_strength: render.voxel_ao_strength,
        voxel_ao_cutout: render.voxel_ao_cutout,
        water_reflections_enabled: render.water_reflections_enabled,
        water_reflection_screen_space: render.water_reflection_screen_space,
        water_reflection_strength: render.water_reflection_strength,
        water_reflection_near_boost: render.water_reflection_near_boost,
        water_reflection_blue_tint: render.water_reflection_blue_tint,
        water_reflection_tint_strength: render.water_reflection_tint_strength,
        water_wave_strength: render.water_wave_strength,
        water_wave_speed: render.water_wave_speed,
        water_wave_detail_strength: render.water_wave_detail_strength,
        water_wave_detail_scale: render.water_wave_detail_scale,
        water_wave_detail_speed: render.water_wave_detail_speed,
        water_reflection_edge_fade: render.water_reflection_edge_fade,
        water_reflection_sky_fill: render.water_reflection_sky_fill,
        water_ssr_steps: render.water_ssr_steps,
        water_ssr_thickness: render.water_ssr_thickness,
        water_ssr_max_distance: render.water_ssr_max_distance,
        water_ssr_stride: render.water_ssr_stride,
        sound_master: sound.master,
        sound_music: sound.music,
        sound_record: sound.record,
        sound_weather: sound.weather,
        sound_block: sound.block,
        sound_hostile: sound.hostile,
        sound_neutral: sound.neutral,
        sound_player: sound.player,
        sound_ambient: sound.ambient,
        chat_background_opacity: state.chat_background_opacity,
        chat_font_size: state.chat_font_size,
        scoreboard_background_opacity: state.scoreboard_background_opacity,
        scoreboard_font_size: state.scoreboard_font_size,
        title_background_opacity: state.title_background_opacity,
        title_font_size: state.title_font_size,
        flight_speed_boost_enabled: render.flight_speed_boost_enabled,
        flight_speed_boost_multiplier: render.flight_speed_boost_multiplier,
        cutout_debug_mode: render.cutout_debug_mode,
        show_layer_entities: render.show_layer_entities,
        show_layer_chunks_opaque: render.show_layer_chunks_opaque,
        show_layer_chunks_cutout: render.show_layer_chunks_cutout,
        show_layer_chunks_transparent: render.show_layer_chunks_transparent,
    }
}

pub fn apply_options(
    options: &ClientOptionsFile,
    state: &mut ConnectUiState,
    render: &mut RenderDebugSettings,
    sound: &mut SoundSettings,
    window: &mut Window,
) {
    render.fov_deg = options.fov_deg.clamp(60.0, 140.0);
    render.render_distance_chunks = options.render_distance_chunks.clamp(2, 64);
    render.infinite_render_distance = options.infinite_render_distance;
    render.mesh_enqueue_budget_per_frame = options.mesh_enqueue_budget_per_frame.clamp(1, 128);
    render.mesh_apply_budget_per_frame = options.mesh_apply_budget_per_frame.clamp(1, 64);
    render.mesh_max_in_flight = options.mesh_max_in_flight.clamp(1, 256);
    render.shader_quality_mode = options.shader_quality_mode.clamp(0, 3);
    if let Some(mode) = AntiAliasingMode::from_options_value(&options.aa_mode) {
        render.aa_mode = mode;
    }
    // Explicit toggles in options file override preset defaults.
    render.shadows_enabled = options.shadows_enabled;
    render.shadow_distance_scale = options.shadow_distance_scale.clamp(0.25, 20.0);
    render.occlusion_cull_enabled = options.occlusion_cull_enabled;
    render.occlusion_anchor_player = options.occlusion_anchor_player;
    render.cull_guard_chunk_radius = options.cull_guard_chunk_radius.clamp(0, 5);
    render.frustum_fov_debug = options.frustum_fov_debug;
    render.frustum_fov_deg = options.frustum_fov_deg.clamp(30.0, 140.0);
    render.use_greedy_meshing = options.use_greedy_meshing;
    render.wireframe_enabled = options.wireframe_enabled;
    render.render_held_items = options.render_held_items;
    render.render_first_person_arms = options.render_first_person_arms;
    render.render_self_model = options.render_self_model;
    render.show_chunk_borders = options.show_chunk_borders;
    render.shading_model = ShadingModel::from_options_value(&options.shading_model)
        .unwrap_or(if options.enable_pbr_terrain_lighting {
            ShadingModel::PbrFancy
        } else {
            ShadingModel::VanillaLighting
        });
    render.enable_pbr_terrain_lighting = options.enable_pbr_terrain_lighting;
    render.vanilla_sky_light_strength = options.vanilla_sky_light_strength.clamp(0.0, 2.0);
    render.vanilla_block_light_strength = options.vanilla_block_light_strength.clamp(0.0, 2.0);
    render.vanilla_face_shading_strength =
        options.vanilla_face_shading_strength.clamp(0.0, 1.0);
    render.vanilla_ambient_floor = options.vanilla_ambient_floor.clamp(0.0, 0.95);
    render.vanilla_light_curve = options.vanilla_light_curve.clamp(0.35, 2.5);
    render.vanilla_foliage_tint_strength =
        options.vanilla_foliage_tint_strength.clamp(0.0, 2.5);
    render.vanilla_block_shadow_mode =
        VanillaBlockShadowMode::from_options_value(&options.vanilla_block_shadow_mode)
            .unwrap_or(VanillaBlockShadowMode::SkylightOnly);
    render.vanilla_block_shadow_strength =
        options.vanilla_block_shadow_strength.clamp(0.0, 1.0);
    render.vanilla_sun_trace_samples = options.vanilla_sun_trace_samples.clamp(1, 8);
    render.vanilla_sun_trace_distance = options.vanilla_sun_trace_distance.clamp(1.0, 12.0);
    render.vanilla_top_face_sun_bias = options.vanilla_top_face_sun_bias.clamp(0.0, 0.5);
    render.vanilla_ao_shadow_blend = options.vanilla_ao_shadow_blend.clamp(0.0, 1.0);
    render.sync_sun_with_time = options.sync_sun_with_time;
    render.render_sun_sprite = options.render_sun_sprite;
    render.sun_azimuth_deg = options.sun_azimuth_deg.clamp(-360.0, 360.0);
    render.sun_elevation_deg = options.sun_elevation_deg.clamp(-89.0, 89.0);
    render.sun_strength = options.sun_strength.clamp(0.0, 2.0);
    render.sun_warmth = options.sun_warmth.clamp(0.0, 1.0);
    render.shadow_opacity = options.shadow_opacity.clamp(0.0, 1.0);
    render.player_shadow_opacity = options.player_shadow_opacity.clamp(0.0, 1.0);
    render.ambient_strength = options.ambient_strength.clamp(0.0, 2.0);
    render.ambient_brightness = options.ambient_brightness.clamp(0.0, 2.0);
    render.sun_illuminance = options.sun_illuminance.clamp(0.0, 50_000.0);
    render.fill_illuminance = options.fill_illuminance.clamp(0.0, 10_000.0);
    render.fog_enabled = options.fog_enabled;
    render.fog_intensity = options.fog_intensity.clamp(0.0, 2.0);
    render.fog_density = options.fog_density.clamp(0.0, 0.1);
    render.fog_start = options.fog_start.clamp(0.0, 1_000.0);
    render.fog_end = options.fog_end.clamp(0.0, 2_000.0);
    render.water_absorption = options.water_absorption.clamp(0.0, 1.0);
    render.water_fresnel = options.water_fresnel.clamp(0.0, 1.0);
    render.shadow_map_size = options.shadow_map_size.clamp(256, 4096);
    render.shadow_cascades = options.shadow_cascades.clamp(1, 4);
    render.shadow_max_distance = options.shadow_max_distance.clamp(4.0, 500.0);
    render.shadow_first_cascade_far_bound =
        options.shadow_first_cascade_far_bound.clamp(1.0, 300.0);
    render.shadow_depth_bias = options.shadow_depth_bias.clamp(0.0, 0.2);
    render.shadow_normal_bias = options.shadow_normal_bias.clamp(0.0, 2.0);
    render.color_saturation = options.color_saturation.clamp(0.0, 2.0);
    render.color_contrast = options.color_contrast.clamp(0.0, 2.0);
    render.color_brightness = options.color_brightness.clamp(-0.5, 0.5);
    render.color_gamma = options.color_gamma.clamp(0.2, 2.5);
    render.voxel_ao_enabled = options.voxel_ao_enabled;
    render.voxel_ao_strength = options.voxel_ao_strength.clamp(0.0, 1.0);
    render.voxel_ao_cutout = options.voxel_ao_cutout;
    render.water_reflections_enabled = options.water_reflections_enabled;
    render.water_reflection_screen_space = options.water_reflection_screen_space;
    render.water_reflection_strength = options.water_reflection_strength.clamp(0.0, 3.0);
    render.water_reflection_near_boost = options.water_reflection_near_boost.clamp(0.0, 1.0);
    render.water_reflection_blue_tint = options.water_reflection_blue_tint;
    render.water_reflection_tint_strength = options.water_reflection_tint_strength.clamp(0.0, 2.0);
    render.water_wave_strength = options.water_wave_strength.clamp(0.0, 1.2);
    render.water_wave_speed = options.water_wave_speed.clamp(0.0, 4.0);
    render.water_wave_detail_strength = options.water_wave_detail_strength.clamp(0.0, 1.0);
    render.water_wave_detail_scale = options.water_wave_detail_scale.clamp(1.0, 8.0);
    render.water_wave_detail_speed = options.water_wave_detail_speed.clamp(0.0, 4.0);
    render.water_reflection_edge_fade = options.water_reflection_edge_fade.clamp(0.01, 0.5);
    render.water_reflection_sky_fill = options.water_reflection_sky_fill.clamp(0.0, 1.0);
    render.water_ssr_steps = options.water_ssr_steps.clamp(4, 64);
    render.water_ssr_thickness = options.water_ssr_thickness.clamp(0.02, 2.0);
    render.water_ssr_max_distance = options.water_ssr_max_distance.clamp(4.0, 400.0);
    render.water_ssr_stride = options.water_ssr_stride.clamp(0.2, 8.0);
    sound.master = options.sound_master.clamp(0.0, 1.0);
    sound.music = options.sound_music.clamp(0.0, 1.0);
    sound.record = options.sound_record.clamp(0.0, 1.0);
    sound.weather = options.sound_weather.clamp(0.0, 1.0);
    sound.block = options.sound_block.clamp(0.0, 1.0);
    sound.hostile = options.sound_hostile.clamp(0.0, 1.0);
    sound.neutral = options.sound_neutral.clamp(0.0, 1.0);
    sound.player = options.sound_player.clamp(0.0, 1.0);
    sound.ambient = options.sound_ambient.clamp(0.0, 1.0);
    state.chat_background_opacity = options.chat_background_opacity.clamp(0.0, 255.0);
    state.chat_font_size = options.chat_font_size.clamp(10.0, 28.0);
    state.scoreboard_background_opacity = options.scoreboard_background_opacity.clamp(0.0, 255.0);
    state.scoreboard_font_size = options.scoreboard_font_size.clamp(10.0, 28.0);
    state.title_background_opacity = options.title_background_opacity.clamp(0.0, 255.0);
    state.title_font_size = options.title_font_size.clamp(14.0, 56.0);
    render.flight_speed_boost_enabled = options.flight_speed_boost_enabled;
    render.flight_speed_boost_multiplier = options.flight_speed_boost_multiplier.clamp(1.0, 10.0);
    render.cutout_debug_mode = options.cutout_debug_mode.clamp(0, 8);
    render.show_layer_entities = options.show_layer_entities;
    render.show_layer_chunks_opaque = options.show_layer_chunks_opaque;
    render.show_layer_chunks_cutout = options.show_layer_chunks_cutout;
    render.show_layer_chunks_transparent = options.show_layer_chunks_transparent;
    state.vsync_enabled = options.vsync_enabled;
    window.present_mode = if state.vsync_enabled {
        PresentMode::AutoVsync
    } else {
        PresentMode::AutoNoVsync
    };
}

pub fn load_client_options(
    path: &str,
    state: &mut ConnectUiState,
    render: &mut RenderDebugSettings,
    sound: &mut SoundSettings,
    window: &mut Window,
) -> Result<(), String> {
    let path_buf = PathBuf::from(path);
    let content = match std::fs::read_to_string(&path_buf) {
        Ok(v) => v,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            let defaults = ClientOptionsFile::default();
            apply_options(&defaults, state, render, sound, window);
            return save_client_options(path, state, render, sound);
        }
        Err(err) => return Err(format!("Failed to read options file {}: {}", path, err)),
    };
    let parsed = toml::from_str::<ClientOptionsFile>(&content)
        .map_err(|err| format!("Invalid TOML options {}: {}", path, err))?;
    apply_options(&parsed, state, render, sound, window);
    Ok(())
}

pub fn save_client_options(
    path: &str,
    state: &ConnectUiState,
    render: &RenderDebugSettings,
    sound: &SoundSettings,
) -> Result<(), String> {
    let path_buf = PathBuf::from(path);
    if let Some(parent) = path_buf.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|err| {
            format!(
                "Failed to create options directory {}: {}",
                parent.display(),
                err
            )
        })?;
    }
    let body = toml::to_string_pretty(&options_to_file(state, render, sound))
        .map_err(|err| format!("Failed to encode options TOML: {}", err))?;
    std::fs::write(&path_buf, body)
        .map_err(|err| format!("Failed to write options file {}: {}", path, err))
}

pub(crate) fn short_uuid(uuid: &str) -> String {
    uuid.chars().take(8).collect::<String>()
}

pub(crate) fn default_prism_accounts_path() -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home)
        .join(".local")
        .join("share")
        .join("PrismLauncher")
        .join("accounts.json")
        .display()
        .to_string()
}

pub fn load_prism_accounts(prism_path: &str) -> Vec<UiAuthAccount> {
    let Ok(raw) = std::fs::read_to_string(prism_path) else {
        return Vec::new();
    };
    let Ok(root) = serde_json::from_str::<Value>(&raw) else {
        return Vec::new();
    };
    let Some(accounts) = root.get("accounts").and_then(Value::as_array) else {
        return Vec::new();
    };

    let mut out = Vec::new();
    for acc in accounts {
        if acc.get("type").and_then(Value::as_str) != Some("MSA") {
            continue;
        }
        let username = acc
            .pointer("/profile/name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let mut uuid = acc
            .pointer("/profile/id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        uuid.retain(|c| c != '-');
        let active = acc.get("active").and_then(Value::as_bool).unwrap_or(false);

        if username.is_empty() || uuid.len() != 32 {
            continue;
        }
        out.push(UiAuthAccount {
            username,
            uuid,
            active,
        });
    }
    out
}
