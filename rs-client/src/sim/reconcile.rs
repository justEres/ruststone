use bevy::prelude::Vec3;

use super::movement::{WorldCollision, simulate_tick};
use super::predict::PredictionBuffer;
use super::types::{InputState, PlayerSimState};

pub struct ReconcileResult {
    pub correction: Vec3,
    pub replayed_ticks: u32,
    pub hard_teleport: bool,
}

pub fn reconcile(
    buffer: &mut PredictionBuffer,
    world: &WorldCollision,
    server_tick: u32,
    server_state: PlayerSimState,
    client_tick: u32,
    current_state: &mut PlayerSimState,
) -> Option<ReconcileResult> {
    let Some(predicted) = find_frame(buffer, server_tick) else {
        return None;
    };

    let err = server_state.pos - predicted.state.pos;
    let err_len = err.length();

    const SMALL_EPS: f32 = 0.001;
    const SOFT_CORRECT: f32 = 0.1;
    const HARD_TELEPORT: f32 = 3.0;

    if err_len < SMALL_EPS {
        return None;
    }

    if err_len >= HARD_TELEPORT {
        *current_state = server_state;
        buffer.truncate_older_than(server_tick);
        return Some(ReconcileResult {
            correction: err,
            replayed_ticks: 0,
            hard_teleport: true,
        });
    }

    let mut base_state = server_state;
    let mut replayed = 0u32;

    for t in (server_tick + 1)..=client_tick {
        let input = buffer
            .get_by_tick(t)
            .map(|f| f.input)
            .unwrap_or(InputState::default());
        base_state = simulate_tick(&base_state, &input, world);
        if let Some(frame) = buffer.get_by_tick_mut(t) {
            frame.state = base_state;
        }
        replayed += 1;
    }

    *current_state = base_state;
    Some(ReconcileResult {
        correction: err,
        replayed_ticks: replayed,
        hard_teleport: err_len >= HARD_TELEPORT,
    })
}

fn find_frame<'a>(
    buffer: &'a PredictionBuffer,
    tick: u32,
) -> Option<&'a super::types::PredictedFrame> {
    if let Some(frame) = buffer.get_by_tick(tick) {
        return Some(frame);
    }

    let latest = buffer.latest_tick()?;
    for t in (0..=latest).rev() {
        if t < tick {
            break;
        }
        if let Some(frame) = buffer.get_by_tick(t) {
            return Some(frame);
        }
    }
    None
}
