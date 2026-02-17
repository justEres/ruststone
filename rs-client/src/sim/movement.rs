use bevy::prelude::Vec3;
use rs_utils::{BlockModelKind, block_model_kind, block_state_id, block_state_meta};

use super::collision::{WorldCollisionMap, is_solid};
use super::types::{InputState, PlayerSimState};

const PLAYER_HALF_WIDTH: f32 = 0.3;
const PLAYER_HEIGHT: f32 = 1.8;
const PLAYER_STEP_HEIGHT: f32 = 0.6;
const COLLISION_EPS: f32 = 1e-5;

const GRAVITY: f32 = -0.08;
const AIR_DRAG: f32 = 0.98;
const WATER_GRAVITY: f32 = -0.02;
const WATER_DRAG: f32 = 0.8;
const WATER_SURFACE_STEP: f32 = 0.3;
const JUMP_VEL: f32 = 0.42;
const BASE_MOVE_SPEED: f32 = 0.1;
const SPEED_IN_AIR: f32 = 0.02;
const WATER_MOVE_SPEED: f32 = 0.02;
const SWIM_UP_ACCEL: f32 = 0.04;
const SLIPPERINESS_DEFAULT: f32 = 0.6;
const SNEAK_EDGE_STEP: f32 = 0.05;
const MOVE_INPUT_DAMPING: f32 = 0.98;
const SNEAK_INPUT_SCALE: f32 = 0.3;
const SPRINT_FORWARD_THRESHOLD: f32 = 0.8;
const FLY_VERTICAL_ACCEL_MULT: f32 = 3.0;
const FLY_HORIZONTAL_DAMPING: f32 = 0.91;
const FLY_VERTICAL_DAMPING: f32 = 0.6;
const FLY_SPRINT_MULT: f32 = 2.0;

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

    fn is_player_in_water(&self, pos: Vec3) -> bool {
        let x = pos.x.floor() as i32;
        let z = pos.z.floor() as i32;
        let y0 = (pos.y + 0.2).floor() as i32;
        let y1 = (pos.y + 0.9).floor() as i32;
        let y2 = (pos.y + 1.4).floor() as i32;
        is_water_state(self.block_at(x, y0, z))
            || is_water_state(self.block_at(x, y1, z))
            || is_water_state(self.block_at(x, y2, z))
    }

    fn collect_collision_boxes(&self, min: Vec3, max: Vec3) -> Vec<Aabb> {
        let (min_x, max_x) = block_range(min.x, max.x);
        let (min_y, max_y) = block_range(min.y, max.y);
        let (min_z, max_z) = block_range(min.z, max.z);
        let mut out = Vec::new();
        for y in min_y..=max_y {
            for z in min_z..=max_z {
                for x in min_x..=max_x {
                    let block_state = self.block_at(x, y, z);
                    append_block_collision_boxes(self, block_state, x, y, z, &mut out);
                }
            }
        }
        out
    }

    fn aabb_collides(&self, min: Vec3, max: Vec3) -> bool {
        let query = Aabb::new(min, max);
        for block in self.collect_collision_boxes(min, max) {
            if query.intersects(&block) {
                return true;
            }
        }
        false
    }

    fn aabb_has_liquid(&self, bb: &Aabb) -> bool {
        let (min_x, max_x) = block_range(bb.min.x, bb.max.x);
        let (min_y, max_y) = block_range(bb.min.y, bb.max.y);
        let (min_z, max_z) = block_range(bb.min.z, bb.max.z);
        for y in min_y..=max_y {
            for z in min_z..=max_z {
                for x in min_x..=max_x {
                    let block_state = self.block_at(x, y, z);
                    if !is_water_state(block_state) {
                        continue;
                    }
                    let liquid_bb = Aabb::new(
                        Vec3::new(x as f32, y as f32, z as f32),
                        Vec3::new(x as f32 + 1.0, y as f32 + 1.0, z as f32 + 1.0),
                    );
                    if bb.intersects(&liquid_bb) {
                        return true;
                    }
                }
            }
        }
        false
    }

    // Vanilla parity helper for Entity::isOffsetPositionInLiquid.
    fn is_offset_position_in_liquid(&self, pos: Vec3, offset: Vec3) -> bool {
        let bb = player_aabb(pos).offset(offset);
        !self.aabb_collides(bb.min, bb.max) && !self.aabb_has_liquid(&bb)
    }

    fn has_support_one_block_down(&self, pos: Vec3) -> bool {
        let min = Vec3::new(
            pos.x - PLAYER_HALF_WIDTH,
            pos.y - 1.0,
            pos.z - PLAYER_HALF_WIDTH,
        );
        let max = Vec3::new(
            pos.x + PLAYER_HALF_WIDTH,
            pos.y + PLAYER_HEIGHT - 1.0,
            pos.z + PLAYER_HALF_WIDTH,
        );
        self.aabb_collides(min, max)
    }

    pub fn clamp_sneak_edge_velocity(&self, pos: Vec3, vel: Vec3) -> Vec3 {
        if self.map.is_none() {
            return vel;
        }

        let mut dx = vel.x;
        let mut dz = vel.z;

        while dx.abs() > COLLISION_EPS
            && !self.has_support_one_block_down(pos + Vec3::new(dx, 0.0, 0.0))
        {
            dx = step_toward_zero(dx);
        }

        while dz.abs() > COLLISION_EPS
            && !self.has_support_one_block_down(pos + Vec3::new(0.0, 0.0, dz))
        {
            dz = step_toward_zero(dz);
        }

        while dx.abs() > COLLISION_EPS
            && dz.abs() > COLLISION_EPS
            && !self.has_support_one_block_down(pos + Vec3::new(dx, 0.0, dz))
        {
            dx = step_toward_zero(dx);
            dz = step_toward_zero(dz);
        }

        Vec3::new(dx, vel.y, dz)
    }

    pub fn resolve(
        &self,
        mut pos: Vec3,
        mut vel: Vec3,
        was_on_ground: bool,
    ) -> (Vec3, Vec3, bool, bool) {
        let original = vel;
        let mut bb = player_aabb(pos);

        let broadphase = bb.expanded_by_motion(vel);
        let mut boxes = self.collect_collision_boxes(broadphase.min, broadphase.max);

        let mut y = vel.y;
        for block in &boxes {
            y = calculate_y_offset(&bb, block, y);
        }
        bb = bb.offset(Vec3::new(0.0, y, 0.0));

        let mut x = vel.x;
        for block in &boxes {
            x = calculate_x_offset(&bb, block, x);
        }
        bb = bb.offset(Vec3::new(x, 0.0, 0.0));

        let mut z = vel.z;
        for block in &boxes {
            z = calculate_z_offset(&bb, block, z);
        }
        bb = bb.offset(Vec3::new(0.0, 0.0, z));

        let stepped_down = original.y != y && original.y < 0.0;
        let horizontal_blocked = original.x != x || original.z != z;

        if PLAYER_STEP_HEIGHT > 0.0 && (was_on_ground || stepped_down) && horizontal_blocked {
            let start = player_aabb(pos);
            let mut stepped = start;

            let step_motion = Vec3::new(original.x, PLAYER_STEP_HEIGHT, original.z);
            boxes = self.collect_collision_boxes(
                stepped.expanded_by_motion(step_motion).min,
                stepped.expanded_by_motion(step_motion).max,
            );

            let mut up = PLAYER_STEP_HEIGHT;
            for block in &boxes {
                up = calculate_y_offset(&stepped, block, up);
            }
            stepped = stepped.offset(Vec3::new(0.0, up, 0.0));

            let mut step_x = original.x;
            for block in &boxes {
                step_x = calculate_x_offset(&stepped, block, step_x);
            }
            stepped = stepped.offset(Vec3::new(step_x, 0.0, 0.0));

            let mut step_z = original.z;
            for block in &boxes {
                step_z = calculate_z_offset(&stepped, block, step_z);
            }
            stepped = stepped.offset(Vec3::new(0.0, 0.0, step_z));

            let mut down = -up;
            for block in &boxes {
                down = calculate_y_offset(&stepped, block, down);
            }
            stepped = stepped.offset(Vec3::new(0.0, down, 0.0));

            let step_dist_sq = step_x * step_x + step_z * step_z;
            let flat_dist_sq = x * x + z * z;
            if step_dist_sq > flat_dist_sq + 1.0e-6 {
                bb = stepped;
                x = step_x;
                y = up + down;
                z = step_z;
            }
        }

        if original.x != x {
            vel.x = 0.0;
        } else {
            vel.x = x;
        }
        if original.y != y {
            vel.y = 0.0;
        } else {
            vel.y = y;
        }
        if original.z != z {
            vel.z = 0.0;
        } else {
            vel.z = z;
        }

        pos = aabb_feet_position(bb);
        let mut on_ground = original.y != y && original.y < 0.0;
        if !on_ground && original.y <= 0.0 {
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

        let collided_horizontally = original.x != x || original.z != z;
        (pos, vel, on_ground, collided_horizontally)
    }
}

