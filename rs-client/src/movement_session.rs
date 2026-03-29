use std::collections::VecDeque;
use std::time::Instant;

use bevy::prelude::*;
use rs_utils::{AppState, ApplicationState, PerfTimings, ToNet, ToNetMessage};
use tracing::{debug, info, warn};

use crate::net::events::{NetEvent, NetEventQueue};
use crate::sim::collision::WorldCollisionMap;
use crate::sim::movement::WorldCollision;
use crate::sim::reconcile::reconcile;
use crate::sim::{
    DebugStats, PlayerSimState, SimClock, SimReady, SimRenderState, SimState,
    VisualCorrectionOffset,
};
use crate::sim_systems::{LatencyEstimate, PredictionHistory};
use crate::timing::Timing;

const MOVE_PKT_GROUND: u8 = 0;
const MOVE_PKT_LOOK: u8 = 1;
const MOVE_PKT_POS: u8 = 2;
const MOVE_PKT_POS_LOOK: u8 = 3;

const TELEPORT_COMMIT_HOLD_TICKS: u8 = 4;
const TELEPORT_RESYNC_HOLD_TICKS: u8 = 2;
const FORCE_POSLOOK_TICKS_AFTER_CORRECTION: u8 = 8;
const JOURNAL_LIMIT: usize = 32;
const POS_DELTA_SQ_EPS: f32 = 0.0009;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MovementPhase {
    Normal,
    AwaitingTeleportAck,
    AwaitingTeleportCommit,
    ResyncHold,
    Replay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MovementPacketSource {
    Ack,
    Normal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MovementPacketKind {
    Ground,
    Look,
    Pos,
    PosLook,
}

#[derive(Debug, Clone, Copy)]
pub struct ServerCorrection {
    pub sim_pos: Vec3,
    pub ack_pos: (f64, f64, f64),
    pub sim_yaw: f32,
    pub sim_pitch: f32,
    pub packet_yaw_deg: f32,
    pub packet_pitch_deg: f32,
    pub on_ground: bool,
    pub on_ground_known: bool,
    #[allow(dead_code)]
    pub recv_instant: Instant,
    pub recv_sim_tick: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct TransactionAck {
    pub window_id: u8,
    pub action_number: i16,
    pub accepted: bool,
    pub queued_phase: MovementPhase,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub struct SentMovementPacket {
    pub sim_tick: u32,
    pub wall_time: Instant,
    pub source: MovementPacketSource,
    pub kind: MovementPacketKind,
    pub pos_f64: (f64, f64, f64),
    pub pos_f32: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub on_ground: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct MovementObservation {
    pub pos: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub on_ground: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct PlannedMovementPacket {
    pub source: MovementPacketSource,
    pub kind: MovementPacketKind,
    pub pos_f64: (f64, f64, f64),
    pub pos_f32: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub on_ground: bool,
}

#[derive(Debug, Resource)]
pub struct MovementSession {
    pub phase: MovementPhase,
    pub pending_server_corrections: VecDeque<ServerCorrection>,
    pub pending_tx_acks: VecDeque<TransactionAck>,
    pub outbound_journal: VecDeque<SentMovementPacket>,
    pub active_correction: Option<ServerCorrection>,
    pub last_authoritative_state: PlayerSimState,
    pub physics_hold_ticks: u32,
    pub phase_ticks_remaining: u8,
    pub baseline_initialized: bool,
    pub baseline_pos: Vec3,
    pub baseline_yaw_deg: f32,
    pub baseline_pitch_deg: f32,
    pub ticks_since_pos: u32,
    pub last_sent_tick: Option<u32>,
    pub repeated_correction_count: u32,
    pub force_poslook_ticks: u8,
    pub blocked_normal_send_tick: Option<u32>,
}

impl Default for MovementSession {
    fn default() -> Self {
        Self {
            phase: MovementPhase::Normal,
            pending_server_corrections: VecDeque::new(),
            pending_tx_acks: VecDeque::new(),
            outbound_journal: VecDeque::with_capacity(JOURNAL_LIMIT),
            active_correction: None,
            last_authoritative_state: PlayerSimState::default(),
            physics_hold_ticks: 0,
            phase_ticks_remaining: 0,
            baseline_initialized: false,
            baseline_pos: Vec3::ZERO,
            baseline_yaw_deg: 0.0,
            baseline_pitch_deg: 0.0,
            ticks_since_pos: 0,
            last_sent_tick: None,
            repeated_correction_count: 0,
            force_poslook_ticks: 0,
            blocked_normal_send_tick: None,
        }
    }
}

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

    fn transition_to(&mut self, next: MovementPhase, reason: &'static str) {
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

    fn begin_correction(&mut self, correction: ServerCorrection) {
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

    fn make_ack_packet(&self, correction: ServerCorrection) -> PlannedMovementPacket {
        let on_ground = Self::correction_effective_on_ground(correction);
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
            on_ground,
        }
    }

    fn make_normal_packet(&mut self, obs: MovementObservation) -> PlannedMovementPacket {
        let moved = if self.baseline_initialized {
            obs.pos.distance_squared(self.baseline_pos) > POS_DELTA_SQ_EPS || self.ticks_since_pos >= 20
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

    fn record_packet(&mut self, tick: u32, packet: PlannedMovementPacket) {
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
        if self.outbound_journal.len() == JOURNAL_LIMIT {
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
                let correction = self.active_correction.expect("teleport ack phase requires active correction");
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

fn kind_to_wire(kind: MovementPacketKind) -> u8 {
    match kind {
        MovementPacketKind::Ground => MOVE_PKT_GROUND,
        MovementPacketKind::Look => MOVE_PKT_LOOK,
        MovementPacketKind::Pos => MOVE_PKT_POS,
        MovementPacketKind::PosLook => MOVE_PKT_POS_LOOK,
    }
}

fn kind_name(kind: MovementPacketKind) -> &'static str {
    match kind {
        MovementPacketKind::Ground => "ground",
        MovementPacketKind::Look => "look",
        MovementPacketKind::Pos => "pos",
        MovementPacketKind::PosLook => "poslook",
    }
}

fn source_name(source: MovementPacketSource) -> &'static str {
    match source {
        MovementPacketSource::Ack => "ack",
        MovementPacketSource::Normal => "normal",
    }
}

fn wrap_degrees(mut deg: f32) -> f32 {
    while deg <= -180.0 {
        deg += 360.0;
    }
    while deg > 180.0 {
        deg -= 360.0;
    }
    deg
}

fn has_loaded_player_chunk(collision_map: &WorldCollisionMap, pos: Vec3) -> bool {
    let chunk_x = (pos.x.floor() as i32).div_euclid(16);
    let chunk_z = (pos.z.floor() as i32).div_euclid(16);
    collision_map.has_chunk(chunk_x, chunk_z)
}

fn estimate_server_tick(
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
                let server_state = PlayerSimState {
                    pos,
                    vel: Vec3::ZERO,
                    on_ground,
                    collided_horizontally: false,
                    jump_ticks: 0,
                    yaw,
                    pitch,
                };
                let correction_delta = pos - sim_state.current.pos;
                session.begin_correction(correction);
                let ack_packet = session.make_ack_packet(correction);
                let tick = sim_clock.tick;
                let _ = to_net.0.send(ToNetMessage::PlayerMovePosLook {
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
                tracing::info!(
                    tick,
                    source = source_name(ack_packet.source),
                    kind = kind_name(ack_packet.kind),
                    wire_kind = kind_to_wire(ack_packet.kind),
                    x = ack_packet.pos_f64.0,
                    y = ack_packet.pos_f64.1,
                    z = ack_packet.pos_f64.2,
                    yaw = ack_packet.yaw,
                    pitch = ack_packet.pitch,
                    on_ground = ack_packet.on_ground,
                    phase = ?session.phase,
                    "Outgoing movement packet"
                );
                latency.last_sent = Some(Instant::now());
                sim_render.previous = server_state;
                sim_state.current = server_state;
                history.0 = PredictionHistory::default().0;
                visual_offset.0 = Vec3::ZERO;
                debug.last_correction = correction_delta.length();
                debug.last_replay = 0;
                debug.last_velocity_correction = 0.0;
                debug.last_reconciled_server_tick = None;
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
            let _ = to_net
                .0
                .send(ToNetMessage::PlayerMoveGround { on_ground: packet.on_ground });
        }
        MovementPacketKind::Look => {
            let _ = to_net.0.send(ToNetMessage::PlayerMoveLook {
                yaw: packet.yaw,
                pitch: packet.pitch,
                on_ground: packet.on_ground,
            });
        }
        MovementPacketKind::Pos => {
            let _ = to_net.0.send(ToNetMessage::PlayerMovePos {
                x: packet.pos_f64.0,
                y: packet.pos_f64.1,
                z: packet.pos_f64.2,
                on_ground: packet.on_ground,
            });
        }
        MovementPacketKind::PosLook => {
            let _ = to_net.0.send(ToNetMessage::PlayerMovePosLook {
                x: packet.pos_f64.0,
                y: packet.pos_f64.1,
                z: packet.pos_f64.2,
                yaw: packet.yaw,
                pitch: packet.pitch,
                on_ground: packet.on_ground,
            });
        }
    }

    tracing::info!(
        tick,
        source = source_name(packet.source),
        kind = kind_name(packet.kind),
        wire_kind = kind_to_wire(packet.kind),
        x = packet.pos_f64.0,
        y = packet.pos_f64.1,
        z = packet.pos_f64.2,
        yaw = packet.yaw,
        pitch = packet.pitch,
        on_ground = packet.on_ground,
        phase = ?session.phase,
        "Outgoing movement packet"
    );
    latency.last_sent = Some(Instant::now());
}

pub fn transaction_pacing_system(app_state: Res<AppState>, to_net: Res<ToNet>, mut session: ResMut<MovementSession>) {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_correction() -> ServerCorrection {
        ServerCorrection {
            sim_pos: Vec3::new(1.0, 64.0, 2.0),
            ack_pos: (1.0, 64.0000001, 2.0),
            sim_yaw: 0.0,
            sim_pitch: 0.0,
            packet_yaw_deg: 180.0,
            packet_pitch_deg: 0.0,
            on_ground: true,
            on_ground_known: true,
            recv_instant: Instant::now(),
            recv_sim_tick: 42,
        }
    }

    fn sample_unknown_ground_correction() -> ServerCorrection {
        ServerCorrection {
            on_ground_known: false,
            ..sample_correction()
        }
    }

    #[test]
    fn correction_ack_uses_exact_server_payload() {
        let mut session = MovementSession::default();
        session.begin_correction(sample_correction());
        let packet = session
            .make_ack_packet(sample_correction());
        assert_eq!(packet.source, MovementPacketSource::Ack);
        assert_eq!(packet.kind, MovementPacketKind::PosLook);
        assert_eq!(packet.pos_f64, (1.0, 64.0000001, 2.0));
    }

    #[test]
    fn correction_window_suppresses_normal_packets_until_replay() {
        let mut session = MovementSession::default();
        let correction = sample_correction();
        session.begin_correction(correction);
        let packet = session.make_ack_packet(correction);
        session.phase_ticks_remaining = TELEPORT_COMMIT_HOLD_TICKS;
        session.transition_to(
            MovementPhase::AwaitingTeleportCommit,
            "test immediate correction ack",
        );
        session.record_packet(1, packet);

        assert!(session.plan_movement_packet(
            2,
            MovementObservation {
                pos: Vec3::new(2.0, 64.0, 2.0),
                yaw: 10.0,
                pitch: 0.0,
                on_ground: true,
            },
            true
        ).is_none());
        assert!(session.plan_movement_packet(
            3,
            MovementObservation {
                pos: Vec3::new(2.0, 64.0, 2.0),
                yaw: 10.0,
                pitch: 0.0,
                on_ground: true,
            },
            true
        ).is_none());
        assert!(session.plan_movement_packet(
            4,
            MovementObservation {
                pos: Vec3::new(2.0, 64.0, 2.0),
                yaw: 10.0,
                pitch: 0.0,
                on_ground: true,
            },
            true
        ).is_none());
        assert!(session.plan_movement_packet(
            5,
            MovementObservation {
                pos: Vec3::new(2.0, 64.0, 2.0),
                yaw: 10.0,
                pitch: 0.0,
                on_ground: true,
            },
            true,
        ).is_none());
        assert!(session.plan_movement_packet(
            6,
            MovementObservation {
                pos: Vec3::new(2.0, 64.0, 2.0),
                yaw: 10.0,
                pitch: 0.0,
                on_ground: true,
            },
            true,
        ).is_none());
        assert!(session.plan_movement_packet(
            7,
            MovementObservation {
                pos: Vec3::new(2.0, 64.0, 2.0),
                yaw: 10.0,
                pitch: 0.0,
                on_ground: true,
            },
            true,
        ).is_none());
        let packet = session.plan_movement_packet(
            8,
            MovementObservation {
                pos: Vec3::new(2.0, 64.0, 2.0),
                yaw: 10.0,
                pitch: 0.0,
                on_ground: true,
            },
            true,
        );
        assert!(packet.is_some());
        assert_eq!(session.phase, MovementPhase::Normal);
    }

    #[test]
    fn grim_transactions_are_throttled_during_correction() {
        let mut session = MovementSession::default();
        let correction = sample_correction();
        session.begin_correction(correction);
        let packet = session.make_ack_packet(correction);
        session.phase_ticks_remaining = TELEPORT_COMMIT_HOLD_TICKS;
        session.transition_to(
            MovementPhase::AwaitingTeleportCommit,
            "test immediate correction ack",
        );
        session.record_packet(1, packet);
        session.queue_transaction_ack(0, -1, true);
        session.queue_transaction_ack(0, -2, true);
        let first = session.pop_next_tx_ack_for_send(false);
        assert!(first.is_none());

        let _ = session.plan_movement_packet(
            2,
            MovementObservation {
                pos: Vec3::new(2.0, 64.0, 2.0),
                yaw: 10.0,
                pitch: 0.0,
                on_ground: true,
            },
            true,
        );
        let _ = session.plan_movement_packet(
            3,
            MovementObservation {
                pos: Vec3::new(2.0, 64.0, 2.0),
                yaw: 10.0,
                pitch: 0.0,
                on_ground: true,
            },
            true,
        );
        let _ = session.plan_movement_packet(
            4,
            MovementObservation {
                pos: Vec3::new(2.0, 64.0, 2.0),
                yaw: 10.0,
                pitch: 0.0,
                on_ground: true,
            },
            true,
        );
        let _ = session.plan_movement_packet(
            5,
            MovementObservation {
                pos: Vec3::new(2.0, 64.0, 2.0),
                yaw: 10.0,
                pitch: 0.0,
                on_ground: true,
            },
            true,
        );
        let _ = session.plan_movement_packet(
            6,
            MovementObservation {
                pos: Vec3::new(2.0, 64.0, 2.0),
                yaw: 10.0,
                pitch: 0.0,
                on_ground: true,
            },
            true,
        );
        let _ = session.plan_movement_packet(
            7,
            MovementObservation {
                pos: Vec3::new(2.0, 64.0, 2.0),
                yaw: 10.0,
                pitch: 0.0,
                on_ground: true,
            },
            true,
        );
        let first = session.pop_next_tx_ack_for_send(false);
        assert_eq!(first.map(|ack| ack.action_number), Some(-1));
    }

    #[test]
    fn replay_waits_for_grim_transactions_to_drain() {
        let mut session = MovementSession::default();
        let correction = sample_correction();
        session.begin_correction(correction);
        let packet = session.make_ack_packet(correction);
        session.phase_ticks_remaining = 0;
        session.transition_to(MovementPhase::Replay, "test replay phase");
        session.record_packet(1, packet);
        session.queue_transaction_ack(0, -1, true);

        let packet = session.plan_movement_packet(
            2,
            MovementObservation {
                pos: Vec3::new(2.0, 64.0, 2.0),
                yaw: 10.0,
                pitch: 0.0,
                on_ground: true,
            },
            true,
        );
        assert!(packet.is_none());

        let _ = session.pop_next_tx_ack_for_send(false);
        let packet = session.plan_movement_packet(
            3,
            MovementObservation {
                pos: Vec3::new(2.0, 64.0, 2.0),
                yaw: 10.0,
                pitch: 0.0,
                on_ground: true,
            },
            true,
        );
        assert!(packet.is_some());
        assert_eq!(session.phase, MovementPhase::Normal);
    }

    #[test]
    fn correction_blocks_normal_send_for_same_tick() {
        let mut session = MovementSession::default();
        let correction = sample_correction();
        session.begin_correction(correction);
        let packet = session.plan_movement_packet(
            correction.recv_sim_tick,
            MovementObservation {
                pos: Vec3::new(2.0, 64.0, 2.0),
                yaw: 10.0,
                pitch: 0.0,
                on_ground: true,
            },
            true,
        );
        assert!(packet.is_some());
        assert_eq!(packet.unwrap().source, MovementPacketSource::Ack);

        let packet = session.plan_movement_packet(
            correction.recv_sim_tick,
            MovementObservation {
                pos: Vec3::new(2.5, 64.0, 2.5),
                yaw: 12.0,
                pitch: 1.0,
                on_ground: true,
            },
            true,
        );
        assert!(packet.is_none());
    }

    #[test]
    fn correction_ack_uses_inferred_ground_when_server_omits_it() {
        let mut session = MovementSession::default();
        let correction = sample_unknown_ground_correction();
        session.begin_correction(correction);
        let packet = session.make_ack_packet(correction);
        assert!(packet.on_ground);
        assert!(session.last_authoritative_state.on_ground);
    }
}
