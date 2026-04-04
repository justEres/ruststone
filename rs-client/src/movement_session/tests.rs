use std::time::Instant;

use bevy::prelude::*;

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
    let packet = session.make_ack_packet(sample_correction());
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

    for tick in 2..=7 {
        assert!(session
            .plan_movement_packet(
                tick,
                MovementObservation {
                    pos: Vec3::new(2.0, 64.0, 2.0),
                    yaw: 10.0,
                    pitch: 0.0,
                    on_ground: true,
                },
                true,
            )
            .is_none());
    }
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

    for tick in 2..=7 {
        let _ = session.plan_movement_packet(
            tick,
            MovementObservation {
                pos: Vec3::new(2.0, 64.0, 2.0),
                yaw: 10.0,
                pitch: 0.0,
                on_ground: true,
            },
            true,
        );
    }
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
fn correction_ack_matches_vanilla_on_ground_false() {
    let mut session = MovementSession::default();
    let correction = sample_unknown_ground_correction();
    session.begin_correction(correction);
    let packet = session.make_ack_packet(correction);
    assert!(!packet.on_ground);
    assert!(session.last_authoritative_state.on_ground);
}
