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
use rs_render::RenderDebugSettings;
use crate::sim::{
    CurrentInput, DebugStats, DebugUiState, PredictedFrame, SimClock, SimState, VisualCorrectionOffset,
};

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
) {
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
) {
    if !matches!(app_state.0, ApplicationState::Connected) || !sim_ready.0 {
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
) {
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
}

pub fn visual_smoothing_system(
    time: Res<Time>,
    mut offset: ResMut<VisualCorrectionOffset>,
    mut debug: ResMut<DebugStats>,
) {
    let decay = 0.15f32;
    let factor = (1.0 - decay).powf(time.delta_secs() * 20.0);
    offset.0 *= factor;
    debug.smoothing_offset_len = offset.0.length();
}

pub fn apply_visual_transform_system(
    sim_state: Res<SimState>,
    offset: Res<VisualCorrectionOffset>,
    mut player_query: Query<(&mut Transform, &mut LookAngles), With<Player>>,
    mut camera_query: Query<&mut Transform, (With<PlayerCamera>, Without<Player>)>,
) {
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
}

pub fn debug_overlay_system(
    mut contexts: EguiContexts,
    debug: Res<DebugStats>,
    sim_clock: Res<SimClock>,
    history: Res<PredictionHistory>,
    diagnostics: Res<DiagnosticsStore>,
    mut debug_ui: ResMut<DebugUiState>,
    mut render_debug: ResMut<RenderDebugSettings>,
) {
    if !debug_ui.open {
        return;
    }
    let ctx = contexts.ctx_mut().unwrap();
    egui::Window::new("Debug")
        .default_pos(egui::pos2(12.0, 12.0))
        .show(ctx, |ui| {
            ui.checkbox(&mut debug_ui.show_prediction, "Prediction");
            ui.checkbox(&mut debug_ui.show_performance, "Performance");
            ui.checkbox(&mut debug_ui.show_render, "Render");

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
            }

            if debug_ui.show_render {
                ui.separator();
                ui.checkbox(&mut render_debug.shadows_enabled, "Shadows");
                ui.checkbox(&mut render_debug.use_greedy_meshing, "Binary greedy meshing");
                let mut dist = render_debug.render_distance_chunks as i32;
                if ui
                    .add(egui::Slider::new(&mut dist, 2..=32).text("Render distance"))
                    .changed()
                {
                    render_debug.render_distance_chunks = dist;
                }
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
        });
}
