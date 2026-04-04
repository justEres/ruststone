use std::collections::VecDeque;
use std::time::Instant;

use bevy::prelude::*;

use crate::sim::PlayerSimState;

pub(crate) const TELEPORT_COMMIT_HOLD_TICKS: u8 = 4;
pub(crate) const TELEPORT_RESYNC_HOLD_TICKS: u8 = 2;
pub(crate) const FORCE_POSLOOK_TICKS_AFTER_CORRECTION: u8 = 8;
pub(crate) const JOURNAL_LIMIT: usize = 32;
pub(crate) const POS_DELTA_SQ_EPS: f32 = 0.0009;

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
    pub movement_epoch: u64,
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
            movement_epoch: 0,
        }
    }
}
