use super::super::*;
use super::world::raycast_block;
use crate::sim::movement::collision_parity_expected_box_count;
use crate::sim_systems::{PerfMonitorSample, PerformanceMonitorState};
use bevy::ecs::system::SystemParam;

#[derive(SystemParam)]
pub(crate) struct DebugOverlayParams<'w, 's> {
    debug: Res<'w, DebugStats>,
    sim_clock: Res<'w, SimClock>,
    history: Res<'w, PredictionHistory>,
    diagnostics: Res<'w, DiagnosticsStore>,
    time: Res<'w, Time>,
    debug_ui: ResMut<'w, DebugUiState>,
    render_debug: ResMut<'w, RenderDebugSettings>,
    render_perf: Res<'w, RenderPerfStats>,
    sim_state: Res<'w, SimState>,
    input: Res<'w, CurrentInput>,
    ui_state: Res<'w, UiState>,
    player_status: Res<'w, rs_utils::PlayerStatus>,
    monitor: Res<'w, PerformanceMonitorState>,
    timings: ResMut<'w, PerfTimings>,
    _marker: std::marker::PhantomData<&'s ()>,
}

pub fn draw_entity_hitboxes_system(
    mut gizmos: Gizmos,
    settings: Res<EntityHitboxDebug>,
    app_state: Res<AppState>,
    entities: Query<(&GlobalTransform, &RemoteEntity, &RemoteVisual)>,
) {
    if !settings.enabled || !matches!(app_state.0, ApplicationState::Connected) {
        return;
    }

    for (transform, remote, visual) in &entities {
        let (half_w, height) = match remote.kind {
            rs_utils::NetEntityKind::Player | rs_utils::NetEntityKind::Mob(_) => (0.34, 1.8),
            rs_utils::NetEntityKind::Item => (0.22, 0.35),
            rs_utils::NetEntityKind::ExperienceOrb => (0.18, 0.28),
            rs_utils::NetEntityKind::Object(_) => (0.28, 0.56),
        };
        let feet = transform.translation() - Vec3::Y * visual.y_offset;
        let min = Vec3::new(feet.x - half_w, feet.y, feet.z - half_w);
        let max = Vec3::new(feet.x + half_w, feet.y + height, feet.z + half_w);
        draw_aabb_lines(&mut gizmos, min, max, Color::srgba(0.2, 1.0, 0.2, 1.0));
    }
}

pub fn draw_chunk_debug_system(
    mut gizmos: Gizmos,
    render_debug: Res<RenderDebugSettings>,
    app_state: Res<AppState>,
    break_indicator: Res<BreakIndicator>,
    freecam: Res<FreecamState>,
    player_status: Res<rs_utils::PlayerStatus>,
    collision_map: Res<WorldCollisionMap>,
    chunks: Query<(&ChunkRoot, &Visibility)>,
    camera: Query<&GlobalTransform, With<PlayerCamera>>,
) {
    if !matches!(app_state.0, ApplicationState::Connected) {
        return;
    }

    if render_debug.show_chunk_borders {
        let color = Color::srgba(0.25, 0.75, 1.0, 1.0);
        for (chunk, vis) in &chunks {
            if matches!(*vis, Visibility::Hidden) {
                continue;
            }
            let base = Vec3::new(chunk.key.0 as f32 * 16.0, 0.0, chunk.key.1 as f32 * 16.0);
            let min = base;
            let max = base + Vec3::new(16.0, 256.0, 16.0);
            draw_aabb_lines(&mut gizmos, min, max, color);
        }
    }

    if render_debug.show_look_ray {
        let Ok(cam) = camera.get_single() else {
            return;
        };
        let origin = cam.translation();
        let dir = *cam.forward();
        let end = origin + dir * 3.0;
        gizmos.line(origin, end, Color::srgba(1.0, 0.2, 0.2, 1.0));
    }

    if render_debug.show_target_block_outline && !freecam.active {
        let Ok(cam) = camera.get_single() else {
            return;
        };
        let origin = cam.translation();
        let dir = *cam.forward();
        let max_reach = if player_status.gamemode == 1 {
            CREATIVE_BLOCK_REACH
        } else {
            SURVIVAL_BLOCK_REACH
        };
        if let Some(hit) = raycast_block(&collision_map, origin, dir, max_reach) {
            let state = collision_map.block_at(hit.block.x, hit.block.y, hit.block.z);
            let world = WorldCollision::with_map(&collision_map);
            let boxes = target_outline_boxes(&world, state, hit.block.x, hit.block.y, hit.block.z);
            let inflate = 0.0025;
            for (mut min, mut max) in boxes.iter().copied() {
                min -= Vec3::splat(inflate);
                max += Vec3::splat(inflate);
                draw_aabb_lines(&mut gizmos, min, max, Color::srgba(0.02, 0.02, 0.02, 1.0));
            }
            if break_indicator.active {
                let p = break_indicator.progress.clamp(0.0, 1.0);
                let crack_color = Color::srgb(0.32 + 0.50 * p, 0.32 - 0.20 * p, 0.32 - 0.20 * p);
                let crack_inflate = 0.008 + p * 0.010;
                for (min, max) in boxes {
                    draw_aabb_lines(
                        &mut gizmos,
                        min - Vec3::splat(crack_inflate),
                        max + Vec3::splat(crack_inflate),
                        crack_color,
                    );
                }
            }
        }
    }
}

