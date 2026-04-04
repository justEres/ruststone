use bevy::prelude::*;

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum AntiAliasingMode {
    Off,
    Fxaa,
    SmaaHigh,
    SmaaUltra,
    Msaa4,
    Msaa8,
}

impl Default for AntiAliasingMode {
    fn default() -> Self {
        Self::SmaaUltra
    }
}

impl AntiAliasingMode {
    pub const ALL: [Self; 6] = [
        Self::Off,
        Self::Fxaa,
        Self::SmaaHigh,
        Self::SmaaUltra,
        Self::Msaa4,
        Self::Msaa8,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Fxaa => "FXAA",
            Self::SmaaHigh => "SMAA High",
            Self::SmaaUltra => "SMAA Ultra",
            Self::Msaa4 => "MSAA 4x",
            Self::Msaa8 => "MSAA 8x",
        }
    }

    pub const fn as_options_value(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Fxaa => "fxaa",
            Self::SmaaHigh => "smaa_high",
            Self::SmaaUltra => "smaa_ultra",
            Self::Msaa4 => "msaa4",
            Self::Msaa8 => "msaa8",
        }
    }

    pub fn from_options_value(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "off" => Some(Self::Off),
            "fxaa" => Some(Self::Fxaa),
            "smaa_high" | "smaahigh" => Some(Self::SmaaHigh),
            "smaa_ultra" | "smaaultra" => Some(Self::SmaaUltra),
            "msaa4" | "msaa_4" => Some(Self::Msaa4),
            "msaa8" | "msaa_8" => Some(Self::Msaa8),
            _ => None,
        }
    }
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum ShadingModel {
    ClassicFast,
    VanillaLighting,
    PbrFancy,
}

impl Default for ShadingModel {
    fn default() -> Self {
        Self::VanillaLighting
    }
}

impl ShadingModel {
    pub const ALL: [Self; 3] = [Self::ClassicFast, Self::VanillaLighting, Self::PbrFancy];

    pub const fn label(self) -> &'static str {
        match self {
            Self::ClassicFast => "Classic Fast",
            Self::VanillaLighting => "Vanilla Lighting",
            Self::PbrFancy => "PBR Fancy",
        }
    }

    pub const fn as_options_value(self) -> &'static str {
        match self {
            Self::ClassicFast => "classic_fast",
            Self::VanillaLighting => "vanilla_lighting",
            Self::PbrFancy => "pbr_fancy",
        }
    }

    pub fn from_options_value(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "classic_fast" | "classicfast" | "fast" => Some(Self::ClassicFast),
            "vanilla_lighting" | "vanillalighting" | "vanilla" => Some(Self::VanillaLighting),
            "pbr_fancy" | "pbrfancy" | "pbr" | "fancy" => Some(Self::PbrFancy),
            _ => None,
        }
    }
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum VanillaBlockShadowMode {
    Off,
    SkylightOnly,
    SkylightPlusSunTrace,
}

impl Default for VanillaBlockShadowMode {
    fn default() -> Self {
        Self::SkylightOnly
    }
}

impl VanillaBlockShadowMode {
    pub const ALL: [Self; 3] = [Self::Off, Self::SkylightOnly, Self::SkylightPlusSunTrace];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::SkylightOnly => "Skylight Only",
            Self::SkylightPlusSunTrace => "Skylight + Sun Trace",
        }
    }

    pub const fn as_options_value(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::SkylightOnly => "skylight_only",
            Self::SkylightPlusSunTrace => "skylight_plus_sun_trace",
        }
    }

    pub fn from_options_value(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "off" => Some(Self::Off),
            "skylight_only" | "skylightonly" => Some(Self::SkylightOnly),
            "skylight_plus_sun_trace" | "skylightplussuntrace" | "sun_trace" => {
                Some(Self::SkylightPlusSunTrace)
            }
            _ => None,
        }
    }
}

