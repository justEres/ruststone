use std::time::Instant;

use bevy::prelude::*;
use tracing::{debug, info, warn};

use crate::movement_session::state::{
    FORCE_POSLOOK_TICKS_AFTER_CORRECTION, MovementObservation, MovementPacketKind,
    MovementPacketSource, MovementPhase, MovementSession, PlannedMovementPacket, POS_DELTA_SQ_EPS,
    SentMovementPacket, ServerCorrection, TELEPORT_COMMIT_HOLD_TICKS,
    TELEPORT_RESYNC_HOLD_TICKS, TransactionAck,
};
use crate::sim::collision::WorldCollisionMap;
use crate::sim::PlayerSimState;
use crate::sim_systems::PredictionHistory;

impl MovementSession {
    fn has_pending_grim_transactions(&self) -> bool {
        self.pending_tx_acks
            .iter()
            .any(|ack| ack.window_id == 0 && ack.action_number < 0)
    }

    fn correction_effective_on_ground(correction: ServerCorrection) -> bool {
        correction.on_ground
    }

    pub fn reset_all(&mut self) {
        *self = Self::default();
    }

    pub fn reset_runtime(&mut self) {
        self.phase = MovementPhase::Normal;
        self.pending_server_corrections.clear();
        self.pending_tx_acks.clear();
        self.active_correction = None;
        self.physics_hold_ticks = 0;
        self.phase_ticks_remaining = 0;
        self.baseline_initialized = false;
        self.ticks_since_pos = 0;
        self.last_sent_tick = None;
        self.outbound_journal.clear();
        self.repeated_correction_count = 0;
        self.force_poslook_ticks = 0;
        self.blocked_normal_send_tick = None;
        self.movement_epoch = 0;
    }

    pub fn queue_transaction_ack(&mut self, window_id: u8, action_number: i16, accepted: bool) {
        self.pending_tx_acks.push_back(TransactionAck {
            window_id,
            action_number,
            accepted,
            queued_phase: self.phase,
        });
    }

    pub fn correction_active(&self) -> bool {
        !matches!(self.phase, MovementPhase::Normal)
            || !self.pending_server_corrections.is_empty()
            || self.active_correction.is_some()
    }

    pub fn consume_physics_hold(&mut self) -> bool {
        if self.physics_hold_ticks == 0 {
            return false;
        }
        self.physics_hold_ticks = self.physics_hold_ticks.saturating_sub(1);
        true
    }

    pub(crate) fn transition_to(&mut self, next: MovementPhase, reason: &'static str) {
        if self.phase == next {
            return;
        }
        info!(
            old_phase = ?self.phase,
            new_phase = ?next,
            reason,
            pending_corrections = self.pending_server_corrections.len(),
            pending_tx_acks = self.pending_tx_acks.len(),
            "movement-session: phase transition"
        );
        self.phase = next;
    }

    fn sync_baseline(&mut self, pos: Vec3, yaw: f32, pitch: f32) {
        self.baseline_initialized = true;
        self.baseline_pos = pos;
        self.baseline_yaw_deg = yaw;
        self.baseline_pitch_deg = pitch;
        self.ticks_since_pos = 0;
    }

    pub(crate) fn begin_correction(&mut self, correction: ServerCorrection) {
        let repeated_same_correction = correction
            .sim_pos
            .distance_squared(self.last_authoritative_state.pos)
            <= 1.0e-6
            && correction.on_ground == self.last_authoritative_state.on_ground;
        self.repeated_correction_count = if repeated_same_correction {
            self.repeated_correction_count.saturating_add(1)
        } else {
            0
        };
        let effective_on_ground = Self::correction_effective_on_ground(correction);
        self.last_authoritative_state = PlayerSimState {
            pos: correction.sim_pos,
            vel: Vec3::ZERO,
            on_ground: effective_on_ground,
            collided_horizontally: false,
            jump_ticks: 0,
            yaw: correction.sim_yaw,
            pitch: correction.sim_pitch,
        };
        self.active_correction = Some(correction);
        self.pending_server_corrections.clear();
        self.physics_hold_ticks = 1;
        self.phase_ticks_remaining = 0;
        self.movement_epoch = self.movement_epoch.wrapping_add(1);
        self.sync_baseline(
            correction.sim_pos,
            correction.packet_yaw_deg,
            correction.packet_pitch_deg,
        );
        self.force_poslook_ticks = FORCE_POSLOOK_TICKS_AFTER_CORRECTION;
        self.blocked_normal_send_tick = Some(correction.recv_sim_tick);
        self.transition_to(MovementPhase::AwaitingTeleportAck, "server correction received");
        info!(
            sim_tick = correction.recv_sim_tick,
            x = correction.ack_pos.0,
            y = correction.ack_pos.1,
            z = correction.ack_pos.2,
            yaw = correction.packet_yaw_deg,
            pitch = correction.packet_pitch_deg,
            on_ground = effective_on_ground,
            on_ground_known = correction.on_ground_known,
            repeats = self.repeated_correction_count,
            "movement-session: correction queued"
        );
    }

