use std::time::Instant;

use bevy::input::mouse::MouseMotion;
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};

use rs_render::{LookAngles, Player, PlayerCamera};
use rs_utils::{AppState, ApplicationState, ToNet, ToNetMessage, UiState};

use crate::net::events::NetEventQueue;
use crate::sim::collision::WorldCollisionMap;
use crate::sim::movement::{simulate_tick, WorldCollision};
use crate::sim::predict::PredictionBuffer;
use crate::sim::reconcile::reconcile;
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use rs_render::{RenderDebugSettings, debug::RenderPerfStats};
use rs_utils::PerfTimings;
use crate::sim::{
    CurrentInput, DebugStats, DebugUiState, PredictedFrame, SimClock, SimState, VisualCorrectionOffset,
};

#[derive(Resource, Default)]
pub struct FrameTimingState {
    pub start: Option<Instant>,
}

#[derive(Resource)]
pub struct PredictionHistory(pub PredictionBuffer);

impl Default for PredictionHistory {
    fn default() -> Self {
        Self(PredictionBuffer::new(512))
    }
}

#[derive(Default, Resource)]
pub struct LatencyEstimate {
    pub one_way_ticks: u32,
    pub last_sent: Option<Instant>,
}

pub fn input_collect_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut motion_events: EventReader<MouseMotion>,
    mut input: ResMut<CurrentInput>,
    app_state: Res<AppState>,
    ui_state: Res<UiState>,
    player_status: Res<rs_utils::PlayerStatus>,
    mut timings: ResMut<PerfTimings>,
) {
    let start = std::time::Instant::now();
    if !matches!(app_state.0, ApplicationState::Connected)
        || ui_state.chat_open
        || ui_state.paused
        || player_status.dead
    {
        motion_events.clear();
        input.0.forward = 0.0;
        input.0.strafe = 0.0;
        input.0.sprint = false;
        input.0.sneak = false;
        input.0.jump = false;
        timings.input_collect_ms = start.elapsed().as_secs_f32() * 1000.0;
        return;
    }

    let mut look_delta = Vec2::ZERO;
    for ev in motion_events.read() {
        look_delta += ev.delta;
    }

    let sensitivity = 0.002;
    // Bevy uses right-handed yaw (positive rotates left), so invert mouse X for FPS feel.
    input.0.yaw -= look_delta.x * sensitivity;
    // Mouse up should look up.
    input.0.pitch -= look_delta.y * sensitivity;
    input.0.pitch = input.0.pitch.clamp(-1.54, 1.54);

    input.0.forward = 0.0;
    input.0.strafe = 0.0;
    if keys.pressed(KeyCode::KeyW) {
        input.0.forward += 1.0;
    }
    if keys.pressed(KeyCode::KeyS) {
        input.0.forward -= 1.0;
    }
    if keys.pressed(KeyCode::KeyD) {
        input.0.strafe += 1.0;
    }
    if keys.pressed(KeyCode::KeyA) {
        input.0.strafe -= 1.0;
    }

    if keys.just_pressed(KeyCode::Space) {
        input.0.jump = true;
    }

    input.0.sprint = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    input.0.sneak = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    timings.input_collect_ms = start.elapsed().as_secs_f32() * 1000.0;
}

pub fn frame_timing_start(
    time: Res<Time>,
    mut state: ResMut<FrameTimingState>,
    mut timings: ResMut<PerfTimings>,
) {
    state.start = Some(Instant::now());
    timings.frame_delta_ms = time.delta_secs() * 1000.0;
}

pub fn frame_timing_end(
    mut state: ResMut<FrameTimingState>,
    mut timings: ResMut<PerfTimings>,
) {
    if let Some(start) = state.start.take() {
        timings.main_thread_ms = start.elapsed().as_secs_f32() * 1000.0;
    }
}

pub fn debug_toggle_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut debug_ui: ResMut<DebugUiState>,
    ui_state: Res<UiState>,
) {
    if ui_state.chat_open {
        return;
    }
    if keys.just_pressed(KeyCode::KeyF) {
        debug_ui.open = !debug_ui.open;
    }
}

