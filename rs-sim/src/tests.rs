use super::collision::WorldCollisionMap;
use super::movement::{
    WorldCollision, collision_parity_expected_box_count, debug_block_collision_boxes,
    effective_sprint, simulate_tick,
};
use super::predict::PredictionBuffer;
use super::reconcile::reconcile;
use super::types::{InputState, PlayerSimState, PredictedFrame};
use bevy::prelude::Vec3;
use rs_utils::BlockUpdate;

fn input_sequence(len: usize) -> Vec<InputState> {
    let mut inputs = Vec::with_capacity(len);
    for i in 0..len {
        inputs.push(InputState {
            forward: if i % 2 == 0 { 1.0 } else { 0.5 },
            strafe: if i % 3 == 0 { 0.2 } else { -0.1 },
            jump: i % 30 == 0,
            sprint: i % 20 < 10,
            sneak: i % 50 > 40,
            can_fly: false,
            flying: false,
            flying_speed: 0.05,
            speed_multiplier: 1.0,
            jump_boost_amplifier: None,
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

#[test]
fn sprint_requires_strong_forward_input() {
    let input = InputState {
        sprint: true,
        sneak: false,
        forward: 0.79,
        ..Default::default()
    };
    assert!(!effective_sprint(&input));

    let input = InputState {
        sprint: true,
        sneak: false,
        forward: 0.8,
        ..Default::default()
    };
    assert!(effective_sprint(&input));
}

fn block_state(block_id: u16, meta: u16) -> u16 {
    (block_id << 4) | (meta & 0xF)
}

fn apply_blocks(map: &mut WorldCollisionMap, blocks: &[(i32, i32, i32, u16)]) {
    for (x, y, z, state) in blocks {
        map.apply_block_update(BlockUpdate {
            x: *x,
            y: *y,
            z: *z,
            block_id: *state,
        });
    }
}

fn approx_eq(a: f32, b: f32) -> bool {
    (a - b).abs() <= 1e-4
}

fn boxes_contain(
    boxes: &[(Vec3, Vec3)],
    min: (f32, f32, f32),
    max: (f32, f32, f32),
) -> bool {
    boxes.iter().any(|(box_min, box_max)| {
        approx_eq(box_min.x, min.0)
            && approx_eq(box_min.y, min.1)
            && approx_eq(box_min.z, min.2)
            && approx_eq(box_max.x, max.0)
            && approx_eq(box_max.y, max.1)
            && approx_eq(box_max.z, max.2)
    })
}

fn lay_floor(map: &mut WorldCollisionMap, x_min: i32, x_max: i32, z_min: i32, z_max: i32, y: i32) {
    let stone = block_state(1, 0);
    for z in z_min..=z_max {
        for x in x_min..=x_max {
            map.apply_block_update(BlockUpdate {
                x,
                y,
                z,
                block_id: stone,
            });
        }
    }
}

#[test]
fn stair_straight_collision_boxes() {
    let mut map = WorldCollisionMap::default();
    apply_blocks(&mut map, &[(0, 0, 0, block_state(53, 3))]); // oak stair, bottom, facing north
    let world = WorldCollision::with_map(&map);
    let boxes = debug_block_collision_boxes(&world, block_state(53, 3), 0, 0, 0);
    assert_eq!(boxes.len(), 2);
    assert_eq!(
        collision_parity_expected_box_count(&world, block_state(53, 3), 0, 0, 0),
        Some(2)
    );
}

#[test]
fn stair_outer_left_shape_collision_boxes() {
    let mut map = WorldCollisionMap::default();
    // center: north-facing stair; front neighbor: west-facing stair => outer-left corner.
    apply_blocks(
        &mut map,
        &[
            (0, 0, 0, block_state(53, 3)),
            (0, 0, -1, block_state(53, 1)),
        ],
    );
    let world = WorldCollision::with_map(&map);
    let boxes = debug_block_collision_boxes(&world, block_state(53, 3), 0, 0, 0);
    assert_eq!(boxes.len(), 2);
    assert_eq!(
        collision_parity_expected_box_count(&world, block_state(53, 3), 0, 0, 0),
        Some(2)
    );
    assert!(boxes_contain(&boxes, (0.0, 0.5, 0.0), (0.5, 1.0, 0.5)));
}

#[test]
fn stair_inner_right_shape_collision_boxes() {
    let mut map = WorldCollisionMap::default();
    // center: north-facing stair; back neighbor: east-facing stair => inner-right corner.
    apply_blocks(
        &mut map,
        &[
            (0, 0, 0, block_state(53, 3)),
            (0, 0, 1, block_state(53, 0)),
        ],
    );
    let world = WorldCollision::with_map(&map);
    let boxes = debug_block_collision_boxes(&world, block_state(53, 3), 0, 0, 0);
    assert_eq!(boxes.len(), 3);
    assert_eq!(
        collision_parity_expected_box_count(&world, block_state(53, 3), 0, 0, 0),
        Some(3)
    );
    assert!(boxes_contain(&boxes, (0.5, 0.5, 0.5), (1.0, 1.0, 1.0)));
}

#[test]
fn connectivity_collision_box_parity_counts() {
    let mut map = WorldCollisionMap::default();
    apply_blocks(
        &mut map,
        &[
            // fence with east/west connections
            (10, 0, 0, block_state(85, 0)),
            (9, 0, 0, block_state(85, 0)),
            (11, 0, 0, block_state(85, 0)),
            // pane cross
            (20, 0, 0, block_state(102, 0)),
            (19, 0, 0, block_state(102, 0)),
            (21, 0, 0, block_state(102, 0)),
            (20, 0, -1, block_state(102, 0)),
            (20, 0, 1, block_state(102, 0)),
            // wall north/south
            (30, 0, 0, block_state(139, 0)),
            (30, 0, -1, block_state(139, 0)),
            (30, 0, 1, block_state(139, 0)),
            // closed door
            (40, 0, 0, block_state(64, 0)),
            // closed gate
            (50, 0, 0, block_state(107, 0)),
        ],
    );
    let world = WorldCollision::with_map(&map);

    for (x, y, z, state) in [
        (10, 0, 0, block_state(85, 0)),
        (20, 0, 0, block_state(102, 0)),
        (30, 0, 0, block_state(139, 0)),
        (40, 0, 0, block_state(64, 0)),
        (50, 0, 0, block_state(107, 0)),
    ] {
        let actual = debug_block_collision_boxes(&world, state, x, y, z).len();
        let expected = collision_parity_expected_box_count(&world, state, x, y, z);
        assert_eq!(Some(actual), expected, "mismatch at ({x},{y},{z})");
    }
}

#[test]
fn diagonal_stair_sprint_regression_stays_stable() {
    let mut map = WorldCollisionMap::default();
    lay_floor(&mut map, -2, 6, -3, 3, 0);
    // west-facing stair obstacle in the sprint path.
    apply_blocks(&mut map, &[(1, 1, 0, block_state(53, 1))]);
    let world = WorldCollision::with_map(&map);

    let mut state = PlayerSimState {
        pos: Vec3::new(0.2, 1.0, 0.5),
        vel: Vec3::ZERO,
        on_ground: true,
        collided_horizontally: false,
        yaw: -std::f32::consts::FRAC_PI_2,
        pitch: 0.0,
    };
    let input = InputState {
        forward: 1.0,
        strafe: 0.0,
        jump: false,
        sprint: true,
        sneak: false,
        can_fly: false,
        flying: false,
        flying_speed: 0.05,
        speed_multiplier: 1.0,
        jump_boost_amplifier: None,
        yaw: -std::f32::consts::FRAC_PI_2,
        pitch: 0.0,
    };

    for _ in 0..40 {
        state = simulate_tick(&state, &input, &world);
        assert!(state.pos.is_finite());
        assert!(state.vel.is_finite());
    }
    // Regression guard: forward progress happens, but simulation remains bounded and stable.
    assert!(state.pos.x > 0.4, "expected forward progress, got x={}", state.pos.x);
    assert!(state.pos.y >= 0.9 && state.pos.y <= 2.1, "unexpected y={}", state.pos.y);
}

#[test]
fn sprint_jump_into_stair_preserves_forward_progress() {
    let mut map = WorldCollisionMap::default();
    lay_floor(&mut map, -2, 6, -3, 3, 0);
    apply_blocks(&mut map, &[(1, 1, 0, block_state(53, 1))]);
    let world = WorldCollision::with_map(&map);

    let mut state = PlayerSimState {
        pos: Vec3::new(0.2, 1.0, 0.5),
        vel: Vec3::ZERO,
        on_ground: true,
        collided_horizontally: false,
        yaw: -std::f32::consts::FRAC_PI_2,
        pitch: 0.0,
    };
    let mut input = InputState {
        forward: 1.0,
        strafe: 0.0,
        jump: true,
        sprint: true,
        sneak: false,
        can_fly: false,
        flying: false,
        flying_speed: 0.05,
        speed_multiplier: 1.0,
        jump_boost_amplifier: None,
        yaw: -std::f32::consts::FRAC_PI_2,
        pitch: 0.0,
    };

    for tick in 0..8 {
        state = simulate_tick(&state, &input, &world);
        if tick == 0 {
            input.jump = false;
        }
    }

    assert!(
        state.pos.x > 1.05,
        "expected sprint-jump stair entry progress, got x={}",
        state.pos.x
    );
    assert!(
        state.vel.x > 0.01,
        "unexpected forward velocity drop while stair-jumping, vel.x={}",
        state.vel.x
    );
}

#[test]
fn closed_door_and_gate_block_forward_motion() {
    let mut map = WorldCollisionMap::default();
    lay_floor(&mut map, -2, 8, -2, 2, 0);
    apply_blocks(
        &mut map,
        &[
            (1, 1, 0, block_state(64, 0)),
            (3, 1, 0, block_state(107, 0)),
        ],
    );
    let world = WorldCollision::with_map(&map);

    let mut state = PlayerSimState {
        pos: Vec3::new(0.2, 1.0, 0.5),
        vel: Vec3::ZERO,
        on_ground: true,
        collided_horizontally: false,
        yaw: -std::f32::consts::FRAC_PI_2,
        pitch: 0.0,
    };
    let input = InputState {
        forward: 1.0,
        strafe: 0.0,
        jump: false,
        sprint: true,
        sneak: false,
        can_fly: false,
        flying: false,
        flying_speed: 0.05,
        speed_multiplier: 1.0,
        jump_boost_amplifier: None,
        yaw: -std::f32::consts::FRAC_PI_2,
        pitch: 0.0,
    };

    for _ in 0..80 {
        state = simulate_tick(&state, &input, &world);
        assert!(state.pos.is_finite());
    }
    // Door at x=1 should block center from getting close enough to pass through.
    assert!(
        state.pos.x < 0.78,
        "player tunneled through closed colliders, x={}",
        state.pos.x
    );
}

#[test]
fn open_fence_gate_allows_forward_motion() {
    let mut map = WorldCollisionMap::default();
    lay_floor(&mut map, -2, 8, -2, 2, 0);
    // Open gate (meta bit 0x4 set).
    apply_blocks(&mut map, &[(0, 1, 0, block_state(107, 0x4))]);
    let world = WorldCollision::with_map(&map);

    let mut state = PlayerSimState {
        pos: Vec3::new(0.5, 1.0, -0.8),
        vel: Vec3::ZERO,
        on_ground: true,
        collided_horizontally: false,
        yaw: std::f32::consts::PI,
        pitch: 0.0,
    };
    let input = InputState {
        forward: 1.0,
        strafe: 0.0,
        jump: false,
        sprint: true,
        sneak: false,
        can_fly: false,
        flying: false,
        flying_speed: 0.05,
        speed_multiplier: 1.0,
        jump_boost_amplifier: None,
        yaw: std::f32::consts::PI,
        pitch: 0.0,
    };

    for _ in 0..80 {
        state = simulate_tick(&state, &input, &world);
        assert!(state.pos.is_finite());
    }
    assert!(
        state.pos.z > 1.2,
        "expected passing through open gate, z={}",
        state.pos.z
    );
}

#[test]
fn open_door_allows_forward_motion() {
    let mut map = WorldCollisionMap::default();
    lay_floor(&mut map, -2, 8, -2, 2, 0);
    // Open door lower-half metadata (facing east + open bit).
    apply_blocks(&mut map, &[(1, 1, 0, block_state(64, 0x4))]);
    let world = WorldCollision::with_map(&map);

    let mut state = PlayerSimState {
        pos: Vec3::new(0.2, 1.0, 0.5),
        vel: Vec3::ZERO,
        on_ground: true,
        collided_horizontally: false,
        yaw: -std::f32::consts::FRAC_PI_2,
        pitch: 0.0,
    };
    let input = InputState {
        forward: 1.0,
        strafe: 0.0,
        jump: false,
        sprint: true,
        sneak: false,
        can_fly: false,
        flying: false,
        flying_speed: 0.05,
        speed_multiplier: 1.0,
        jump_boost_amplifier: None,
        yaw: -std::f32::consts::FRAC_PI_2,
        pitch: 0.0,
    };

    for _ in 0..80 {
        state = simulate_tick(&state, &input, &world);
        assert!(state.pos.is_finite());
    }
    assert!(
        state.pos.x > 1.6,
        "expected passing through open door, x={}",
        state.pos.x
    );
}