fn target_outline_boxes(
    world: &WorldCollision,
    block_state: u16,
    block_x: i32,
    block_y: i32,
    block_z: i32,
) -> Vec<(Vec3, Vec3)> {
    let mut boxes = debug_block_collision_boxes(world, block_state, block_x, block_y, block_z);
    if !boxes.is_empty() {
        return boxes;
    }

    let min = Vec3::new(block_x as f32, block_y as f32, block_z as f32);
    let max = min + Vec3::ONE;
    let id = block_state_id(block_state);

    let fallback = match block_model_kind(id) {
        rs_utils::BlockModelKind::Cross => {
            let h = if id == 175 { 1.0 } else { 0.875 };
            Some((min + Vec3::new(0.1, 0.0, 0.1), min + Vec3::new(0.9, h, 0.9)))
        }
        rs_utils::BlockModelKind::TorchLike => Some((
            min + Vec3::new(0.4, 0.0, 0.4),
            min + Vec3::new(0.6, 0.75, 0.6),
        )),
        rs_utils::BlockModelKind::Custom => match id {
            26 => Some((min, min + Vec3::new(1.0, 9.0 / 16.0, 1.0))), // bed
            27 | 28 | 66 | 157 | 171 => Some((min, min + Vec3::new(1.0, 1.0 / 16.0, 1.0))), // rails/carpet
            78 => {
                let h = ((block_state_meta(block_state) & 0x7) as f32 + 1.0) / 8.0;
                Some((min, min + Vec3::new(1.0, h.clamp(0.125, 1.0), 1.0)))
            }
            _ => Some((min, max)),
        },
        _ => Some((min, max)),
    };

    if let Some(bb) = fallback {
        boxes.push(bb);
    }
    boxes
}

fn draw_aabb_lines(gizmos: &mut Gizmos, min: Vec3, max: Vec3, color: Color) {
    let p000 = Vec3::new(min.x, min.y, min.z);
    let p001 = Vec3::new(min.x, min.y, max.z);
    let p010 = Vec3::new(min.x, max.y, min.z);
    let p011 = Vec3::new(min.x, max.y, max.z);
    let p100 = Vec3::new(max.x, min.y, min.z);
    let p101 = Vec3::new(max.x, min.y, max.z);
    let p110 = Vec3::new(max.x, max.y, min.z);
    let p111 = Vec3::new(max.x, max.y, max.z);

    gizmos.line(p000, p001, color);
    gizmos.line(p001, p011, color);
    gizmos.line(p011, p010, color);
    gizmos.line(p010, p000, color);

    gizmos.line(p100, p101, color);
    gizmos.line(p101, p111, color);
    gizmos.line(p111, p110, color);
    gizmos.line(p110, p100, color);

    gizmos.line(p000, p100, color);
    gizmos.line(p001, p101, color);
    gizmos.line(p010, p110, color);
    gizmos.line(p011, p111, color);
}