pub fn fixed_sim_tick_system(
    mut sim_clock: ResMut<SimClock>,
    mut sim_state: ResMut<SimState>,
    mut input: ResMut<CurrentInput>,
    mut history: ResMut<PredictionHistory>,
    mut latency: ResMut<LatencyEstimate>,
    collision_map: Res<WorldCollisionMap>,
    app_state: Res<AppState>,
    to_net: Res<ToNet>,
    sim_ready: Res<crate::sim::SimReady>,
    mut timings: ResMut<PerfTimings>,
) {
    let start = std::time::Instant::now();
    if !matches!(app_state.0, ApplicationState::Connected) || !sim_ready.0 {
        timings.fixed_tick_ms = start.elapsed().as_secs_f32() * 1000.0;
        return;
    }
    let world = WorldCollision::with_map(&collision_map);
    let tick = sim_clock.tick;
    let input_snapshot = input.0;
    let next_state = simulate_tick(&sim_state.current, &input_snapshot, &world);

    history.0.push(PredictedFrame {
        tick,
        input: input_snapshot,
        state: next_state,
    });

    sim_state.current = next_state;
    sim_clock.tick = sim_clock.tick.wrapping_add(1);
    input.0.jump = false;

    if matches!(app_state.0, ApplicationState::Connected) {
        let pos = sim_state.current.pos;
        let yaw = (std::f32::consts::PI - sim_state.current.yaw).to_degrees();
        let pitch = -sim_state.current.pitch.to_degrees();
        let _ = to_net.0.send(ToNetMessage::PlayerMove {
            x: pos.x as f64,
            y: pos.y as f64,
            z: pos.z as f64,
            yaw,
            pitch,
            on_ground: sim_state.current.on_ground,
        });
        latency.last_sent = Some(Instant::now());
    }
    timings.fixed_tick_ms = start.elapsed().as_secs_f32() * 1000.0;
}

pub fn net_event_apply_system(
    mut net_events: ResMut<NetEventQueue>,
    mut sim_state: ResMut<SimState>,
    mut history: ResMut<PredictionHistory>,
    mut visual_offset: ResMut<VisualCorrectionOffset>,
    mut debug: ResMut<DebugStats>,
    mut latency: ResMut<LatencyEstimate>,
    collision_map: Res<WorldCollisionMap>,
    sim_clock: Res<SimClock>,
    mut sim_ready: ResMut<crate::sim::SimReady>,
    mut timings: ResMut<PerfTimings>,
) {
    let start = std::time::Instant::now();
    let world = WorldCollision::with_map(&collision_map);
    for event in net_events.drain() {
        let (pos, yaw, pitch, on_ground, recv_instant) = match event {
            crate::net::events::NetEvent::ServerPosLook {
                pos,
                yaw,
                pitch,
                on_ground,
                recv_instant,
            } => (pos, yaw, pitch, on_ground, recv_instant),
        };
        if let Some(last_sent) = latency.last_sent {
            let rtt = recv_instant.saturating_duration_since(last_sent);
            let one_way = rtt.as_secs_f32() * 0.5;
            latency.one_way_ticks = (one_way / 0.05).round() as u32;
            debug.one_way_ticks = latency.one_way_ticks;
        }

        let client_tick = sim_clock.tick;
        let server_tick = client_tick.saturating_sub(latency.one_way_ticks);

        let server_state = crate::sim::PlayerSimState {
            pos,
            vel: sim_state.current.vel,
            on_ground,
            yaw,
            pitch,
        };

        if history.0.latest_tick().is_none() {
            sim_state.current = server_state;
            visual_offset.0 = Vec3::ZERO;
            sim_ready.0 = true;
            continue;
        }

        if let Some(result) = reconcile(
            &mut history.0,
            &world,
            server_tick,
            server_state,
            client_tick.saturating_sub(1),
            &mut sim_state.current,
        ) {
            debug.last_correction = result.correction.length();
            debug.last_replay = result.replayed_ticks;
            if result.hard_teleport {
                visual_offset.0 = Vec3::ZERO;
            } else {
                visual_offset.0 += result.correction;
            }
        }
    }
    timings.net_apply_ms = start.elapsed().as_secs_f32() * 1000.0;
}

