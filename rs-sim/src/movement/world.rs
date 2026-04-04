use bevy::prelude::Vec3;
use rs_utils::block_state_id;

use super::aabb::{
    aabb_feet_position, block_range, calculate_x_offset, calculate_y_offset, calculate_z_offset,
    player_aabb, Aabb,
};
use super::block_shapes::{
    append_block_collision_boxes, block_slipperiness, is_water_state, step_toward_zero,
};
use super::{COLLISION_EPS, PLAYER_HALF_WIDTH, PLAYER_HEIGHT, PLAYER_STEP_HEIGHT};
use crate::collision::WorldCollisionMap;

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

    pub(crate) fn block_at(&self, x: i32, y: i32, z: i32) -> u16 {
        self.map.map_or(0, |map| map.block_at(x, y, z))
    }

    pub(crate) fn has_chunk_at_pos(&self, pos: Vec3) -> bool {
        let chunk_x = (pos.x.floor() as i32).div_euclid(16);
        let chunk_z = (pos.z.floor() as i32).div_euclid(16);
        self.map.is_some_and(|map| map.has_chunk(chunk_x, chunk_z))
    }

    pub(crate) fn is_player_in_water(&self, pos: Vec3) -> bool {
        let bb = player_aabb(pos)
            .offset(Vec3::new(0.0, -0.4, 0.0))
            .contract(0.001, 0.001, 0.001);
        self.aabb_has_liquid(&bb)
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

    pub fn is_supported(&self, pos: Vec3) -> bool {
        if !self.has_chunk_at_pos(pos) {
            return false;
        }
        let bb = player_aabb(pos).offset(Vec3::new(0.0, -0.001, 0.0));
        self.aabb_collides(bb.min, bb.max)
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

    pub(crate) fn is_offset_position_in_liquid(&self, pos: Vec3, offset: Vec3) -> bool {
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

    pub(crate) fn ground_slipperiness(&self, pos: Vec3) -> f32 {
        let x = pos.x.floor() as i32;
        let y = (pos.y - 1.0).floor() as i32;
        let z = pos.z.floor() as i32;
        block_slipperiness(self.block_at(x, y, z))
    }

    pub(crate) fn is_on_soul_sand(&self, pos: Vec3) -> bool {
        let y = (pos.y - 0.2).floor() as i32;
        let x0 = (pos.x - PLAYER_HALF_WIDTH).floor() as i32;
        let x1 = (pos.x + PLAYER_HALF_WIDTH).floor() as i32;
        let z0 = (pos.z - PLAYER_HALF_WIDTH).floor() as i32;
        let z1 = (pos.z + PLAYER_HALF_WIDTH).floor() as i32;
        for z in z0..=z1 {
            for x in x0..=x1 {
                if block_state_id(self.block_at(x, y, z)) == 88 {
                    return true;
                }
            }
        }
        false
    }

    pub(crate) fn clamp_sneak_edge_velocity(&self, pos: Vec3, vel: Vec3) -> Vec3 {
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

    pub(crate) fn resolve(
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
            let prev_x = x;
            let prev_y = y;
            let prev_z = z;
            let prev_bb = bb;
            let axisalignedbb = player_aabb(pos);

            bb = axisalignedbb;
            y = PLAYER_STEP_HEIGHT;
            let query = bb.add_coord(Vec3::new(original.x, y, original.z));
            boxes = self.collect_collision_boxes(query.min, query.max);

            let mut bb_a = bb;
            let bb_a_query = bb_a.add_coord(Vec3::new(original.x, 0.0, original.z));
            let mut y_a = y;
            for block in &boxes {
                y_a = calculate_y_offset(&bb_a_query, block, y_a);
            }
            bb_a = bb_a.offset(Vec3::new(0.0, y_a, 0.0));
            let mut x_a = original.x;
            for block in &boxes {
                x_a = calculate_x_offset(&bb_a, block, x_a);
            }
            bb_a = bb_a.offset(Vec3::new(x_a, 0.0, 0.0));
            let mut z_a = original.z;
            for block in &boxes {
                z_a = calculate_z_offset(&bb_a, block, z_a);
            }
            bb_a = bb_a.offset(Vec3::new(0.0, 0.0, z_a));

            let mut bb_b = bb;
            let mut y_b = y;
            for block in &boxes {
                y_b = calculate_y_offset(&bb_b, block, y_b);
            }
            bb_b = bb_b.offset(Vec3::new(0.0, y_b, 0.0));
            let mut x_b = original.x;
            for block in &boxes {
                x_b = calculate_x_offset(&bb_b, block, x_b);
            }
            bb_b = bb_b.offset(Vec3::new(x_b, 0.0, 0.0));
            let mut z_b = original.z;
            for block in &boxes {
                z_b = calculate_z_offset(&bb_b, block, z_b);
            }
            bb_b = bb_b.offset(Vec3::new(0.0, 0.0, z_b));

            let dist_a = x_a * x_a + z_a * z_a;
            let dist_b = x_b * x_b + z_b * z_b;

            if dist_a > dist_b {
                x = x_a;
                z = z_a;
                y = -y_a;
                bb = bb_a;
            } else {
                x = x_b;
                z = z_b;
                y = -y_b;
                bb = bb_b;
            }

            for block in &boxes {
                y = calculate_y_offset(&bb, block, y);
            }
            bb = bb.offset(Vec3::new(0.0, y, 0.0));

            if prev_x * prev_x + prev_z * prev_z >= x * x + z * z {
                x = prev_x;
                y = prev_y;
                z = prev_z;
                bb = prev_bb;
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
        let on_ground = original.y != y && original.y < 0.0;
        let collided_horizontally = original.x != x || original.z != z;
        (pos, vel, on_ground, collided_horizontally)
    }
}
