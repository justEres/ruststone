use bevy::prelude::Vec3;

use crate::{InputState, PlayerSimState};

use super::world::WorldCollision;
use super::{
    AIR_DRAG, BASE_MOVE_SPEED, FLY_HORIZONTAL_DAMPING, FLY_SPRINT_MULT, FLY_VERTICAL_ACCEL_MULT,
    FLY_VERTICAL_DAMPING, GRAVITY, JUMP_VEL, MOVE_INPUT_DAMPING, SOUL_SAND_SLOWDOWN,
    SPEED_IN_AIR, SNEAK_INPUT_SCALE, SPRINT_FORWARD_THRESHOLD, SWIM_UP_ACCEL, WATER_DRAG,
    WATER_GRAVITY, WATER_MOVE_SPEED, WATER_SURFACE_STEP,
};

pub fn simulate_tick(
    prev: &PlayerSimState,
    input: &InputState,
    world: &WorldCollision,
) -> PlayerSimState {
    let mut state = *prev;
    state.yaw = input.yaw;
    state.pitch = input.pitch;
    if state.jump_ticks > 0 {
        state.jump_ticks = state.jump_ticks.saturating_sub(1);
    }
    if !input.jump {
        state.jump_ticks = 0;
    }
    if !world.has_chunk_at_pos(state.pos) {
        return simulate_tick_unloaded_chunk(state, input);
    }
    let sprinting = effective_sprint(input);
    let flying = input.can_fly && input.flying;
    let in_water = world.is_player_in_water(state.pos);

    if flying {
        let fly_speed = input.flying_speed.max(0.0);
        let fly_move_speed = fly_speed * if sprinting { FLY_SPRINT_MULT } else { 1.0 };

        let wish = damped_move_input(input.strafe, input.forward, false);
        move_flying(&mut state.vel, wish.x, wish.z, fly_move_speed, state.yaw);

        if input.sneak {
            state.vel.y -= fly_speed * FLY_VERTICAL_ACCEL_MULT;
        }
        if input.jump {
            state.vel.y += fly_speed * FLY_VERTICAL_ACCEL_MULT;
        }

        let (pos, vel, on_ground, collided_horizontally) =
            world.resolve(state.pos, state.vel, state.on_ground);
        state.pos = pos;
        state.vel = vel;
        state.on_ground = on_ground;
        state.collided_horizontally = collided_horizontally;
        state.vel.x *= FLY_HORIZONTAL_DAMPING;
        state.vel.z *= FLY_HORIZONTAL_DAMPING;
        state.vel.y *= FLY_VERTICAL_DAMPING;
        return state;
    }

    let on_ground_for_move = state.on_ground;

    if !in_water && on_ground_for_move && input.jump && state.jump_ticks == 0 {
        let jump_boost = input
            .jump_boost_amplifier
            .map_or(0.0, |amp| 0.1 * (f32::from(amp) + 1.0));
        state.vel.y = JUMP_VEL + jump_boost;
        state.jump_ticks = 10;
        if sprinting {
            let (sin_yaw, cos_yaw) = state.yaw.sin_cos();
            let forward = Vec3::new(-sin_yaw, 0.0, -cos_yaw);
            state.vel.x += forward.x * 0.2;
            state.vel.z += forward.z * 0.2;
        }
    }

    let wish = damped_move_input(input.strafe, input.forward, input.sneak);
    let move_speed =
        BASE_MOVE_SPEED * input.speed_multiplier.max(0.0) * if sprinting { 1.3 } else { 1.0 };

    let f4 = if on_ground_for_move {
        world.ground_slipperiness(state.pos) * 0.91
    } else {
        0.91
    };

    let f = 0.16277136 / (f4 * f4 * f4);
    let jump_movement_factor = if sprinting {
        SPEED_IN_AIR * 1.3
    } else {
        SPEED_IN_AIR
    };
    let f5 = if in_water {
        WATER_MOVE_SPEED
    } else if on_ground_for_move {
        move_speed * f
    } else {
        jump_movement_factor
    };

    move_flying(&mut state.vel, wish.x, wish.z, f5, state.yaw);

    if on_ground_for_move && input.sneak {
        let clamped = world.clamp_sneak_edge_velocity(state.pos, state.vel);
        state.vel.x = clamped.x;
        state.vel.z = clamped.z;
    }

    let pre_move_y = state.pos.y;
    let (pos, vel, on_ground, collided_horizontally) =
        world.resolve(state.pos, state.vel, on_ground_for_move);
    state.pos = pos;
    state.vel = vel;
    state.on_ground = on_ground;
    state.collided_horizontally = collided_horizontally;

    if state.on_ground && world.is_on_soul_sand(state.pos) {
        state.vel.x *= SOUL_SAND_SLOWDOWN;
        state.vel.z *= SOUL_SAND_SLOWDOWN;
    }

    if in_water {
        if input.jump {
            state.vel.y += SWIM_UP_ACCEL;
        }
        state.vel.x *= WATER_DRAG;
        state.vel.y *= WATER_DRAG;
        state.vel.z *= WATER_DRAG;
        state.vel.y += WATER_GRAVITY;
        if collided_horizontally
            && world.is_offset_position_in_liquid(
                state.pos,
                Vec3::new(
                    state.vel.x,
                    state.vel.y + 0.6 - state.pos.y + pre_move_y,
                    state.vel.z,
                ),
            )
        {
            state.vel.y = WATER_SURFACE_STEP;
        }
    } else {
        state.vel.y += GRAVITY;
        state.vel.y *= AIR_DRAG;
        state.vel.x *= f4;
        state.vel.z *= f4;
    }
    state
}