#[derive(Resource, Debug, Clone)]
pub struct RenderDebugSettings {
    pub shadows_enabled: bool,
    pub shadow_distance_scale: f32,
    pub render_distance_chunks: i32,
    pub infinite_render_distance: bool,
    pub fov_deg: f32,
    pub use_greedy_meshing: bool,
    pub wireframe_enabled: bool,
    pub aa_mode: AntiAliasingMode,
    pub occlusion_cull_enabled: bool,
    pub occlusion_anchor_player: bool,
    pub cull_guard_chunk_radius: i32,
    pub frustum_fov_debug: bool,
    pub frustum_fov_deg: f32,
    pub show_chunk_borders: bool,
    pub show_coordinates: bool,
    pub show_look_info: bool,
    pub show_look_ray: bool,
    pub show_target_block_outline: bool,
    pub render_held_items: bool,
    pub render_first_person_arms: bool,
    pub render_self_model: bool,
    pub shading_model: ShadingModel,
    pub shader_quality_mode: u8,
    pub enable_pbr_terrain_lighting: bool,
    pub vanilla_sky_light_strength: f32,
    pub vanilla_block_light_strength: f32,
    pub vanilla_face_shading_strength: f32,
    pub vanilla_ambient_floor: f32,
    pub vanilla_light_curve: f32,
    pub vanilla_foliage_tint_strength: f32,
    pub vanilla_block_shadow_mode: VanillaBlockShadowMode,
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
    pub barrier_billboard: bool,
    pub water_reflections_enabled: bool,
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
    pub water_reflection_screen_space: bool,
    pub water_ssr_steps: u8,
    pub water_ssr_thickness: f32,
    pub water_ssr_max_distance: f32,
    pub water_ssr_stride: f32,
    pub flight_speed_boost_enabled: bool,
    pub flight_speed_boost_multiplier: f32,
    pub mesh_enqueue_budget_per_frame: u32,
    pub mesh_apply_budget_per_frame: u32,
    pub mesh_max_in_flight: u32,
    pub cutout_debug_mode: u8,
    pub show_layer_entities: bool,
    pub show_layer_chunks_opaque: bool,
    pub show_layer_chunks_cutout: bool,
    pub show_layer_chunks_transparent: bool,
    pub force_remesh: bool,
    pub clear_and_rebuild_meshes: bool,
    pub material_rebuild_nonce: u32,
}

impl Default for RenderDebugSettings {
    fn default() -> Self {
        Self {
            shadows_enabled: true,
            shadow_distance_scale: 1.0,
            render_distance_chunks: 12,
            infinite_render_distance: false,
            fov_deg: 110.0,
            use_greedy_meshing: true,
            wireframe_enabled: false,
            aa_mode: AntiAliasingMode::default(),
            occlusion_cull_enabled: true,
            occlusion_anchor_player: false,
            cull_guard_chunk_radius: 1,
            frustum_fov_debug: false,
            frustum_fov_deg: 110.0,
            show_chunk_borders: false,
            show_coordinates: false,
            show_look_info: false,
            show_look_ray: false,
            show_target_block_outline: true,
            render_held_items: true,
            render_first_person_arms: true,
            render_self_model: true,
            shading_model: ShadingModel::VanillaLighting,
            shader_quality_mode: 2,
            enable_pbr_terrain_lighting: false,
            vanilla_sky_light_strength: 1.00,
            vanilla_block_light_strength: 0.96,
            vanilla_face_shading_strength: 0.52,
            vanilla_ambient_floor: 0.26,
            vanilla_light_curve: 1.10,
            vanilla_foliage_tint_strength: 1.00,
            vanilla_block_shadow_mode: VanillaBlockShadowMode::SkylightOnly,
            vanilla_block_shadow_strength: 0.28,
            vanilla_sun_trace_samples: 4,
            vanilla_sun_trace_distance: 4.0,
            vanilla_top_face_sun_bias: 0.12,
            vanilla_ao_shadow_blend: 0.40,
            sync_sun_with_time: true,
            render_sun_sprite: true,
            sun_azimuth_deg: 62.0,
            sun_elevation_deg: 62.0,
            sun_strength: 0.56,
            sun_warmth: 0.18,
            shadow_opacity: 1.0,
            player_shadow_opacity: 1.0,
            ambient_strength: 0.52,
            ambient_brightness: 0.80,
            sun_illuminance: 11_500.0,
            fill_illuminance: 2_200.0,
            fog_enabled: false,
            fog_intensity: 1.0,
            fog_density: 0.012,
            fog_start: 70.0,
            fog_end: 220.0,
            water_absorption: 0.18,
            water_fresnel: 0.12,
            shadow_map_size: 1536,
            shadow_cascades: 2,
            shadow_max_distance: 96.0,
            shadow_first_cascade_far_bound: 28.0,
            shadow_depth_bias: 0.022,
            shadow_normal_bias: 0.55,
            color_saturation: 1.08,
            color_contrast: 1.06,
            color_brightness: 0.0,
            color_gamma: 1.0,
            voxel_ao_enabled: true,
            voxel_ao_strength: 1.0,
            voxel_ao_cutout: true,
            barrier_billboard: true,
            water_reflections_enabled: true,
            water_reflection_strength: 0.85,
            water_reflection_near_boost: 0.18,
            water_reflection_blue_tint: false,
            water_reflection_tint_strength: 0.20,
            water_wave_strength: 0.42,
            water_wave_speed: 1.0,
            water_wave_detail_strength: 0.22,
            water_wave_detail_scale: 2.4,
            water_wave_detail_speed: 1.7,
            water_reflection_edge_fade: 0.22,
            water_reflection_sky_fill: 0.55,
            water_reflection_screen_space: false,
            water_ssr_steps: 28,
            water_ssr_thickness: 0.24,
            water_ssr_max_distance: 80.0,
            water_ssr_stride: 1.25,
            flight_speed_boost_enabled: false,
            flight_speed_boost_multiplier: 2.0,
            mesh_enqueue_budget_per_frame: 24,
            mesh_apply_budget_per_frame: 8,
            mesh_max_in_flight: 48,
            cutout_debug_mode: 0,
            show_layer_entities: true,
            show_layer_chunks_opaque: true,
            show_layer_chunks_cutout: true,
            show_layer_chunks_transparent: true,
            force_remesh: false,
            clear_and_rebuild_meshes: false,
            material_rebuild_nonce: 0,
        }
    }
}