#[derive(Clone, Copy, Debug)]
struct Aabb {
    min: Vec3,
    max: Vec3,
}

impl Aabb {
    fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    fn offset(self, delta: Vec3) -> Self {
        Self {
            min: self.min + delta,
            max: self.max + delta,
        }
    }

    fn expanded_by_motion(self, motion: Vec3) -> Self {
        let min = Vec3::new(
            self.min.x.min(self.min.x + motion.x),
            self.min.y.min(self.min.y + motion.y),
            self.min.z.min(self.min.z + motion.z),
        );
        let max = Vec3::new(
            self.max.x.max(self.max.x + motion.x),
            self.max.y.max(self.max.y + motion.y),
            self.max.z.max(self.max.z + motion.z),
        );
        Self { min, max }
    }

    fn intersects(&self, other: &Aabb) -> bool {
        self.max.x > other.min.x
            && self.min.x < other.max.x
            && self.max.y > other.min.y
            && self.min.y < other.max.y
            && self.max.z > other.min.z
            && self.min.z < other.max.z
    }
}

fn player_aabb(pos: Vec3) -> Aabb {
    Aabb::new(
        Vec3::new(pos.x - PLAYER_HALF_WIDTH, pos.y, pos.z - PLAYER_HALF_WIDTH),
        Vec3::new(
            pos.x + PLAYER_HALF_WIDTH,
            pos.y + PLAYER_HEIGHT,
            pos.z + PLAYER_HALF_WIDTH,
        ),
    )
}