fn simulate_tick_unloaded_chunk(mut state: PlayerSimState, input: &InputState) -> PlayerSimState {
    let sprinting = effective_sprint(input);
    let flying = input.can_fly && input.flying;

    if flying {
        let fly_speed = input.flying_speed.max(0.0);
        let fly_move_speed = fly_speed * if sprinting { FLY_SPRINT_MULT } else { 1.0 };

        let wish = damped_move_input(input.strafe, input.forward, false);

        move_flying(&mut state.vel, wish.x, wish.z, fly_move_speed, state.yaw);

        if input.sneak {
            state.vel.y -= fly_speed * FLY_VERTICAL_ACCEL_MULT;
        }
        if input.jump {
            state.vel.y += fly_speed * FLY_VERTICAL_ACCEL_MULT;
        }

        state.pos += state.vel;
        state.on_ground = false;
        state.collided_horizontally = false;
        state.vel.x *= FLY_HORIZONTAL_DAMPING;
        state.vel.z *= FLY_HORIZONTAL_DAMPING;
        state.vel.y *= FLY_VERTICAL_DAMPING;
        return state;
    }

    let on_ground_for_move = state.on_ground;

    if on_ground_for_move && input.jump && state.jump_ticks == 0 {
        let jump_boost = input
            .jump_boost_amplifier
            .map_or(0.0, |amp| 0.1 * (f32::from(amp) + 1.0));
        state.vel.y = JUMP_VEL + jump_boost;
        state.jump_ticks = 10;
        if sprinting {
            let (sin_yaw, cos_yaw) = state.yaw.sin_cos();
            let forward = Vec3::new(-sin_yaw, 0.0, -cos_yaw);
            state.vel.x += forward.x * 0.2;
            state.vel.z += forward.z * 0.2;
        }
    }

    let wish = damped_move_input(input.strafe, input.forward, input.sneak);
    let move_speed =
        BASE_MOVE_SPEED * input.speed_multiplier.max(0.0) * if sprinting { 1.3 } else { 1.0 };
    let accel = if on_ground_for_move {
        move_speed * 0.16277136 / (0.91 * 0.91 * 0.91)
    } else if sprinting {
        SPEED_IN_AIR * 1.3
    } else {
        SPEED_IN_AIR
    };
    move_flying(&mut state.vel, wish.x, wish.z, accel, state.yaw);

    state.pos.x += state.vel.x;
    state.pos.z += state.vel.z;
    state.vel.y = 0.0;
    state.on_ground = false;
    state.collided_horizontally = false;
    state.vel.x *= AIR_DRAG;
    state.vel.z *= AIR_DRAG;
    state
}

fn move_flying(vel: &mut Vec3, strafe: f32, forward: f32, friction: f32, yaw: f32) {
    let f = strafe * strafe + forward * forward;
    if f < 1.0e-4 {
        return;
    }

    let mut f = f.sqrt();
    if f < 1.0 {
        f = 1.0;
    }
    let f = friction / f;
    let strafe = strafe * f;
    let forward = forward * f;

    let (sin_yaw, cos_yaw) = yaw.sin_cos();
    let forward_dir = Vec3::new(-sin_yaw, 0.0, -cos_yaw);
    let right_dir = Vec3::new(cos_yaw, 0.0, -sin_yaw);
    let dir = right_dir * strafe + forward_dir * forward;
    vel.x += dir.x;
    vel.z += dir.z;
}

fn damped_move_input(strafe: f32, forward: f32, sneak: bool) -> Vec3 {
    let mut wish = Vec3::new(strafe, 0.0, forward);
    if wish.length_squared() > 1.0 {
        wish = wish.normalize();
    }
    if sneak {
        wish.x *= SNEAK_INPUT_SCALE;
        wish.z *= SNEAK_INPUT_SCALE;
    }
    wish.x *= MOVE_INPUT_DAMPING;
    wish.z *= MOVE_INPUT_DAMPING;
    wish
}

pub fn effective_sprint(input: &InputState) -> bool {
    input.sprint && !input.sneak && input.forward >= SPRINT_FORWARD_THRESHOLD
}