#[derive(Resource, Debug, Clone, Copy)]
pub struct MeshingToggleState {
    pub last_use_greedy: bool,
    pub last_voxel_ao_enabled: bool,
    pub last_voxel_ao_cutout: bool,
    pub last_voxel_ao_strength: f32,
    pub last_barrier_billboard: bool,
    pub last_shading_model: ShadingModel,
    pub last_vanilla_block_shadow_mode: VanillaBlockShadowMode,
    pub last_vanilla_block_shadow_strength: f32,
    pub last_vanilla_sun_trace_samples: u8,
    pub last_vanilla_sun_trace_distance: f32,
    pub last_vanilla_top_face_sun_bias: f32,
    pub last_vanilla_face_shading_strength: f32,
    pub last_vanilla_ambient_floor: f32,
    pub last_vanilla_light_curve: f32,
    pub last_vanilla_foliage_tint_strength: f32,
    pub last_vanilla_sky_light_strength: f32,
    pub last_vanilla_block_light_strength: f32,
    pub last_vanilla_ao_shadow_blend: f32,
}

impl Default for MeshingToggleState {
    fn default() -> Self {
        Self {
            last_use_greedy: true,
            last_voxel_ao_enabled: true,
            last_voxel_ao_cutout: true,
            last_voxel_ao_strength: 1.0,
            last_barrier_billboard: true,
            last_shading_model: ShadingModel::VanillaLighting,
            last_vanilla_block_shadow_mode: VanillaBlockShadowMode::SkylightOnly,
            last_vanilla_block_shadow_strength: 0.42,
            last_vanilla_sun_trace_samples: 4,
            last_vanilla_sun_trace_distance: 4.0,
            last_vanilla_top_face_sun_bias: 0.12,
            last_vanilla_face_shading_strength: 0.70,
            last_vanilla_ambient_floor: 0.16,
            last_vanilla_light_curve: 1.10,
            last_vanilla_foliage_tint_strength: 1.00,
            last_vanilla_sky_light_strength: 1.00,
            last_vanilla_block_light_strength: 0.92,
            last_vanilla_ao_shadow_blend: 0.55,
        }
    }
}

#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct RenderPerfStats {
    pub last_mesh_build_ms: f32,
    pub avg_mesh_build_ms: f32,
    pub last_apply_ms: f32,
    pub avg_apply_ms: f32,
    pub last_enqueue_ms: f32,
    pub avg_enqueue_ms: f32,
    pub last_meshes_applied: u32,
    pub in_flight: u32,
    pub last_updates: u32,
    pub last_updates_raw: u32,
    pub total_meshes: u32,
    pub visible_meshes_distance: u32,
    pub visible_meshes_view: u32,
    pub total_chunks: u32,
    pub visible_chunks: u32,
    pub apply_debug_ms: f32,
    pub gather_stats_ms: f32,
    pub occlusion_cull_ms: f32,
    pub visible_chunks_after_occlusion: u32,
    pub occluded_chunks: u32,
    pub mat_pass_opaque: f32,
    pub mat_pass_cutout: f32,
    pub mat_pass_cutout_culled: f32,
    pub mat_pass_transparent: f32,
    pub mat_alpha_opaque: u8,
    pub mat_alpha_cutout: u8,
    pub mat_alpha_cutout_culled: u8,
    pub mat_alpha_transparent: u8,
    pub mat_unlit_opaque: bool,
    pub mat_unlit_cutout: bool,
    pub mat_unlit_cutout_culled: bool,
    pub mat_unlit_transparent: bool,
    pub shading_model: u32,
    pub gpu_timing_supported: bool,
    pub gpu_frame_ms: f32,
    pub gpu_hottest_pass_ms: f32,
    pub mesh_bake_shadow_ms: f32,
}
