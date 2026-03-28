use bevy::core_pipeline::{
    fxaa::{Fxaa, Sensitivity},
    prepass::DepthPrepass,
    smaa::{Smaa, SmaaPreset},
};
use bevy::pbr::ScreenSpaceAmbientOcclusion;
use bevy::prelude::*;
use bevy::render::view::Msaa;

use crate::components::PlayerCamera;
use crate::debug::{AntiAliasingMode, RenderDebugSettings};

pub fn apply_antialiasing(
    settings: Res<RenderDebugSettings>,
    mut camera_query: Query<(Entity, Option<&mut Fxaa>, Option<&mut Smaa>, &mut Msaa), With<PlayerCamera>>,
    mut commands: Commands,
) {
    if !settings.is_changed() {
        return;
    }
    let Ok((camera_entity, fxaa_opt, smaa_opt, mut msaa)) = camera_query.single_mut() else {
        return;
    };

    match settings.aa_mode {
        AntiAliasingMode::Off => {
            *msaa = Msaa::Off;
            if let Some(mut fxaa) = fxaa_opt {
                fxaa.enabled = false;
            }
            if smaa_opt.is_some() {
                commands.entity(camera_entity).remove::<Smaa>();
            }
        }
        AntiAliasingMode::Fxaa => {
            *msaa = Msaa::Off;
            if let Some(mut fxaa) = fxaa_opt {
                fxaa.enabled = true;
                fxaa.edge_threshold = Sensitivity::Ultra;
                fxaa.edge_threshold_min = Sensitivity::High;
            } else {
                commands.entity(camera_entity).insert(Fxaa {
                    enabled: true,
                    edge_threshold: Sensitivity::Ultra,
                    edge_threshold_min: Sensitivity::High,
                });
            }
            if smaa_opt.is_some() {
                commands.entity(camera_entity).remove::<Smaa>();
            }
        }
        AntiAliasingMode::SmaaHigh => {
            *msaa = Msaa::Off;
            if let Some(mut fxaa) = fxaa_opt {
                fxaa.enabled = false;
            }
            if let Some(mut smaa) = smaa_opt {
                smaa.preset = SmaaPreset::High;
            } else {
                commands.entity(camera_entity).insert(Smaa {
                    preset: SmaaPreset::High,
                });
            }
        }
        AntiAliasingMode::SmaaUltra => {
            *msaa = Msaa::Off;
            if let Some(mut fxaa) = fxaa_opt {
                fxaa.enabled = false;
            }
            if let Some(mut smaa) = smaa_opt {
                smaa.preset = SmaaPreset::Ultra;
            } else {
                commands.entity(camera_entity).insert(Smaa {
                    preset: SmaaPreset::Ultra,
                });
            }
        }
        AntiAliasingMode::Msaa4 => {
            *msaa = Msaa::Sample4;
            if let Some(mut fxaa) = fxaa_opt {
                fxaa.enabled = true;
                fxaa.edge_threshold = Sensitivity::High;
                fxaa.edge_threshold_min = Sensitivity::Medium;
            } else {
                commands.entity(camera_entity).insert(Fxaa {
                    enabled: true,
                    edge_threshold: Sensitivity::High,
                    edge_threshold_min: Sensitivity::Medium,
                });
            }
            if smaa_opt.is_some() {
                commands.entity(camera_entity).remove::<Smaa>();
            }
        }
        AntiAliasingMode::Msaa8 => {
            *msaa = Msaa::Sample8;
            if let Some(mut fxaa) = fxaa_opt {
                fxaa.enabled = true;
                fxaa.edge_threshold = Sensitivity::High;
                fxaa.edge_threshold_min = Sensitivity::Medium;
            } else {
                commands.entity(camera_entity).insert(Fxaa {
                    enabled: true,
                    edge_threshold: Sensitivity::High,
                    edge_threshold_min: Sensitivity::Medium,
                });
            }
            if smaa_opt.is_some() {
                commands.entity(camera_entity).remove::<Smaa>();
            }
        }
    }
}

pub fn apply_ssao_quality(
    settings: Res<RenderDebugSettings>,
    camera_query: Query<Entity, With<PlayerCamera>>,
    mut commands: Commands,
) {
    if !settings.is_changed() {
        return;
    }
    let Ok(camera_entity) = camera_query.single() else {
        return;
    };
    let _ = settings;
    commands
        .entity(camera_entity)
        .remove::<ScreenSpaceAmbientOcclusion>();
}

pub fn apply_depth_prepass_for_ssr(
    settings: Res<RenderDebugSettings>,
    camera_query: Query<(Entity, Option<&DepthPrepass>), With<PlayerCamera>>,
    mut commands: Commands,
) {
    let Ok((camera_entity, has_depth_prepass)) = camera_query.single() else {
        return;
    };
    let want_depth_prepass =
        settings.water_reflections_enabled && settings.water_reflection_screen_space;
    match (want_depth_prepass, has_depth_prepass.is_some()) {
        (true, false) => {
            commands.entity(camera_entity).insert(DepthPrepass);
        }
        (false, true) => {
            commands.entity(camera_entity).remove::<DepthPrepass>();
        }
        _ => {}
    }
}