fn ray_aabb_distance(
    origin: Vec3,
    dir: Vec3,
    min: Vec3,
    max: Vec3,
    max_distance: f32,
) -> Option<f32> {
    let mut t_min = 0.0f32;
    let mut t_max = max_distance;

    for axis in 0..3 {
        let (origin_axis, dir_axis, min_axis, max_axis) = match axis {
            0 => (origin.x, dir.x, min.x, max.x),
            1 => (origin.y, dir.y, min.y, max.y),
            _ => (origin.z, dir.z, min.z, max.z),
        };

        if dir_axis.abs() <= f32::EPSILON {
            if origin_axis < min_axis || origin_axis > max_axis {
                return None;
            }
            continue;
        }

        let inv = 1.0 / dir_axis;
        let mut t1 = (min_axis - origin_axis) * inv;
        let mut t2 = (max_axis - origin_axis) * inv;
        if t1 > t2 {
            std::mem::swap(&mut t1, &mut t2);
        }

        t_min = t_min.max(t1);
        t_max = t_max.min(t2);
        if t_max < t_min {
            return None;
        }
    }

    if t_min <= max_distance {
        Some(t_min.max(0.0))
    } else {
        None
    }
}

fn normal_to_face_index(normal: IVec3) -> u8 {
    match (normal.x, normal.y, normal.z) {
        (0, -1, 0) => 0, // bottom
        (0, 1, 0) => 1,  // top
        (0, 0, -1) => 2, // north
        (0, 0, 1) => 3,  // south
        (-1, 0, 0) => 4, // west
        (1, 0, 0) => 5,  // east
        _ => 1,
    }
}

fn yaw_deg_to_cardinal(yaw_deg_mc: f32) -> (&'static str, &'static str) {
    // Minecraft 1.8 yaw: 0 = South (+Z), 90 = West (-X), 180 = North (-Z), 270 = East (+X).
    let yaw = yaw_deg_mc.rem_euclid(360.0);
    let idx = ((yaw / 90.0).round() as i32).rem_euclid(4);
    match idx {
        0 => ("South", "+Z"),
        1 => ("West", "-X"),
        2 => ("North", "-Z"),
        _ => ("East", "+X"),
    }
}

fn block_model_kind_label(kind: rs_utils::BlockModelKind) -> &'static str {
    match kind {
        rs_utils::BlockModelKind::FullCube => "FullCube",
        rs_utils::BlockModelKind::Cross => "Cross",
        rs_utils::BlockModelKind::Slab => "Slab",
        rs_utils::BlockModelKind::Stairs => "Stairs",
        rs_utils::BlockModelKind::Fence => "Fence",
        rs_utils::BlockModelKind::Pane => "Pane",
        rs_utils::BlockModelKind::Fluid => "Fluid",
        rs_utils::BlockModelKind::TorchLike => "TorchLike",
        rs_utils::BlockModelKind::Custom => "Custom",
    }
}

