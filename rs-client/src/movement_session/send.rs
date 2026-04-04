use std::time::Instant;

use bevy::prelude::*;

use crate::movement_session::planner::{has_loaded_player_chunk, wrap_degrees};
use crate::movement_session::state::{
    MovementObservation, MovementPacketKind, MovementSession,
};
use crate::sim::collision::WorldCollisionMap;
use crate::sim::{SimClock, SimReady, SimState};
use crate::sim_systems::LatencyEstimate;
use rs_utils::{AppState, ApplicationState, ToNet, ToNetMessage};
use tracing::debug;

pub fn movement_session_send_system(
    app_state: Res<AppState>,
    sim_ready: Res<SimReady>,
    sim_clock: Res<SimClock>,
    sim_state: Res<SimState>,
    collision_map: Res<WorldCollisionMap>,
    to_net: Res<ToNet>,
    mut latency: ResMut<LatencyEstimate>,
    mut session: ResMut<MovementSession>,
) {
    if !matches!(app_state.0, ApplicationState::Connected) || !sim_ready.0 {
        session.reset_runtime();
        return;
    }

    let tick = sim_clock.tick.saturating_sub(1);
    let mut yaw = wrap_degrees((std::f32::consts::PI - sim_state.current.yaw).to_degrees());
    let mut pitch = -sim_state.current.pitch.to_degrees();
    if !yaw.is_finite() {
        yaw = 0.0;
    }
    if !pitch.is_finite() {
        pitch = 0.0;
    }
    pitch = pitch.clamp(-90.0, 90.0);

    let obs = MovementObservation {
        pos: sim_state.current.pos,
        yaw,
        pitch,
        on_ground: sim_state.current.on_ground,
    };
    let chunk_loaded = has_loaded_player_chunk(&collision_map, sim_state.current.pos);
    let Some(packet) = session.plan_movement_packet(tick, obs, chunk_loaded) else {
        return;
    };

    match packet.kind {
        MovementPacketKind::Ground => {
            let _ = to_net.0.send(ToNetMessage::PlayerMoveGround {
                epoch: session.movement_epoch,
                on_ground: packet.on_ground,
            });
        }
        MovementPacketKind::Look => {
            let _ = to_net.0.send(ToNetMessage::PlayerMoveLook {
                epoch: session.movement_epoch,
                yaw: packet.yaw,
                pitch: packet.pitch,
                on_ground: packet.on_ground,
            });
        }
        MovementPacketKind::Pos => {
            let _ = to_net.0.send(ToNetMessage::PlayerMovePos {
                epoch: session.movement_epoch,
                x: packet.pos_f64.0,
                y: packet.pos_f64.1,
                z: packet.pos_f64.2,
                on_ground: packet.on_ground,
            });
        }
        MovementPacketKind::PosLook => {
            let _ = to_net.0.send(ToNetMessage::PlayerMovePosLook {
                epoch: session.movement_epoch,
                x: packet.pos_f64.0,
                y: packet.pos_f64.1,
                z: packet.pos_f64.2,
                yaw: packet.yaw,
                pitch: packet.pitch,
                on_ground: packet.on_ground,
            });
        }
    }

    latency.last_sent = Some(Instant::now());
}

pub fn transaction_pacing_system(
    app_state: Res<AppState>,
    to_net: Res<ToNet>,
    mut session: ResMut<MovementSession>,
) {
    if !matches!(app_state.0, ApplicationState::Connected) {
        session.pending_tx_acks.clear();
        return;
    }

    while let Some(ack) = session.pop_next_tx_ack_for_send(false) {
        let is_grim_transaction = ack.window_id == 0 && ack.action_number < 0;
        let _ = to_net.0.send(ToNetMessage::ConfirmTransaction {
            id: ack.window_id,
            action_number: ack.action_number,
            accepted: ack.accepted,
        });
        debug!(
            phase = ?session.phase,
            window_id = ack.window_id,
            action_number = ack.action_number,
            accepted = ack.accepted,
            queued_phase = ?ack.queued_phase,
            "movement-session: transaction ack sent"
        );
        if session.correction_active() && is_grim_transaction {
            break;
        }
    }
}
