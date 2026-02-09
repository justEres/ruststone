use bevy::prelude::Vec3;

use super::collision::{is_solid, WorldCollisionMap};
use super::types::{InputState, PlayerSimState};

const PLAYER_HALF_WIDTH: f32 = 0.3;
const PLAYER_HEIGHT: f32 = 1.8;
const COLLISION_EPS: f32 = 1e-5;

const GRAVITY: f32 = -0.08;
const AIR_DRAG: f32 = 0.98;
const JUMP_VEL: f32 = 0.42;
const BASE_MOVE_SPEED: f32 = 0.1;
const SPEED_IN_AIR: f32 = 0.02;
const SLIPPERINESS_DEFAULT: f32 = 0.6;

pub struct WorldCollision<'a> {
    map: Option<&'a WorldCollisionMap>,
}

impl<'a> WorldCollision<'a> {
    pub fn empty() -> Self {
        Self { map: None }
    }

    pub fn with_map(map: &'a WorldCollisionMap) -> Self {
        Self { map: Some(map) }
    }

    fn block_at(&self, x: i32, y: i32, z: i32) -> u16 {
        self.map.map_or(0, |map| map.block_at(x, y, z))
    }

    fn is_solid_at(&self, x: i32, y: i32, z: i32) -> bool {
        is_solid(self.block_at(x, y, z))
    }

    fn aabb_collides(&self, min: Vec3, max: Vec3) -> bool {
        let (min_x, max_x) = block_range(min.x, max.x);
        let (min_y, max_y) = block_range(min.y, max.y);
        let (min_z, max_z) = block_range(min.z, max.z);

        for y in min_y..=max_y {
            for z in min_z..=max_z {
                for x in min_x..=max_x {
                    if self.is_solid_at(x, y, z) {
                        return true;
                    }
                }
            }
        }
        false
    }

    pub fn resolve(&self, mut pos: Vec3, mut vel: Vec3) -> (Vec3, Vec3, bool) {
        let mut on_ground = false;

        // X axis
        if vel.x.abs() > 0.0 {
            let mut new_pos = pos;
            new_pos.x += vel.x;
            let min = Vec3::new(
                new_pos.x - PLAYER_HALF_WIDTH,
                new_pos.y,
                new_pos.z - PLAYER_HALF_WIDTH,
            );
            let max = Vec3::new(
                new_pos.x + PLAYER_HALF_WIDTH,
                new_pos.y + PLAYER_HEIGHT,
                new_pos.z + PLAYER_HALF_WIDTH,
            );
            if self.aabb_collides(min, max) {
                let (min_y, max_y) = block_range(min.y, max.y);
                let (min_z, max_z) = block_range(min.z, max.z);
                let (min_x, max_x) = block_range(min.x, max.x);
                if vel.x > 0.0 {
                    let mut limit = new_pos.x;
                    for y in min_y..=max_y {
                        for z in min_z..=max_z {
                            for x in min_x..=max_x {
                                if self.is_solid_at(x, y, z) {
                                    let candidate = x as f32 - PLAYER_HALF_WIDTH;
                                    if candidate < limit {
                                        limit = candidate;
                                    }
                                }
                            }
                        }
                    }
                    new_pos.x = limit;
                } else {
                    let mut limit = new_pos.x;
                    for y in min_y..=max_y {
                        for z in min_z..=max_z {
                            for x in min_x..=max_x {
                                if self.is_solid_at(x, y, z) {
                                    let candidate = (x + 1) as f32 + PLAYER_HALF_WIDTH;
                                    if candidate > limit {
                                        limit = candidate;
                                    }
                                }
                            }
                        }
                    }
                    new_pos.x = limit;
                }
                vel.x = 0.0;
            }
            pos.x = new_pos.x;
        }

        // Z axis
        if vel.z.abs() > 0.0 {
            let mut new_pos = pos;
            new_pos.z += vel.z;
            let min = Vec3::new(
                new_pos.x - PLAYER_HALF_WIDTH,
                new_pos.y,
                new_pos.z - PLAYER_HALF_WIDTH,
            );
            let max = Vec3::new(
                new_pos.x + PLAYER_HALF_WIDTH,
                new_pos.y + PLAYER_HEIGHT,
                new_pos.z + PLAYER_HALF_WIDTH,
            );
            if self.aabb_collides(min, max) {
                let (min_y, max_y) = block_range(min.y, max.y);
                let (min_x, max_x) = block_range(min.x, max.x);
                let (min_z, max_z) = block_range(min.z, max.z);
                if vel.z > 0.0 {
                    let mut limit = new_pos.z;
                    for y in min_y..=max_y {
                        for x in min_x..=max_x {
                            for z in min_z..=max_z {
                                if self.is_solid_at(x, y, z) {
                                    let candidate = z as f32 - PLAYER_HALF_WIDTH;
                                    if candidate < limit {
                                        limit = candidate;
                                    }
                                }
                            }
                        }
                    }
                    new_pos.z = limit;
                } else {
                    let mut limit = new_pos.z;
                    for y in min_y..=max_y {
                        for x in min_x..=max_x {
                            for z in min_z..=max_z {
                                if self.is_solid_at(x, y, z) {
                                    let candidate = (z + 1) as f32 + PLAYER_HALF_WIDTH;
                                    if candidate > limit {
                                        limit = candidate;
                                    }
                                }
                            }
                        }
                    }
                    new_pos.z = limit;
                }
                vel.z = 0.0;
            }
            pos.z = new_pos.z;
        }

        // Y axis
        if vel.y.abs() > 0.0 {
            let mut new_pos = pos;
            new_pos.y += vel.y;
            let min = Vec3::new(
                new_pos.x - PLAYER_HALF_WIDTH,
                new_pos.y,
                new_pos.z - PLAYER_HALF_WIDTH,
            );
            let max = Vec3::new(
                new_pos.x + PLAYER_HALF_WIDTH,
                new_pos.y + PLAYER_HEIGHT,
                new_pos.z + PLAYER_HALF_WIDTH,
            );
            if self.aabb_collides(min, max) {
                let (min_z, max_z) = block_range(min.z, max.z);
                let (min_x, max_x) = block_range(min.x, max.x);
                let (min_y, max_y) = block_range(min.y, max.y);
                if vel.y > 0.0 {
                    let mut limit = new_pos.y;
                    for x in min_x..=max_x {
                        for z in min_z..=max_z {
                            for y in min_y..=max_y {
                                if self.is_solid_at(x, y, z) {
                                    let candidate = y as f32 - PLAYER_HEIGHT;
                                    if candidate < limit {
                                        limit = candidate;
                                    }
                                }
                            }
                        }
                    }
                    new_pos.y = limit;
                } else {
                    let mut limit = new_pos.y;
                    for x in min_x..=max_x {
                        for z in min_z..=max_z {
                            for y in min_y..=max_y {
                                if self.is_solid_at(x, y, z) {
                                    let candidate = (y + 1) as f32;
                                    if candidate > limit {
                                        limit = candidate;
                                    }
                                }
                            }
                        }
                    }
                    new_pos.y = limit;
                    on_ground = true;
                }
                vel.y = 0.0;
            }
            pos.y = new_pos.y;
        }

        if !on_ground {
            let min = Vec3::new(
                pos.x - PLAYER_HALF_WIDTH,
                pos.y - 0.02,
                pos.z - PLAYER_HALF_WIDTH,
            );
            let max = Vec3::new(
                pos.x + PLAYER_HALF_WIDTH,
                pos.y - 0.001,
                pos.z + PLAYER_HALF_WIDTH,
            );
            if self.aabb_collides(min, max) {
                on_ground = true;
            }
        }

        (pos, vel, on_ground)
    }
}