fn aabb_feet_position(aabb: Aabb) -> Vec3 {
    Vec3::new(
        (aabb.min.x + aabb.max.x) * 0.5,
        aabb.min.y,
        (aabb.min.z + aabb.max.z) * 0.5,
    )
}

fn overlap_xz(a: &Aabb, b: &Aabb) -> bool {
    a.max.x > b.min.x && a.min.x < b.max.x && a.max.z > b.min.z && a.min.z < b.max.z
}

fn overlap_yz(a: &Aabb, b: &Aabb) -> bool {
    a.max.y > b.min.y && a.min.y < b.max.y && a.max.z > b.min.z && a.min.z < b.max.z
}

fn overlap_xy(a: &Aabb, b: &Aabb) -> bool {
    a.max.x > b.min.x && a.min.x < b.max.x && a.max.y > b.min.y && a.min.y < b.max.y
}

fn calculate_y_offset(entity: &Aabb, block: &Aabb, mut dy: f32) -> f32 {
    if !overlap_xz(entity, block) {
        return dy;
    }
    if dy > 0.0 && entity.max.y <= block.min.y {
        dy = dy.min(block.min.y - entity.max.y);
    } else if dy < 0.0 && entity.min.y >= block.max.y {
        dy = dy.max(block.max.y - entity.min.y);
    }
    dy
}

