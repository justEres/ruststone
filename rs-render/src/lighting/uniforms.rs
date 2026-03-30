use std::time::Instant;

use bevy::prelude::*;

use crate::chunk::AtlasLightingUniform;
use crate::debug::{RenderDebugSettings, ShadingModel, VanillaBlockShadowMode};
use crate::reflection::DEFAULT_WATER_PLANE_Y;
use rs_utils::WorldTime;

pub(super) fn cutout_alpha_mode(settings: &RenderDebugSettings) -> AlphaMode {
    let _ = settings;
    AlphaMode::Mask(0.5)
}

pub(super) fn water_reflection_mode(settings: &RenderDebugSettings, fixed_debug: bool) -> f32 {
    if fixed_debug {
        0.0
    } else if settings.shading_model != ShadingModel::PbrFancy {
        if settings.water_reflections_enabled {
            2.0
        } else {
            0.0
        }
    } else if settings.water_reflections_enabled && settings.water_reflection_screen_space {
        4.0
    } else {
        0.0
    }
}

pub(super) fn sun_color_from_warmth(warmth: f32) -> Color {
    let t = warmth.clamp(0.0, 1.0);
    Color::srgb(1.0, 1.0 - 0.18 * t, 1.0 - 0.38 * t)
}

pub fn vanilla_celestial_angle(world_time: i64, partial_ticks: f32) -> f32 {
    let day_time = world_time.rem_euclid(24_000) as f32;
    let mut f = (day_time + partial_ticks) / 24_000.0 - 0.25;
    if f < 0.0 {
        f += 1.0;
    }
    if f > 1.0 {
        f -= 1.0;
    }
    let base = f;
    let curved = 1.0 - (((base * std::f32::consts::PI).cos() + 1.0) * 0.5);
    base + (curved - base) / 3.0
}

pub fn effective_sun_direction(
    settings: &RenderDebugSettings,
    world_time: Option<&WorldTime>,
) -> Vec3 {
    if settings.sync_sun_with_time {
        let time = world_time
            .map(|time| time.interpolated_time_of_day(Instant::now()))
            .unwrap_or(0.0);
        let angle =
            vanilla_celestial_angle(time.floor() as i64, time.fract()) * std::f32::consts::TAU;
        Vec3::new(0.0, angle.cos(), angle.sin()).normalize_or_zero()
    } else {
        let az = settings.sun_azimuth_deg.to_radians();
        let el = settings.sun_elevation_deg.to_radians();
        Vec3::new(el.cos() * az.cos(), el.sin(), el.cos() * az.sin()).normalize_or_zero()
    }
}

pub fn lighting_uniform_for_mode(
    settings: &RenderDebugSettings,
    world_time: Option<&WorldTime>,
    pass_mode: f32,
) -> AtlasLightingUniform {
    let fixed_debug = false;
    let sun_dir = effective_sun_direction(settings, world_time);
    let quality_mode = if fixed_debug {
        0.0
    } else {
        settings.shader_quality_mode.clamp(0, 3) as f32
    };
    let shading_model = match settings.shading_model {
        ShadingModel::ClassicFast => 0.0,
        ShadingModel::VanillaLighting => 1.0,
        ShadingModel::PbrFancy => 2.0,
    };
    let vanilla_shadow_mode = match settings.vanilla_block_shadow_mode {
        VanillaBlockShadowMode::Off => 0.0,
        VanillaBlockShadowMode::SkylightOnly => 1.0,
        VanillaBlockShadowMode::SkylightPlusSunTrace => 2.0,
    };
    AtlasLightingUniform {
        sun_dir_and_strength: Vec4::new(sun_dir.x, sun_dir.y, sun_dir.z, settings.sun_strength),
        ambient_and_fog: Vec4::new(
            settings.ambient_strength,
            if fixed_debug {
                0.0
            } else if settings.fog_enabled {
                settings.fog_density * settings.fog_intensity.clamp(0.0, 2.0)
            } else {
                0.0
            },
            if fixed_debug {
                10_000.0
            } else if settings.fog_enabled {
                settings.fog_start
            } else {
                10_000.0
            },
            if fixed_debug {
                10_001.0
            } else if settings.fog_enabled {
                settings.fog_end
            } else {
                10_001.0
            },
        ),
        quality_and_water: Vec4::new(
            quality_mode,
            settings.water_absorption,
            settings.water_fresnel,
            pass_mode,
        ),
        color_grading: Vec4::new(
            if fixed_debug { 1.0 } else { settings.color_saturation },
            if fixed_debug { 1.0 } else { settings.color_contrast },
            if fixed_debug { 0.0 } else { settings.color_brightness },
            if fixed_debug { 1.0 } else { settings.color_gamma },
        ),
        vanilla_light: Vec4::new(
            shading_model,
            settings.vanilla_sky_light_strength,
            settings.vanilla_block_light_strength,
            settings.vanilla_face_shading_strength,
        ),
        vanilla_shadow: Vec4::new(
            vanilla_shadow_mode,
            settings.vanilla_block_shadow_strength,
            settings.vanilla_light_curve,
            settings.vanilla_ambient_floor,
        ),
        water_effects: Vec4::new(
            water_reflection_mode(settings, fixed_debug),
            if fixed_debug {
                0.0
            } else {
                settings.water_wave_strength
            },
            if fixed_debug {
                0.0
            } else {
                settings.water_wave_speed
            },
            0.0,
        ),
        water_controls: Vec4::new(
            settings.water_reflection_strength,
            DEFAULT_WATER_PLANE_Y,
            settings.water_reflection_near_boost,
            if settings.water_reflection_blue_tint {
                settings.water_reflection_tint_strength
            } else {
                0.0
            },
        ),
        water_extra: Vec4::new(
            if fixed_debug {
                0.0
            } else {
                settings.water_wave_detail_strength
            },
            if fixed_debug {
                1.0
            } else {
                settings.water_wave_detail_scale
            },
            if fixed_debug {
                0.0
            } else {
                settings.water_wave_detail_speed
            },
            if fixed_debug {
                0.01
            } else {
                settings.water_reflection_edge_fade
            },
        ),
        ssr_params: Vec4::new(
            if fixed_debug {
                0.0
            } else {
                settings.water_ssr_steps.clamp(4, 64) as f32
            },
            if fixed_debug {
                0.2
            } else {
                settings.water_ssr_thickness.clamp(0.02, 2.0)
            },
            if fixed_debug {
                40.0
            } else {
                settings.water_ssr_max_distance.clamp(4.0, 400.0)
            },
            if fixed_debug {
                1.0
            } else {
                settings.water_ssr_stride.clamp(0.2, 8.0)
            },
        ),
        debug_flags: Vec4::new(
            settings.cutout_debug_mode as f32,
            settings.water_reflection_sky_fill,
            settings.shadow_opacity.clamp(0.0, 1.0),
            shading_model,
        ),
        grass_overlay_info: Vec4::new(f32::NAN, f32::NAN, f32::NAN, f32::NAN),
        reflection_view_proj: Mat4::IDENTITY,
    }
}