fn block_range(min: f32, max: f32) -> (i32, i32) {
    let min_i = (min + COLLISION_EPS).floor() as i32;
    let max_i = (max - COLLISION_EPS).floor() as i32;
    if min_i <= max_i {
        (min_i, max_i)
    } else {
        (max_i, min_i)
    }
}

pub fn simulate_tick(
    prev: &PlayerSimState,
    input: &InputState,
    world: &WorldCollision,
) -> PlayerSimState {
    let mut state = *prev;
    state.yaw = input.yaw;
    state.pitch = input.pitch;

    if state.on_ground && input.jump {
        state.vel.y = JUMP_VEL;
        state.on_ground = false;
        if input.sprint {
            let (sin_yaw, cos_yaw) = state.yaw.sin_cos();
            state.vel.x -= sin_yaw * 0.2;
            state.vel.z += cos_yaw * 0.2;
        }
    }

    let mut wish = Vec3::new(input.strafe, 0.0, input.forward);
    if wish.length_squared() > 1.0 {
        wish = wish.normalize();
    }
    if input.sneak {
        wish.x *= 0.3;
        wish.z *= 0.3;
    }

    let move_speed = BASE_MOVE_SPEED * if input.sprint { 1.3 } else { 1.0 };

    let mut f4 = 0.91f32;
    if state.on_ground {
        f4 = SLIPPERINESS_DEFAULT * 0.91;
    }

    let f = 0.16277136 / (f4 * f4 * f4);
    let f5 = if state.on_ground {
        move_speed * f
    } else {
        SPEED_IN_AIR * if input.sprint { 1.3 } else { 1.0 }
    };

    move_flying(&mut state.vel, wish.x, wish.z, f5, state.yaw);

    let (pos, vel, on_ground) = world.resolve(state.pos, state.vel);
    state.pos = pos;
    state.vel = vel;
    state.on_ground = on_ground;

    state.vel.y += GRAVITY;
    state.vel.y *= AIR_DRAG;
    state.vel.x *= f4;
    state.vel.z *= f4;
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