fn calculate_x_offset(entity: &Aabb, block: &Aabb, mut dx: f32) -> f32 {
    if !overlap_yz(entity, block) {
        return dx;
    }
    if dx > 0.0 && entity.max.x <= block.min.x {
        dx = dx.min(block.min.x - entity.max.x);
    } else if dx < 0.0 && entity.min.x >= block.max.x {
        dx = dx.max(block.max.x - entity.min.x);
    }
    dx
}

fn calculate_z_offset(entity: &Aabb, block: &Aabb, mut dz: f32) -> f32 {
    if !overlap_xy(entity, block) {
        return dz;
    }
    if dz > 0.0 && entity.max.z <= block.min.z {
        dz = dz.min(block.min.z - entity.max.z);
    } else if dz < 0.0 && entity.min.z >= block.max.z {
        dz = dz.max(block.max.z - entity.min.z);
    }
    dz
}

fn append_box(
    block_x: i32,
    block_y: i32,
    block_z: i32,
    local_min: [f32; 3],
    local_max: [f32; 3],
    out: &mut Vec<Aabb>,
) {
    out.push(Aabb::new(
        Vec3::new(
            block_x as f32 + local_min[0],
            block_y as f32 + local_min[1],
            block_z as f32 + local_min[2],
        ),
        Vec3::new(
            block_x as f32 + local_max[0],
            block_y as f32 + local_max[1],
            block_z as f32 + local_max[2],
        ),
    ));
}

fn append_stair_boxes(
    block_state: u16,
    block_x: i32,
    block_y: i32,
    block_z: i32,
    out: &mut Vec<Aabb>,
) {
    let meta = block_state_meta(block_state);
    let top = (meta & 0x4) != 0;
    let facing = meta & 0x3;

    if top {
        append_box(
            block_x,
            block_y,
            block_z,
            [0.0, 0.5, 0.0],
            [1.0, 1.0, 1.0],
            out,
        );
    } else {
        append_box(
            block_x,
            block_y,
            block_z,
            [0.0, 0.0, 0.0],
            [1.0, 0.5, 1.0],
            out,
        );
    }

    let (min_x, max_x, min_z, max_z) = match facing {
        0 => (0.5, 1.0, 0.0, 1.0), // east
        1 => (0.0, 0.5, 0.0, 1.0), // west
        2 => (0.0, 1.0, 0.5, 1.0), // south
        _ => (0.0, 1.0, 0.0, 0.5), // north
    };
    if top {
        append_box(
            block_x,
            block_y,
            block_z,
            [min_x, 0.0, min_z],
            [max_x, 0.5, max_z],
            out,
        );
    } else {
        append_box(
            block_x,
            block_y,
            block_z,
            [min_x, 0.5, min_z],
            [max_x, 1.0, max_z],
            out,
        );
    }
}

