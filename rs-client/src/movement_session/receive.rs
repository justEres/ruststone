use std::time::Instant;

use bevy::prelude::*;

use crate::movement_session::planner::{estimate_server_tick, wrap_degrees};
use crate::movement_session::state::{
    MovementPhase, MovementSession, TELEPORT_COMMIT_HOLD_TICKS, ServerCorrection,
};
use crate::net::events::{NetEvent, NetEventQueue};
use crate::sim::collision::WorldCollisionMap;
use crate::sim::movement::WorldCollision;
use crate::sim::reconcile::reconcile;
use crate::sim::{DebugStats, SimClock, SimReady, SimRenderState, SimState, VisualCorrectionOffset};
use crate::sim_systems::{LatencyEstimate, PredictionHistory};
use crate::timing::Timing;
use rs_utils::{PerfTimings, ToNet, ToNetMessage};
use tracing::debug;

pub fn movement_session_receive_system(
    mut net_events: ResMut<NetEventQueue>,
    mut session: ResMut<MovementSession>,
    mut sim_render: ResMut<SimRenderState>,
    mut sim_state: ResMut<SimState>,
    mut history: ResMut<PredictionHistory>,
    mut visual_offset: ResMut<VisualCorrectionOffset>,
    mut debug: ResMut<DebugStats>,
    mut latency: ResMut<LatencyEstimate>,
    mut sim_ready: ResMut<SimReady>,
    collision_map: Res<WorldCollisionMap>,
    sim_clock: Res<SimClock>,
    to_net: Res<ToNet>,
    mut timings: ResMut<PerfTimings>,
) {
    let timer = Timing::start();
    let world = WorldCollision::with_map(&collision_map);
    for event in net_events.drain() {
        match event {
            NetEvent::ServerPosLook {
                pos,
                ack_pos,
                yaw,
                pitch,
                flags,
                on_ground,
                on_ground_known,
                recv_instant,
            } => {
                if let Some(last_sent) = latency.last_sent {
                    let rtt = recv_instant.saturating_duration_since(last_sent);
                    let one_way = rtt.as_secs_f32() * 0.5;
                    latency.one_way_ticks = (one_way / 0.05).round() as u32;
                    debug.one_way_ticks = latency.one_way_ticks;
                    debug.last_rtt_ms = rtt.as_secs_f32() * 1000.0;
                    debug.last_one_way_ms = one_way * 1000.0;
                }

                let correction = ServerCorrection {
                    sim_pos: pos,
                    ack_pos,
                    sim_yaw: yaw,
                    sim_pitch: pitch,
                    packet_yaw_deg: wrap_degrees((std::f32::consts::PI - yaw).to_degrees()),
                    packet_pitch_deg: (-pitch.to_degrees()).clamp(-90.0, 90.0),
                    on_ground,
                    on_ground_known,
                    recv_instant,
                    recv_sim_tick: sim_clock.tick,
                };
                let mut corrected_velocity = sim_state.current.vel;
                if (flags & 0x01) == 0 {
                    corrected_velocity.x = 0.0;
                }
                if (flags & 0x02) == 0 {
                    corrected_velocity.y = 0.0;
                }
                if (flags & 0x04) == 0 {
                    corrected_velocity.z = 0.0;
                }

                let server_state = crate::sim::PlayerSimState {
                    pos,
                    vel: corrected_velocity,
                    on_ground,
                    collided_horizontally: false,
                    jump_ticks: 0,
                    yaw,
                    pitch,
                };
                let correction_delta = pos - sim_state.current.pos;
                session.begin_correction(correction);
                let _ = to_net.0.send(ToNetMessage::MovementEpochBarrier {
                    epoch: session.movement_epoch,
                });
                let ack_packet = session.make_ack_packet(correction);
                let tick = sim_clock.tick;
                let _ = to_net.0.send(ToNetMessage::PlayerMovePosLook {
                    epoch: session.movement_epoch,
                    x: ack_packet.pos_f64.0,
                    y: ack_packet.pos_f64.1,
                    z: ack_packet.pos_f64.2,
                    yaw: ack_packet.yaw,
                    pitch: ack_packet.pitch,
                    on_ground: ack_packet.on_ground,
                });
                session.phase_ticks_remaining = TELEPORT_COMMIT_HOLD_TICKS;
                session.transition_to(
                    MovementPhase::AwaitingTeleportCommit,
                    "teleport ack sent immediately",
                );
                session.record_packet(tick, ack_packet);
                latency.last_sent = Some(Instant::now());
                sim_render.previous = server_state;
                sim_state.current = server_state;
                history.0 = PredictionHistory::default().0;
                visual_offset.0 = Vec3::ZERO;
                debug.last_correction = correction_delta.length();
                debug.last_replay = 0;
                debug.last_velocity_correction = 0.0;
                debug.last_reconciled_server_tick = None;
                debug!(
                    tick,
                    flags,
                    vel_x = corrected_velocity.x,
                    vel_y = corrected_velocity.y,
                    vel_z = corrected_velocity.z,
                    "movement-session: applied vanilla-style correction velocity"
                );
                sim_ready.0 = true;
            }
            NetEvent::ServerVelocity {
                velocity,
                recv_instant,
            } => {
                if let Some(last_sent) = latency.last_sent {
                    let rtt = recv_instant.saturating_duration_since(last_sent);
                    let one_way = rtt.as_secs_f32() * 0.5;
                    latency.one_way_ticks = (one_way / 0.05).round() as u32;
                    debug.one_way_ticks = latency.one_way_ticks;
                    debug.last_rtt_ms = rtt.as_secs_f32() * 1000.0;
                    debug.last_one_way_ms = one_way * 1000.0;
                }

                let latest_tick = sim_clock.tick.saturating_sub(1);
                let estimated_tick = latest_tick.saturating_sub(latency.one_way_ticks);
                let mut authoritative_state = history
                    .0
                    .get_by_tick(estimated_tick)
                    .map(|frame| frame.state)
                    .or_else(|| history.0.latest_frame().map(|frame| frame.state))
                    .unwrap_or(sim_state.current);
                authoritative_state.vel = velocity;
                let (server_tick, alignment_delta) = estimate_server_tick(
                    &history,
                    latest_tick,
                    estimated_tick,
                    &authoritative_state,
                );
                debug.last_tick_alignment_delta = alignment_delta;

                let previous_state = sim_state.current;
                if let Some(result) = reconcile(
                    &mut history.0,
                    &world,
                    server_tick,
                    authoritative_state,
                    latest_tick,
                    &mut sim_state.current,
                ) {
                    sim_render.previous = previous_state;
                    visual_offset.0 += previous_state.pos - sim_state.current.pos;
                    debug.last_correction = result.correction.length();
                    debug.last_replay = result.replayed_ticks;
                    debug.last_velocity_correction = result.velocity_correction;
                    debug.last_reconciled_server_tick = Some(server_tick);
                } else {
                    sim_state.current.vel = velocity;
                    if let Some(frame) = history.0.latest_frame_mut() {
                        frame.state = sim_state.current;
                    }
                    debug.last_velocity_correction = 0.0;
                    debug.last_reconciled_server_tick = Some(server_tick);
                }
            }
        }
    }

    timings.net_apply_ms = timer.ms();
}