pub fn debug_overlay_system(
    mut contexts: EguiContexts,
    mut params: DebugOverlayParams,
    collision_map: Res<WorldCollisionMap>,
    camera_query: Query<&GlobalTransform, With<PlayerCamera>>,
) {
    let timer = Timing::start();
    if params.ui_state.ui_hidden {
        params.timings.debug_ui_ms = 0.0;
        return;
    }
    let ctx = contexts.ctx_mut().unwrap();
    if params.debug_ui.perf_monitor_open {
        draw_performance_monitor(ctx, &params.monitor, params.debug_ui.perf_monitor_compact);
    }
    if !params.debug_ui.open {
        params.timings.debug_ui_ms = timer.ms();
        return;
    }
    egui::Window::new("Debug")
        .default_pos(egui::pos2(12.0, 12.0))
        .show(ctx, |ui| {
            let frame_ms = (params.time.delta_secs_f64() * 1000.0) as f32;
            let performance_section = egui::CollapsingHeader::new("Performance")
                .default_open(params.debug_ui.show_performance)
                .show(ui, |ui| {
                    ui.separator();
                    ui.label("Hotkeys: F6 monitor, F7 compact");
                    if let Some(fps) = params
                        .diagnostics
                        .get(&FrameTimeDiagnosticsPlugin::FPS)
                        .and_then(|d| d.smoothed())
                    {
                        ui.label(format!("fps: {:.1}", fps));
                    } else {
                        ui.label("fps: n/a");
                    }
                    ui.label(format!("frame ms (delta): {:.2}", frame_ms));
                    ui.label(format!(
                        "main thread ms: {:.2} {}",
                        params.timings.main_thread_ms,
                        if frame_ms > 0.0 {
                            format!("{:.1}%", (params.timings.main_thread_ms / frame_ms) * 100.0)
                        } else {
                            "n/a".to_string()
                        }
                    ));
                });
            params.debug_ui.show_performance = performance_section.fully_open();

            let render_section = egui::CollapsingHeader::new("Render")
                .default_open(params.debug_ui.show_render)
                .show(ui, |ui| {
                    ui.separator();
                    let layers_section = egui::CollapsingHeader::new("Layers")
                        .default_open(params.debug_ui.render_show_layers)
                        .show(ui, |ui| {
                            ui.separator();
                            ui.checkbox(
                                &mut params.render_debug.show_layer_entities,
                                "Layer: entities",
                            );
                            ui.checkbox(
                                &mut params.render_debug.show_layer_chunks_opaque,
                                "Layer: chunks opaque",
                            );
                            ui.checkbox(
                                &mut params.render_debug.show_layer_chunks_cutout,
                                "Layer: chunks cutout",
                            );
                            ui.checkbox(
                                &mut params.render_debug.show_layer_chunks_transparent,
                                "Layer: chunks transparent",
                            );
                        });
                    params.debug_ui.render_show_layers = layers_section.fully_open();
                    ui.separator();
                    ui.checkbox(&mut params.render_debug.show_coordinates, "Coordinates");
                    ui.checkbox(&mut params.render_debug.show_look_info, "Look info");
                    ui.checkbox(&mut params.render_debug.show_look_ray, "Look ray");
                    ui.checkbox(
                        &mut params.render_debug.show_target_block_outline,
                        "Target block outline",
                    );

                    if params.render_debug.show_coordinates || params.render_debug.show_look_info {
                        ui.separator();
                    }
                    if params.render_debug.show_coordinates {
                        let pos = params.sim_state.current.pos;
                        let block = pos.floor().as_ivec3();
                        let chunk_x = block.x.div_euclid(16);
                        let chunk_z = block.z.div_euclid(16);
                        ui.label(format!("pos: {:.3} {:.3} {:.3}", pos.x, pos.y, pos.z));
                        ui.label(format!("block: {} {} {}", block.x, block.y, block.z));
                        ui.label(format!("chunk: {} {}", chunk_x, chunk_z));
                    }
                    if params.render_debug.show_look_info {
                        let yaw_mc = (std::f32::consts::PI - params.input.0.yaw).to_degrees();
                        let pitch_mc = (-params.input.0.pitch).to_degrees();
                        let (card, axis) = yaw_deg_to_cardinal(yaw_mc);
                        ui.label(format!("yaw/pitch: {:.1} / {:.1}", yaw_mc, pitch_mc));
                        ui.label(format!("facing: {} ({})", card, axis));

                        if let Ok(camera_transform) = camera_query.get_single() {
                            let origin = camera_transform.translation();
                            let dir = *camera_transform.forward();
                            let max_reach = if params.player_status.gamemode == 1 {
                                CREATIVE_BLOCK_REACH
                            } else {
                                SURVIVAL_BLOCK_REACH
                            };
                            if let Some(hit) = raycast_block(&collision_map, origin, dir, max_reach)
                            {
                                let state =
                                    collision_map.block_at(hit.block.x, hit.block.y, hit.block.z);
                                let id = block_state_id(state);
                                let meta = block_state_meta(state);
                                let kind = block_model_kind(id);
                                let reg = block_registry_key(id).unwrap_or("minecraft:unknown");
                                let world = WorldCollision::with_map(&collision_map);
                                let boxes = debug_block_collision_boxes(
                                    &world,
                                    state,
                                    hit.block.x,
                                    hit.block.y,
                                    hit.block.z,
                                );

                                ui.label(format!(
                                    "target block: {} {} {}",
                                    hit.block.x, hit.block.y, hit.block.z
                                ));
                                ui.label(format!(
                                    "id/state/meta: {} / {} / {}  kind: {}",
                                    id,
                                    state,
                                    meta,
                                    block_model_kind_label(kind)
                                ));
                                ui.label(format!("registry: {}", reg));
                                ui.label(format!("collision boxes: {}", boxes.len()));
                                if let Some(expected) = collision_parity_expected_box_count(
                                    &world,
                                    state,
                                    hit.block.x,
                                    hit.block.y,
                                    hit.block.z,
                                ) {
                                    let parity = if boxes.len() == expected {
                                        "ok"
                                    } else {
                                        "mismatch"
                                    };
                                    ui.label(format!(
                                        "collision parity: {} (expected {}, actual {})",
                                        parity,
                                        expected,
                                        boxes.len()
                                    ));
                                }
                                for (idx, (min, max)) in boxes.iter().take(4).enumerate() {
                                    let size = *max - *min;
                                    ui.label(format!(
                                        "box{} min({:.3},{:.3},{:.3}) size({:.3},{:.3},{:.3})",
                                        idx, min.x, min.y, min.z, size.x, size.y, size.z
                                    ));
                                }
                                if boxes.len() > 4 {
                                    ui.label(format!("... {} more boxes", boxes.len() - 4));
                                }
                            } else {
                                ui.label("target block: none");
                            }
                        }
                    }
                });
            params.debug_ui.show_render = render_section.fully_open();

            let prediction_section = egui::CollapsingHeader::new("Prediction")
                .default_open(params.debug_ui.show_prediction)
                .show(ui, |ui| {
                    ui.separator();
                    ui.label(format!("tick: {}", params.sim_clock.tick));
                    ui.label(format!("history cap: {}", params.history.0.capacity()));
                    ui.label(format!(
                        "last correction: {:.4}",
                        params.debug.last_correction
                    ));
                    ui.label(format!("last replay ticks: {}", params.debug.last_replay));
                    ui.label(format!(
                        "last velocity correction: {:.4}",
                        params.debug.last_velocity_correction
                    ));
                    ui.label(format!(
                        "last server tick: {}",
                        params
                            .debug
                            .last_reconciled_server_tick
                            .map(|tick| tick.to_string())
                            .unwrap_or_else(|| "n/a".to_string())
                    ));
                    ui.label(format!(
                        "smoothing offset: {:.4}",
                        params.debug.smoothing_offset_len
                    ));
                    ui.label(format!("one-way ticks: {}", params.debug.one_way_ticks));
                });
            params.debug_ui.show_prediction = prediction_section.fully_open();

            if params.debug_ui.show_performance {
                ui.separator();
                let schedule_section = egui::CollapsingHeader::new("Schedule Timings")
                    .default_open(params.debug_ui.perf_show_schedules)
                    .show(ui, |_ui| {});
                params.debug_ui.perf_show_schedules = schedule_section.fully_open();
                let render_stats_section = egui::CollapsingHeader::new("Render Timings")
                    .default_open(params.debug_ui.perf_show_render_stats)
                    .show(ui, |_ui| {});
                params.debug_ui.perf_show_render_stats = render_stats_section.fully_open();
                let pct = |ms: f32| {
                    if frame_ms <= 0.0 {
                        None
                    } else {
                        Some((ms / frame_ms).max(0.0) * 100.0)
                    }
                };
                let fmt_pct = |ms: f32| {
                    pct(ms)
                        .map(|p| format!("{:.1}%", p))
                        .unwrap_or_else(|| "n/a".to_string())
                };
                if params.debug_ui.perf_show_schedules {
                    ui.label(format!(
                        "handle_messages: {:.3}ms {}",
                        params.timings.handle_messages_ms,
                        fmt_pct(params.timings.handle_messages_ms)
                    ));
                    ui.label(format!(
                        "update schedule: {:.3}ms {}",
                        params.timings.update_ms,
                        fmt_pct(params.timings.update_ms)
                    ));
                    ui.label(format!(
                        "post update: {:.3}ms {}",
                        params.timings.post_update_ms,
                        fmt_pct(params.timings.post_update_ms)
                    ));
                    ui.label(format!(
                        "fixed update: {:.3}ms {}",
                        params.timings.fixed_update_ms,
                        fmt_pct(params.timings.fixed_update_ms)
                    ));
                    ui.label(format!(
                        "input_collect: {:.3}ms {}",
                        params.timings.input_collect_ms,
                        fmt_pct(params.timings.input_collect_ms)
                    ));
                    ui.label(format!(
                        "net_apply: {:.3}ms {}",
                        params.timings.net_apply_ms,
                        fmt_pct(params.timings.net_apply_ms)
                    ));
                    ui.label(format!(
                        "fixed_tick: {:.3}ms {}",
                        params.timings.fixed_tick_ms,
                        fmt_pct(params.timings.fixed_tick_ms)
                    ));
                    ui.label(format!(
                        "smoothing: {:.3}ms {}",
                        params.timings.smoothing_ms,
                        fmt_pct(params.timings.smoothing_ms)
                    ));
                    ui.label(format!(
                        "apply_transform: {:.3}ms {}",
                        params.timings.apply_transform_ms,
                        fmt_pct(params.timings.apply_transform_ms)
                    ));
                    ui.label(format!(
                        "debug_ui: {:.3}ms {}",
                        params.timings.debug_ui_ms,
                        fmt_pct(params.timings.debug_ui_ms)
                    ));
                    ui.label(format!(
                        "ui: {:.3}ms {}",
                        params.timings.ui_ms,
                        fmt_pct(params.timings.ui_ms)
                    ));
                }
                if params.debug_ui.perf_show_render_stats {
                    ui.separator();
                    ui.label(format!(
                        "mesh build ms: {:.2} (avg {:.2}) [async]",
                        params.render_perf.last_mesh_build_ms, params.render_perf.avg_mesh_build_ms
                    ));
                    ui.label(format!(
                        "mesh apply ms: {:.2} (avg {:.2}) {}",
                        params.render_perf.last_apply_ms,
                        params.render_perf.avg_apply_ms,
                        fmt_pct(params.render_perf.last_apply_ms)
                    ));
                    ui.label(format!(
                        "mesh enqueue ms: {:.2} (avg {:.2}) {}",
                        params.render_perf.last_enqueue_ms,
                        params.render_perf.avg_enqueue_ms,
                        fmt_pct(params.render_perf.last_enqueue_ms)
                    ));
                    ui.label(format!(
                        "occlusion cull ms: {:.2} {}",
                        params.render_perf.occlusion_cull_ms,
                        fmt_pct(params.render_perf.occlusion_cull_ms)
                    ));
                    ui.label(format!(
                        "render debug ms: {:.2} {}",
                        params.render_perf.apply_debug_ms,
                        fmt_pct(params.render_perf.apply_debug_ms)
                    ));
                    ui.label(format!(
                        "render stats ms: {:.2} {}",
                        params.render_perf.gather_stats_ms,
                        fmt_pct(params.render_perf.gather_stats_ms)
                    ));
                    ui.label(format!(
                        "mesh applied: {} in_flight: {} updates: {} (raw {})",
                        params.render_perf.last_meshes_applied,
                        params.render_perf.in_flight,
                        params.render_perf.last_updates,
                        params.render_perf.last_updates_raw
                    ));
                    ui.label(format!(
                        "meshes: dist {} / {} view {} / {}",
                        params.render_perf.visible_meshes_distance,
                        params.render_perf.total_meshes,
                        params.render_perf.visible_meshes_view,
                        params.render_perf.total_meshes
                    ));
                    ui.label(format!(
                        "chunks: {} / {} (distance)",
                        params.render_perf.visible_chunks, params.render_perf.total_chunks
                    ));
                    ui.label(format!(
                        "chunks after occlusion: {} (occluded {})",
                        params.render_perf.visible_chunks_after_occlusion,
                        params.render_perf.occluded_chunks
                    ));
                    ui.separator();
                    ui.label(format!(
                        "mat pass w: o={:.1} c={:.1} cc={:.1} t={:.1}",
                        params.render_perf.mat_pass_opaque,
                        params.render_perf.mat_pass_cutout,
                        params.render_perf.mat_pass_cutout_culled,
                        params.render_perf.mat_pass_transparent
                    ));
                    ui.label(format!(
                        "mat alpha: o={} c={} cc={} t={}",
                        params.render_perf.mat_alpha_opaque,
                        params.render_perf.mat_alpha_cutout,
                        params.render_perf.mat_alpha_cutout_culled,
                        params.render_perf.mat_alpha_transparent
                    ));
                    ui.label(format!(
                        "mat unlit: o={} c={} cc={} t={}",
                        params.render_perf.mat_unlit_opaque,
                        params.render_perf.mat_unlit_cutout,
                        params.render_perf.mat_unlit_cutout_culled,
                        params.render_perf.mat_unlit_transparent
                    ));
                }
            }
        });

    let elapsed_ms = timer.ms();
    params.timings.debug_ui_ms = elapsed_ms;
}