fn append_block_collision_boxes(
    world: &WorldCollision,
    block_state: u16,
    block_x: i32,
    block_y: i32,
    block_z: i32,
    out: &mut Vec<Aabb>,
) {
    if !is_solid(block_state) {
        return;
    }
    let block_id = block_state_id(block_state);
    let meta = block_state_meta(block_state);
    match block_model_kind(block_id) {
        BlockModelKind::Slab => {
            if (meta & 0x8) != 0 {
                append_box(
                    block_x,
                    block_y,
                    block_z,
                    [0.0, 0.5, 0.0],
                    [1.0, 1.0, 1.0],
                    out,
                );
            } else {
                append_box(
                    block_x,
                    block_y,
                    block_z,
                    [0.0, 0.0, 0.0],
                    [1.0, 0.5, 1.0],
                    out,
                );
            }
        }
        BlockModelKind::Stairs => append_stair_boxes(block_state, block_x, block_y, block_z, out),
        BlockModelKind::Fence => {
            let connect_east = fence_connects_to(world.block_at(block_x + 1, block_y, block_z));
            let connect_west = fence_connects_to(world.block_at(block_x - 1, block_y, block_z));
            let connect_south = fence_connects_to(world.block_at(block_x, block_y, block_z + 1));
            let connect_north = fence_connects_to(world.block_at(block_x, block_y, block_z - 1));

            append_box(
                block_x,
                block_y,
                block_z,
                [0.375, 0.0, 0.375],
                [0.625, 1.5, 0.625],
                out,
            );
            if connect_north {
                append_box(
                    block_x,
                    block_y,
                    block_z,
                    [0.4375, 0.0, 0.0],
                    [0.5625, 1.5, 0.5],
                    out,
                );
            }
            if connect_south {
                append_box(
                    block_x,
                    block_y,
                    block_z,
                    [0.4375, 0.0, 0.5],
                    [0.5625, 1.5, 1.0],
                    out,
                );
            }
            if connect_west {
                append_box(
                    block_x,
                    block_y,
                    block_z,
                    [0.0, 0.0, 0.4375],
                    [0.5, 1.5, 0.5625],
                    out,
                );
            }
            if connect_east {
                append_box(
                    block_x,
                    block_y,
                    block_z,
                    [0.5, 0.0, 0.4375],
                    [1.0, 1.5, 0.5625],
                    out,
                );
            }
        }
        BlockModelKind::Pane => {
            let connect_east = pane_connects_to(world.block_at(block_x + 1, block_y, block_z));
            let connect_west = pane_connects_to(world.block_at(block_x - 1, block_y, block_z));
            let connect_south = pane_connects_to(world.block_at(block_x, block_y, block_z + 1));
            let connect_north = pane_connects_to(world.block_at(block_x, block_y, block_z - 1));
            let has_x = connect_east || connect_west;
            let has_z = connect_north || connect_south;
            if !has_x || !has_z {
                append_box(
                    block_x,
                    block_y,
                    block_z,
                    [0.4375, 0.0, 0.4375],
                    [0.5625, 1.0, 0.5625],
                    out,
                );
            }
            if connect_north {
                append_box(
                    block_x,
                    block_y,
                    block_z,
                    [0.4375, 0.0, 0.0],
                    [0.5625, 1.0, 0.5],
                    out,
                );
            }
            if connect_south {
                append_box(
                    block_x,
                    block_y,
                    block_z,
                    [0.4375, 0.0, 0.5],
                    [0.5625, 1.0, 1.0],
                    out,
                );
            }
            if connect_west {
                append_box(
                    block_x,
                    block_y,
                    block_z,
                    [0.0, 0.0, 0.4375],
                    [0.5, 1.0, 0.5625],
                    out,
                );
            }
            if connect_east {
                append_box(
                    block_x,
                    block_y,
                    block_z,
                    [0.5, 0.0, 0.4375],
                    [1.0, 1.0, 0.5625],
                    out,
                );
            }
        }
        _ => append_box(
            block_x,
            block_y,
            block_z,
            [0.0, 0.0, 0.0],
            [1.0, 1.0, 1.0],
            out,
        ),
    }
}

fn fence_connects_to(neighbor_state: u16) -> bool {
    let neighbor_id = block_state_id(neighbor_state);
    if neighbor_id == 0 || matches!(neighbor_id, 8 | 9 | 10 | 11) {
        return false;
    }
    if matches!(block_model_kind(neighbor_id), BlockModelKind::Fence) {
        return true;
    }
    if matches!(neighbor_id, 107 | 183 | 184 | 185 | 186 | 187) {
        return true;
    }
    is_solid(neighbor_state)
}

fn pane_connects_to(neighbor_state: u16) -> bool {
    let neighbor_id = block_state_id(neighbor_state);
    if neighbor_id == 0 || matches!(neighbor_id, 8 | 9 | 10 | 11) {
        return false;
    }
    if matches!(block_model_kind(neighbor_id), BlockModelKind::Pane) {
        return true;
    }
    if matches!(neighbor_id, 20 | 95 | 101 | 102 | 160) {
        return true;
    }
    is_solid(neighbor_state)
}