    fn advance_phase_without_send(&mut self) {
        match self.phase {
            MovementPhase::AwaitingTeleportCommit => {
                if self.phase_ticks_remaining > 0 {
                    self.phase_ticks_remaining -= 1;
                }
                if self.phase_ticks_remaining == 0 {
                    self.phase_ticks_remaining = TELEPORT_RESYNC_HOLD_TICKS;
                    self.transition_to(MovementPhase::ResyncHold, "teleport commit window passed");
                }
            }
            MovementPhase::ResyncHold => {
                if self.phase_ticks_remaining > 0 {
                    self.phase_ticks_remaining -= 1;
                }
                if self.phase_ticks_remaining == 0 {
                    self.transition_to(MovementPhase::Replay, "resync hold completed");
                }
            }
            _ => {}
        }
    }

    pub(crate) fn make_ack_packet(&self, correction: ServerCorrection) -> PlannedMovementPacket {
        PlannedMovementPacket {
            source: MovementPacketSource::Ack,
            kind: MovementPacketKind::PosLook,
            pos_f64: correction.ack_pos,
            pos_f32: Vec3::new(
                correction.ack_pos.0 as f32,
                correction.ack_pos.1 as f32,
                correction.ack_pos.2 as f32,
            ),
            yaw: correction.packet_yaw_deg,
            pitch: correction.packet_pitch_deg,
            on_ground: false,
        }
    }

    fn make_normal_packet(&mut self, obs: MovementObservation) -> PlannedMovementPacket {
        let moved = if self.baseline_initialized {
            obs.pos.distance_squared(self.baseline_pos) > POS_DELTA_SQ_EPS
                || self.ticks_since_pos >= 20
        } else {
            true
        };
        let rotated = if self.baseline_initialized {
            (obs.yaw - self.baseline_yaw_deg).abs() > 0.001
                || (obs.pitch - self.baseline_pitch_deg).abs() > 0.001
        } else {
            true
        };

        let mut kind = if moved && rotated {
            MovementPacketKind::PosLook
        } else if moved {
            MovementPacketKind::Pos
        } else if rotated {
            MovementPacketKind::Look
        } else {
            MovementPacketKind::Ground
        };

        if self.force_poslook_ticks > 0 && moved {
            kind = MovementPacketKind::PosLook;
            self.force_poslook_ticks -= 1;
        } else if self.force_poslook_ticks > 0 {
            self.force_poslook_ticks -= 1;
        }

        if moved {
            self.baseline_pos = obs.pos;
            self.ticks_since_pos = 0;
        } else {
            self.ticks_since_pos = self.ticks_since_pos.saturating_add(1);
        }
        self.baseline_yaw_deg = obs.yaw;
        self.baseline_pitch_deg = obs.pitch;
        self.baseline_initialized = true;

        PlannedMovementPacket {
            source: MovementPacketSource::Normal,
            kind,
            pos_f64: (obs.pos.x as f64, obs.pos.y as f64, obs.pos.z as f64),
            pos_f32: obs.pos,
            yaw: obs.yaw,
            pitch: obs.pitch,
            on_ground: obs.on_ground,
        }
    }

    pub(crate) fn record_packet(&mut self, tick: u32, packet: PlannedMovementPacket) {
        debug_assert!(
            self.last_sent_tick != Some(tick),
            "movement-session: more than one movement packet planned for tick {tick}"
        );
        if packet.source == MovementPacketSource::Normal
            && !matches!(self.phase, MovementPhase::Normal | MovementPhase::Replay)
        {
            warn!(
                phase = ?self.phase,
                tick,
                "movement-session: normal movement packet planned during correction window"
            );
        }
        if packet.source == MovementPacketSource::Ack
            && let Some(correction) = self.active_correction
        {
            debug_assert!(
                packet.pos_f64 == correction.ack_pos,
                "movement-session: correction ack lost exact server payload"
            );
        }
        let entry = SentMovementPacket {
            sim_tick: tick,
            wall_time: Instant::now(),
            source: packet.source,
            kind: packet.kind,
            pos_f64: packet.pos_f64,
            pos_f32: packet.pos_f32,
            yaw: packet.yaw,
            pitch: packet.pitch,
            on_ground: packet.on_ground,
        };
        self.last_sent_tick = Some(tick);
        if self.outbound_journal.len() == super::state::JOURNAL_LIMIT {
            self.outbound_journal.pop_front();
        }
        self.outbound_journal.push_back(entry);
        debug!(
            tick,
            phase = ?self.phase,
            source = ?packet.source,
            kind = ?packet.kind,
            x = packet.pos_f64.0,
            y = packet.pos_f64.1,
            z = packet.pos_f64.2,
            yaw = packet.yaw,
            pitch = packet.pitch,
            on_ground = packet.on_ground,
            "movement-session: packet planned"
        );
    }