pub fn visual_smoothing_system(
    time: Res<Time>,
    mut offset: ResMut<VisualCorrectionOffset>,
    mut debug: ResMut<DebugStats>,
    mut timings: ResMut<PerfTimings>,
) {
    let start = std::time::Instant::now();
    let decay = 0.15f32;
    let factor = (1.0 - decay).powf(time.delta_secs() * 20.0);
    offset.0 *= factor;
    debug.smoothing_offset_len = offset.0.length();
    timings.smoothing_ms = start.elapsed().as_secs_f32() * 1000.0;
}

pub fn apply_visual_transform_system(
    sim_state: Res<SimState>,
    offset: Res<VisualCorrectionOffset>,
    mut player_query: Query<(&mut Transform, &mut LookAngles), With<Player>>,
    mut camera_query: Query<&mut Transform, (With<PlayerCamera>, Without<Player>)>,
    mut timings: ResMut<PerfTimings>,
) {
    let start = std::time::Instant::now();
    if let Ok((mut player_transform, mut look)) = player_query.get_single_mut() {
        let pos = sim_state.current.pos + offset.0;
        player_transform.translation = pos;
        look.yaw = sim_state.current.yaw;
        look.pitch = sim_state.current.pitch;
        player_transform.rotation = Quat::from_axis_angle(Vec3::Y, look.yaw);
        if let Ok(mut camera_transform) = camera_query.get_single_mut() {
            camera_transform.rotation = Quat::from_axis_angle(Vec3::X, look.pitch);
        }
    }
    timings.apply_transform_ms = start.elapsed().as_secs_f32() * 1000.0;
}