fn is_water_state(block_state: u16) -> bool {
    matches!(block_state_id(block_state), 8 | 9)
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
    let sprinting = effective_sprint(input);
    let flying = input.can_fly && input.flying;
    let in_water = world.is_player_in_water(state.pos);

    if flying {
        let fly_speed = input.flying_speed.max(0.0);
        let fly_move_speed = fly_speed * if sprinting { FLY_SPRINT_MULT } else { 1.0 };

        let mut wish = Vec3::new(
            input.strafe * MOVE_INPUT_DAMPING,
            0.0,
            input.forward * MOVE_INPUT_DAMPING,
        );
        if wish.length_squared() > 1.0 {
            wish = wish.normalize();
        }

        move_flying(&mut state.vel, wish.x, wish.z, fly_move_speed, state.yaw);

        if input.sneak {
            state.vel.y -= fly_speed * FLY_VERTICAL_ACCEL_MULT;
        }
        if input.jump {
            state.vel.y += fly_speed * FLY_VERTICAL_ACCEL_MULT;
        }

        let (pos, vel, on_ground, _) = world.resolve(state.pos, state.vel, state.on_ground);
        state.pos = pos;
        state.vel = vel;
        state.on_ground = on_ground;

        state.vel.x *= FLY_HORIZONTAL_DAMPING;
        state.vel.z *= FLY_HORIZONTAL_DAMPING;
        state.vel.y *= FLY_VERTICAL_DAMPING;
        return state;
    }

    if !in_water && state.on_ground && input.jump {
        state.vel.y = JUMP_VEL;
        state.on_ground = false;
        if sprinting {
            let (sin_yaw, cos_yaw) = state.yaw.sin_cos();
            let forward = Vec3::new(-sin_yaw, 0.0, -cos_yaw);
            state.vel.x += forward.x * 0.2;
            state.vel.z += forward.z * 0.2;
        }
    }

    let mut wish = Vec3::new(
        input.strafe * MOVE_INPUT_DAMPING,
        0.0,
        input.forward * MOVE_INPUT_DAMPING,
    );
    if wish.length_squared() > 1.0 {
        wish = wish.normalize();
    }
    if input.sneak {
        wish.x *= SNEAK_INPUT_SCALE;
        wish.z *= SNEAK_INPUT_SCALE;
    }

    let move_speed = BASE_MOVE_SPEED * if sprinting { 1.3 } else { 1.0 };

    let mut f4 = 0.91f32;
    if state.on_ground {
        f4 = SLIPPERINESS_DEFAULT * 0.91;
    }

    let f = 0.16277136 / (f4 * f4 * f4);
    let f5 = if in_water {
        WATER_MOVE_SPEED
    } else if state.on_ground {
        move_speed * f
    } else {
        SPEED_IN_AIR * if sprinting { 1.3 } else { 1.0 }
    };

    move_flying(&mut state.vel, wish.x, wish.z, f5, state.yaw);

    if state.on_ground && input.sneak {
        let clamped = world.clamp_sneak_edge_velocity(state.pos, state.vel);
        state.vel.x = clamped.x;
        state.vel.z = clamped.z;
    }

    let pre_move_y = state.pos.y;
    let (pos, vel, on_ground, collided_horizontally) =
        world.resolve(state.pos, state.vel, state.on_ground);
    state.pos = pos;
    state.vel = vel;
    state.on_ground = on_ground;

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

pub fn effective_sprint(input: &InputState) -> bool {
    input.sprint && !input.sneak && input.forward >= SPRINT_FORWARD_THRESHOLD
}

fn step_toward_zero(v: f32) -> f32 {
    if v > 0.0 {
        (v - SNEAK_EDGE_STEP).max(0.0)
    } else {
        (v + SNEAK_EDGE_STEP).min(0.0)
    }
}