    pub fn plan_movement_packet(
        &mut self,
        tick: u32,
        obs: MovementObservation,
        chunk_loaded: bool,
    ) -> Option<PlannedMovementPacket> {
        if let Some(correction) = self.pending_server_corrections.pop_back() {
            self.begin_correction(correction);
        }

        match self.phase {
            MovementPhase::AwaitingTeleportAck => {
                let correction = self
                    .active_correction
                    .expect("teleport ack phase requires active correction");
                let packet = self.make_ack_packet(correction);
                self.phase_ticks_remaining = TELEPORT_COMMIT_HOLD_TICKS;
                self.transition_to(MovementPhase::AwaitingTeleportCommit, "teleport ack sent");
                self.record_packet(tick, packet);
                return Some(packet);
            }
            MovementPhase::AwaitingTeleportCommit | MovementPhase::ResyncHold => {
                warn!(
                    tick,
                    phase = ?self.phase,
                    "movement-session: suppressing normal movement during correction window"
                );
                self.advance_phase_without_send();
                return None;
            }
            MovementPhase::Replay | MovementPhase::Normal => {}
        }

        if self.active_correction.is_some() && self.has_pending_grim_transactions() {
            warn!(
                tick,
                phase = ?self.phase,
                pending_tx_acks = self.pending_tx_acks.len(),
                "movement-session: suppressing movement until grim transactions drain"
            );
            return None;
        }

        if self.blocked_normal_send_tick == Some(tick) {
            warn!(
                tick,
                phase = ?self.phase,
                "movement-session: suppressing normal movement for correction tick"
            );
            return None;
        }

        if !chunk_loaded {
            debug!(
                tick,
                phase = ?self.phase,
                "movement-session: no loaded player chunk, movement send skipped"
            );
            return None;
        }

        let packet = self.make_normal_packet(obs);
        self.record_packet(tick, packet);
        if matches!(self.phase, MovementPhase::Replay) {
            self.active_correction = None;
            self.blocked_normal_send_tick = None;
            self.transition_to(MovementPhase::Normal, "replay send resumed normal flow");
        }
        Some(packet)
    }

    pub fn pop_next_tx_ack_for_send(
        &mut self,
        _grim_sent_during_correction: bool,
    ) -> Option<TransactionAck> {
        let next = *self.pending_tx_acks.front()?;
        let is_grim_transaction = next.window_id == 0 && next.action_number < 0;

        if !matches!(self.phase, MovementPhase::Normal | MovementPhase::Replay)
            && is_grim_transaction
        {
            debug!(
                phase = ?self.phase,
                action_number = next.action_number,
                "movement-session: deferring grim transaction during correction window"
            );
            return None;
        }

        self.pending_tx_acks.pop_front()
    }
}

pub(crate) fn wrap_degrees(mut deg: f32) -> f32 {
    while deg <= -180.0 {
        deg += 360.0;
    }
    while deg > 180.0 {
        deg -= 360.0;
    }
    deg
}

pub(crate) fn has_loaded_player_chunk(collision_map: &WorldCollisionMap, pos: Vec3) -> bool {
    let chunk_x = (pos.x.floor() as i32).div_euclid(16);
    let chunk_z = (pos.z.floor() as i32).div_euclid(16);
    collision_map.has_chunk(chunk_x, chunk_z)
}

pub(crate) fn estimate_server_tick(
    history: &PredictionHistory,
    latest_tick: u32,
    estimated_tick: u32,
    server_state: &PlayerSimState,
) -> (u32, i32) {
    let search_radius = 8u32;
    let start = estimated_tick.saturating_sub(search_radius);
    let end = latest_tick.min(estimated_tick.saturating_add(search_radius));

    let mut best_tick = estimated_tick;
    let mut best_score = f32::INFINITY;
    let mut found = false;

    for tick in start..=end {
        let Some(frame) = history.0.get_by_tick(tick) else {
            continue;
        };
        let predicted = frame.state;
        let pos_err = server_state.pos.distance_squared(predicted.pos);
        let vel_err = server_state.vel.distance_squared(predicted.vel);
        let yaw_err = (server_state.yaw - predicted.yaw).abs();
        let pitch_err = (server_state.pitch - predicted.pitch).abs();
        let ground_err = if server_state.on_ground == predicted.on_ground {
            0.0
        } else {
            0.25
        };
        let score = pos_err + vel_err * 0.35 + yaw_err * 0.02 + pitch_err * 0.02 + ground_err;
        if score < best_score {
            best_score = score;
            best_tick = tick;
            found = true;
        }
    }

    if !found {
        return (estimated_tick, 0);
    }

    (best_tick, best_tick as i32 - estimated_tick as i32)
}