pub fn debug_overlay_system(
    mut contexts: EguiContexts,
    debug: Res<DebugStats>,
    sim_clock: Res<SimClock>,
    history: Res<PredictionHistory>,
    diagnostics: Res<DiagnosticsStore>,
    time: Res<Time>,
    mut debug_ui: ResMut<DebugUiState>,
    mut render_debug: ResMut<RenderDebugSettings>,
    render_perf: Res<RenderPerfStats>,
    mut timings: ResMut<PerfTimings>,
) {
    let start = std::time::Instant::now();
    if !debug_ui.open {
        timings.debug_ui_ms = 0.0;
        return;
    }
    let ctx = contexts.ctx_mut().unwrap();
    egui::Window::new("Debug")
        .default_pos(egui::pos2(12.0, 12.0))
        .show(ctx, |ui| {
            ui.checkbox(&mut debug_ui.show_prediction, "Prediction");
            ui.checkbox(&mut debug_ui.show_performance, "Performance");
            ui.checkbox(&mut debug_ui.show_render, "Render");

            let frame_ms = (time.delta_secs_f64() * 1000.0) as f32;

            if debug_ui.show_performance {
                ui.separator();
                if let Some(fps) = diagnostics
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
                    timings.main_thread_ms,
                    if frame_ms > 0.0 {
                        format!("{:.1}%", (timings.main_thread_ms / frame_ms) * 100.0)
                    } else {
                        "n/a".to_string()
                    }
                ));
            }

            if debug_ui.show_render {
                ui.separator();
                ui.checkbox(&mut render_debug.shadows_enabled, "Shadows");
                ui.checkbox(&mut render_debug.use_greedy_meshing, "Binary greedy meshing");
                ui.checkbox(&mut render_debug.wireframe_enabled, "Wireframe");
                ui.checkbox(&mut render_debug.manual_frustum_cull, "Manual frustum cull");
                ui.checkbox(&mut render_debug.frustum_fov_debug, "Frustum FOV debug");
                if render_debug.frustum_fov_debug {
                    ui.add(
                        egui::Slider::new(&mut render_debug.frustum_fov_deg, 30.0..=140.0)
                            .text("Frustum FOV"),
                    );
                }
                let mut dist = render_debug.render_distance_chunks as i32;
                if ui
                    .add(egui::Slider::new(&mut dist, 2..=32).text("Render distance"))
                    .changed()
                {
                    render_debug.render_distance_chunks = dist;
                }
                ui.add(egui::Slider::new(&mut render_debug.fov_deg, 60.0..=140.0).text("FOV"));
            }

            if debug_ui.show_prediction {
                ui.separator();
                ui.label(format!("tick: {}", sim_clock.tick));
                ui.label(format!("history cap: {}", history.0.capacity()));
                ui.label(format!("last correction: {:.4}", debug.last_correction));
                ui.label(format!("last replay ticks: {}", debug.last_replay));
                ui.label(format!("smoothing offset: {:.4}", debug.smoothing_offset_len));
                ui.label(format!("one-way ticks: {}", debug.one_way_ticks));
            }

            if debug_ui.show_performance {
                ui.separator();
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
                ui.label(format!(
                    "handle_messages: {:.3}ms {}",
                    timings.handle_messages_ms,
                    fmt_pct(timings.handle_messages_ms)
                ));
                ui.label(format!(
                    "input_collect: {:.3}ms {}",
                    timings.input_collect_ms,
                    fmt_pct(timings.input_collect_ms)
                ));
                ui.label(format!(
                    "net_apply: {:.3}ms {}",
                    timings.net_apply_ms,
                    fmt_pct(timings.net_apply_ms)
                ));
                ui.label(format!(
                    "fixed_tick: {:.3}ms {}",
                    timings.fixed_tick_ms,
                    fmt_pct(timings.fixed_tick_ms)
                ));
                ui.label(format!(
                    "smoothing: {:.3}ms {}",
                    timings.smoothing_ms,
                    fmt_pct(timings.smoothing_ms)
                ));
                ui.label(format!(
                    "apply_transform: {:.3}ms {}",
                    timings.apply_transform_ms,
                    fmt_pct(timings.apply_transform_ms)
                ));
                ui.label(format!(
                    "debug_ui: {:.3}ms {}",
                    timings.debug_ui_ms,
                    fmt_pct(timings.debug_ui_ms)
                ));
                ui.label(format!(
                    "ui: {:.3}ms {}",
                    timings.ui_ms,
                    fmt_pct(timings.ui_ms)
                ));

                ui.label(format!(
                    "mesh build ms: {:.2} (avg {:.2}) [async]",
                    render_perf.last_mesh_build_ms, render_perf.avg_mesh_build_ms
                ));
                ui.label(format!(
                    "mesh apply ms: {:.2} (avg {:.2}) {}",
                    render_perf.last_apply_ms,
                    render_perf.avg_apply_ms,
                    fmt_pct(render_perf.last_apply_ms)
                ));
                ui.label(format!(
                    "mesh enqueue ms: {:.2} (avg {:.2}) {}",
                    render_perf.last_enqueue_ms,
                    render_perf.avg_enqueue_ms,
                    fmt_pct(render_perf.last_enqueue_ms)
                ));
                ui.label(format!(
                    "manual cull ms: {:.2} {}",
                    render_perf.manual_cull_ms,
                    fmt_pct(render_perf.manual_cull_ms)
                ));
                ui.label(format!(
                    "render debug ms: {:.2} {}",
                    render_perf.apply_debug_ms,
                    fmt_pct(render_perf.apply_debug_ms)
                ));
                ui.label(format!(
                    "render stats ms: {:.2} {}",
                    render_perf.gather_stats_ms,
                    fmt_pct(render_perf.gather_stats_ms)
                ));
                ui.label(format!(
                    "mesh applied: {} in_flight: {} updates: {} (raw {})",
                    render_perf.last_meshes_applied,
                    render_perf.in_flight,
                    render_perf.last_updates,
                    render_perf.last_updates_raw
                ));
                ui.label(format!(
                    "meshes: dist {} / {} view {} / {}",
                    render_perf.visible_meshes_distance,
                    render_perf.total_meshes,
                    render_perf.visible_meshes_view,
                    render_perf.total_meshes
                ));
                ui.label(format!(
                    "chunks: {} / {} (distance)",
                    render_perf.visible_chunks, render_perf.total_chunks
                ));
            }
        });
    let elapsed_ms = start.elapsed().as_secs_f32() * 1000.0;
    timings.debug_ui_ms = elapsed_ms;
}
