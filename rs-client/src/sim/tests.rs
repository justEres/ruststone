use super::movement::{simulate_tick, WorldCollision};
use super::predict::PredictionBuffer;
use super::reconcile::reconcile;
use super::types::{InputState, PlayerSimState, PredictedFrame};
use bevy::prelude::Vec3;

fn input_sequence(len: usize) -> Vec<InputState> {
    let mut inputs = Vec::with_capacity(len);
    for i in 0..len {
        inputs.push(InputState {
            forward: if i % 2 == 0 { 1.0 } else { 0.5 },
            strafe: if i % 3 == 0 { 0.2 } else { -0.1 },
            jump: i % 30 == 0,
            sprint: i % 20 < 10,
            sneak: i % 50 > 40,
            yaw: (i as f32 * 0.01) % 6.28,
            pitch: (i as f32 * 0.005) % 1.5,
        });
    }
    inputs
}

#[test]
fn determinism() {
    let world = WorldCollision::empty();
    let inputs = input_sequence(200);
    let mut state = PlayerSimState::default();
    for input in &inputs {
        state = simulate_tick(&state, input, &world);
    }
    let final_a = state;

    let mut state = PlayerSimState::default();
    for input in &inputs {
        state = simulate_tick(&state, input, &world);
    }
    let final_b = state;

    let diff = (final_a.pos - final_b.pos).length();
    assert!(diff < 1e-6);
    assert!((final_a.vel - final_b.vel).length() < 1e-6);
    assert!((final_a.yaw - final_b.yaw).abs() < 1e-6);
    assert!((final_a.pitch - final_b.pitch).abs() < 1e-6);
}

#[test]
fn replay_equivalence() {
    let world = WorldCollision::empty();
    let inputs = input_sequence(200);
    let mut buffer = PredictionBuffer::new(256);
    let mut state = PlayerSimState::default();

    for (tick, input) in inputs.iter().enumerate() {
        state = simulate_tick(&state, input, &world);
        buffer.push(PredictedFrame {
            tick: tick as u32,
            input: *input,
            state,
        });
    }

    let server_tick = 120u32;
    let mut corrected = buffer.get_by_tick(server_tick).unwrap().state;
    corrected.pos += Vec3::new(0.05, 0.0, -0.02);

    let mut current = buffer.latest_tick().unwrap();
    let mut current_state = buffer.get_by_tick(current).unwrap().state;

    let res = reconcile(
        &mut buffer,
        &world,
        server_tick,
        corrected,
        current,
        &mut current_state,
    )
    .unwrap();

    let mut authoritative = corrected;
    for t in (server_tick + 1)..=current {
        let input = inputs[t as usize];
        authoritative = simulate_tick(&authoritative, &input, &world);
    }

    let diff = (authoritative.pos - current_state.pos).length();
    assert!(diff < 1e-4, "replay diff {:?}", diff);
    assert!(res.replayed_ticks > 0);
}

#[test]
fn ring_buffer_integrity() {
    let mut buffer = PredictionBuffer::new(8);
    let mut state = PlayerSimState::default();
    for tick in 0..20u32 {
        let input = InputState::default();
        buffer.push(PredictedFrame { tick, input, state });
    }

    assert!(buffer.get_by_tick(0).is_none());
    assert!(buffer.get_by_tick(12).is_some());
    assert!(buffer.get_by_tick(19).is_some());
}
