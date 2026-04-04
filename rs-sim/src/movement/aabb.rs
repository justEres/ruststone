use bevy::prelude::Vec3;

use super::{COLLISION_EPS, PLAYER_HALF_WIDTH, PLAYER_HEIGHT};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Aabb {
    pub(crate) min: Vec3,
    pub(crate) max: Vec3,
}

impl Aabb {
    pub(crate) fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    pub(crate) fn offset(self, delta: Vec3) -> Self {
        Self {
            min: self.min + delta,
            max: self.max + delta,
        }
    }

    pub(crate) fn expanded_by_motion(self, motion: Vec3) -> Self {
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

    pub(crate) fn add_coord(self, delta: Vec3) -> Self {
        let mut min = self.min;
        let mut max = self.max;
        if delta.x < 0.0 {
            min.x += delta.x;
        } else if delta.x > 0.0 {
            max.x += delta.x;
        }
        if delta.y < 0.0 {
            min.y += delta.y;
        } else if delta.y > 0.0 {
            max.y += delta.y;
        }
        if delta.z < 0.0 {
            min.z += delta.z;
        } else if delta.z > 0.0 {
            max.z += delta.z;
        }
        Self { min, max }
    }

    pub(crate) fn contract(self, x: f32, y: f32, z: f32) -> Self {
        Self {
            min: Vec3::new(self.min.x + x, self.min.y + y, self.min.z + z),
            max: Vec3::new(self.max.x - x, self.max.y - y, self.max.z - z),
        }
    }

    pub(crate) fn intersects(&self, other: &Aabb) -> bool {
        self.max.x > other.min.x
            && self.min.x < other.max.x
            && self.max.y > other.min.y
            && self.min.y < other.max.y
            && self.max.z > other.min.z
            && self.min.z < other.max.z
    }
}

pub(crate) fn player_aabb(pos: Vec3) -> Aabb {
    Aabb::new(
        Vec3::new(pos.x - PLAYER_HALF_WIDTH, pos.y, pos.z - PLAYER_HALF_WIDTH),
        Vec3::new(
            pos.x + PLAYER_HALF_WIDTH,
            pos.y + PLAYER_HEIGHT,
            pos.z + PLAYER_HALF_WIDTH,
        ),
    )
}

pub(crate) fn aabb_feet_position(aabb: Aabb) -> Vec3 {
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

pub(crate) fn calculate_y_offset(entity: &Aabb, block: &Aabb, mut dy: f32) -> f32 {
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

pub(crate) fn calculate_x_offset(entity: &Aabb, block: &Aabb, mut dx: f32) -> f32 {
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

pub(crate) fn calculate_z_offset(entity: &Aabb, block: &Aabb, mut dz: f32) -> f32 {
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

pub(crate) fn block_range(min: f32, max: f32) -> (i32, i32) {
    let min_i = (min + COLLISION_EPS).floor() as i32;
    let max_i = (max - COLLISION_EPS).floor() as i32;
    if min_i <= max_i {
        (min_i, max_i)
    } else {
        (max_i, min_i)
    }
}