fn draw_performance_monitor(ctx: &egui::Context, monitor: &PerformanceMonitorState, compact: bool) {
    let Some(latest) = monitor.samples.back().copied() else {
        return;
    };
    let graph_size = if compact {
        egui::vec2(150.0, 44.0)
    } else {
        egui::vec2(220.0, 72.0)
    };
    let margin = if compact { 8 } else { 10 };
    let spacing = if compact { 6.0 } else { 10.0 };

    egui::Area::new(egui::Id::new("performance_monitor_overlay"))
        .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-12.0, 12.0))
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(egui::Color32::from_black_alpha(176))
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(70)))
                .corner_radius(egui::CornerRadius::same(4))
                .inner_margin(egui::Margin::same(margin))
                .show(ui, |ui| {
                    if !compact {
                        ui.horizontal(|ui| {
                            ui.strong("Performance Monitor");
                            ui.label("F6 toggle, F7 compact");
                        });
                        ui.add_space(4.0);
                    }
                    draw_monitor_row(
                        ui,
                        "CPU",
                        latest.cpu_percent,
                        "%",
                        graph_size,
                        egui::Color32::from_rgb(244, 117, 96),
                        &monitor.samples,
                        |sample| sample.cpu_percent,
                        100.0,
                    );
                    ui.add_space(spacing);
                    draw_monitor_row(
                        ui,
                        "GPU / Render",
                        latest.render_ms,
                        "ms",
                        graph_size,
                        egui::Color32::from_rgb(114, 182, 255),
                        &monitor.samples,
                        |sample| sample.render_ms,
                        16.67,
                    );
                    ui.add_space(spacing);
                    draw_monitor_row(
                        ui,
                        "RAM",
                        latest.ram_mb,
                        "MB",
                        graph_size,
                        egui::Color32::from_rgb(128, 211, 120),
                        &monitor.samples,
                        |sample| sample.ram_mb,
                        latest.ram_mb.max(256.0),
                    );
                    if !compact {
                        ui.add_space(6.0);
                        ui.label(format!("frame {:.2} ms", latest.frame_ms));
                    }
                });
        });
}

fn draw_monitor_row(
    ui: &mut egui::Ui,
    label: &str,
    latest: f32,
    unit: &str,
    graph_size: egui::Vec2,
    color: egui::Color32,
    samples: &std::collections::VecDeque<PerfMonitorSample>,
    value_of: impl Fn(&PerfMonitorSample) -> f32,
    baseline_max: f32,
) {
    ui.label(format!("{label} {:.1}{unit}", latest));
    let (rect, _) = ui.allocate_exact_size(graph_size, egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 2.0, egui::Color32::from_black_alpha(84));
    painter.rect_stroke(
        rect,
        2.0,
        egui::Stroke::new(1.0, egui::Color32::from_gray(60)),
        egui::StrokeKind::Outside,
    );
    if samples.len() < 2 {
        return;
    }

    let max_value = samples
        .iter()
        .map(&value_of)
        .fold(baseline_max.max(1.0), f32::max)
        .max(1.0);
    let width = rect.width().max(1.0);
    let height = rect.height().max(1.0);
    let count = (samples.len() - 1).max(1) as f32;
    let mut points = Vec::with_capacity(samples.len());
    for (idx, sample) in samples.iter().enumerate() {
        let x = rect.left() + width * (idx as f32 / count);
        let normalized = (value_of(sample) / max_value).clamp(0.0, 1.0);
        let y = rect.bottom() - normalized * height;
        points.push(egui::pos2(x, y));
    }

    let baseline_y = rect.bottom() - ((baseline_max / max_value).clamp(0.0, 1.0) * height);
    painter.line_segment(
        [
            egui::pos2(rect.left(), baseline_y),
            egui::pos2(rect.right(), baseline_y),
        ],
        egui::Stroke::new(1.0, egui::Color32::from_gray(45)),
    );
    painter.add(egui::Shape::line(points, egui::Stroke::new(1.5, color)));
}
