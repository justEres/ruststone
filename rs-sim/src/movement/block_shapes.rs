use bevy::prelude::Vec3;
use rs_utils::{block_model_kind, block_state_id, block_state_meta, BlockModelKind};

use super::aabb::Aabb;
use super::world::WorldCollision;
use super::{
    SLIPPERINESS_DEFAULT, SLIPPERINESS_ICE, SLIPPERINESS_SLIME, SNEAK_EDGE_STEP,
};
use crate::collision::is_solid;

pub(crate) fn append_box(
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StairDir {
    East,
    West,
    South,
    North,
}

impl StairDir {
    fn from_meta(meta: u8) -> Self {
        match meta & 0x3 {
            0 => Self::East,
            1 => Self::West,
            2 => Self::South,
            _ => Self::North,
        }
    }

    fn opposite(self) -> Self {
        match self {
            Self::East => Self::West,
            Self::West => Self::East,
            Self::South => Self::North,
            Self::North => Self::South,
        }
    }

    fn left(self) -> Self {
        match self {
            Self::East => Self::North,
            Self::West => Self::South,
            Self::South => Self::East,
            Self::North => Self::West,
        }
    }

    fn offset(self) -> (i32, i32) {
        match self {
            Self::East => (1, 0),
            Self::West => (-1, 0),
            Self::South => (0, 1),
            Self::North => (0, -1),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StairShape {
    Straight,
    OuterLeft,
    OuterRight,
    InnerLeft,
    InnerRight,
}

fn stair_info(block_state: u16) -> Option<(StairDir, bool)> {
    if !matches!(
        block_model_kind(block_state_id(block_state)),
        BlockModelKind::Stairs
    ) {
        return None;
    }
    let meta = block_state_meta(block_state);
    Some((StairDir::from_meta(meta), (meta & 0x4) != 0))
}

fn stair_state_at(world: &WorldCollision, x: i32, y: i32, z: i32, dir: StairDir) -> u16 {
    let (dx, dz) = dir.offset();
    world.block_at(x + dx, y, z + dz)
}

fn different_stair_at(
    world: &WorldCollision,
    x: i32,
    y: i32,
    z: i32,
    dir: StairDir,
    current_dir: StairDir,
    current_top: bool,
) -> bool {
    let neighbor = stair_state_at(world, x, y, z, dir);
    let Some((neighbor_dir, neighbor_top)) = stair_info(neighbor) else {
        return true;
    };
    neighbor_dir != current_dir || neighbor_top != current_top
}

fn stair_shape(
    world: &WorldCollision,
    block_state: u16,
    block_x: i32,
    block_y: i32,
    block_z: i32,
) -> StairShape {
    let Some((facing, top)) = stair_info(block_state) else {
        return StairShape::Straight;
    };

    let front = stair_state_at(world, block_x, block_y, block_z, facing);
    if let Some((front_facing, front_top)) = stair_info(front)
        && front_top == top
        && front_facing != facing
        && front_facing != facing.opposite()
        && different_stair_at(
            world,
            block_x,
            block_y,
            block_z,
            front_facing.opposite(),
            facing,
            top,
        )
    {
        return if front_facing == facing.left() {
            StairShape::OuterLeft
        } else {
            StairShape::OuterRight
        };
    }

    let back = stair_state_at(world, block_x, block_y, block_z, facing.opposite());
    if let Some((back_facing, back_top)) = stair_info(back)
        && back_top == top
        && back_facing != facing
        && back_facing != facing.opposite()
        && different_stair_at(world, block_x, block_y, block_z, back_facing, facing, top)
    {
        return if back_facing == facing.left() {
            StairShape::InnerLeft
        } else {
            StairShape::InnerRight
        };
    }

    StairShape::Straight
}

fn stair_straight_rect(facing: StairDir) -> (f32, f32, f32, f32) {
    match facing {
        StairDir::East => (0.5, 1.0, 0.0, 1.0),
        StairDir::West => (0.0, 0.5, 0.0, 1.0),
        StairDir::South => (0.0, 1.0, 0.5, 1.0),
        StairDir::North => (0.0, 1.0, 0.0, 0.5),
    }
}

fn stair_outer_rect(facing: StairDir, left: bool) -> (f32, f32, f32, f32) {
    match (facing, left) {
        (StairDir::East, true) => (0.5, 1.0, 0.0, 0.5),
        (StairDir::East, false) => (0.5, 1.0, 0.5, 1.0),
        (StairDir::West, true) => (0.0, 0.5, 0.5, 1.0),
        (StairDir::West, false) => (0.0, 0.5, 0.0, 0.5),
        (StairDir::South, true) => (0.5, 1.0, 0.5, 1.0),
        (StairDir::South, false) => (0.0, 0.5, 0.5, 1.0),
        (StairDir::North, true) => (0.0, 0.5, 0.0, 0.5),
        (StairDir::North, false) => (0.5, 1.0, 0.0, 0.5),
    }
}

fn stair_inner_extra_rect(facing: StairDir, left: bool) -> (f32, f32, f32, f32) {
    match (facing, left) {
        (StairDir::East, true) => (0.0, 0.5, 0.0, 0.5),
        (StairDir::East, false) => (0.0, 0.5, 0.5, 1.0),
        (StairDir::West, true) => (0.5, 1.0, 0.5, 1.0),
        (StairDir::West, false) => (0.5, 1.0, 0.0, 0.5),
        (StairDir::South, true) => (0.5, 1.0, 0.0, 0.5),
        (StairDir::South, false) => (0.0, 0.5, 0.0, 0.5),
        (StairDir::North, true) => (0.0, 0.5, 0.5, 1.0),
        (StairDir::North, false) => (0.5, 1.0, 0.5, 1.0),
    }
}

fn append_stair_boxes(
    world: &WorldCollision,
    block_state: u16,
    block_x: i32,
    block_y: i32,
    block_z: i32,
    out: &mut Vec<Aabb>,
) {
    let meta = block_state_meta(block_state);
    let top = (meta & 0x4) != 0;
    let facing = StairDir::from_meta(meta);
    let shape = stair_shape(world, block_state, block_x, block_y, block_z);

    if top {
        append_box(block_x, block_y, block_z, [0.0, 0.5, 0.0], [1.0, 1.0, 1.0], out);
    } else {
        append_box(block_x, block_y, block_z, [0.0, 0.0, 0.0], [1.0, 0.5, 1.0], out);
    }

    let (min_y, max_y) = if top { (0.0, 0.5) } else { (0.5, 1.0) };
    let mut push_riser = |rect: (f32, f32, f32, f32)| {
        append_box(
            block_x,
            block_y,
            block_z,
            [rect.0, min_y, rect.2],
            [rect.1, max_y, rect.3],
            out,
        );
    };

    match shape {
        StairShape::Straight => push_riser(stair_straight_rect(facing)),
        StairShape::OuterLeft => push_riser(stair_outer_rect(facing, true)),
        StairShape::OuterRight => push_riser(stair_outer_rect(facing, false)),
        StairShape::InnerLeft => {
            push_riser(stair_straight_rect(facing));
            push_riser(stair_inner_extra_rect(facing, true));
        }
        StairShape::InnerRight => {
            push_riser(stair_straight_rect(facing));
            push_riser(stair_inner_extra_rect(facing, false));
        }
    }
}

pub(crate) fn append_block_collision_boxes(
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
                append_box(block_x, block_y, block_z, [0.0, 0.5, 0.0], [1.0, 1.0, 1.0], out);
            } else {
                append_box(block_x, block_y, block_z, [0.0, 0.0, 0.0], [1.0, 0.5, 1.0], out);
            }
        }
        BlockModelKind::Stairs => append_stair_boxes(world, block_state, block_x, block_y, block_z, out),
        BlockModelKind::Fence => {
            let connect_east = fence_connects_to(world.block_at(block_x + 1, block_y, block_z));
            let connect_west = fence_connects_to(world.block_at(block_x - 1, block_y, block_z));
            let connect_south = fence_connects_to(world.block_at(block_x, block_y, block_z + 1));
            let connect_north = fence_connects_to(world.block_at(block_x, block_y, block_z - 1));
            append_box(block_x, block_y, block_z, [0.375, 0.0, 0.375], [0.625, 1.5, 0.625], out);
            if connect_north {
                append_box(block_x, block_y, block_z, [0.4375, 0.0, 0.0], [0.5625, 1.5, 0.5], out);
            }
            if connect_south {
                append_box(block_x, block_y, block_z, [0.4375, 0.0, 0.5], [0.5625, 1.5, 1.0], out);
            }
            if connect_west {
                append_box(block_x, block_y, block_z, [0.0, 0.0, 0.4375], [0.5, 1.5, 0.5625], out);
            }
            if connect_east {
                append_box(block_x, block_y, block_z, [0.5, 0.0, 0.4375], [1.0, 1.5, 0.5625], out);
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
                append_box(block_x, block_y, block_z, [0.4375, 0.0, 0.4375], [0.5625, 1.0, 0.5625], out);
            }
            if connect_north {
                append_box(block_x, block_y, block_z, [0.4375, 0.0, 0.0], [0.5625, 1.0, 0.5], out);
            }
            if connect_south {
                append_box(block_x, block_y, block_z, [0.4375, 0.0, 0.5], [0.5625, 1.0, 1.0], out);
            }
            if connect_west {
                append_box(block_x, block_y, block_z, [0.0, 0.0, 0.4375], [0.5, 1.0, 0.5625], out);
            }
            if connect_east {
                append_box(block_x, block_y, block_z, [0.5, 0.0, 0.4375], [1.0, 1.0, 0.5625], out);
            }
        }
        BlockModelKind::Custom => append_custom_block_collision_boxes(world, block_id, meta, block_x, block_y, block_z, out),
        _ => append_box(block_x, block_y, block_z, [0.0, 0.0, 0.0], [1.0, 1.0, 1.0], out),
    }
}

fn append_custom_block_collision_boxes(
    world: &WorldCollision,
    block_id: u16,
    meta: u8,
    block_x: i32,
    block_y: i32,
    block_z: i32,
    out: &mut Vec<Aabb>,
) {
    match block_id {
        54 | 130 | 146 => append_box(block_x, block_y, block_z, [1.0 / 16.0, 0.0, 1.0 / 16.0], [15.0 / 16.0, 14.0 / 16.0, 15.0 / 16.0], out),
        26 => append_box(block_x, block_y, block_z, [0.0, 0.0, 0.0], [1.0, 9.0 / 16.0, 1.0], out),
        27 | 28 | 66 | 157 => append_box(block_x, block_y, block_z, [0.0, 0.0, 0.0], [1.0, 1.0 / 16.0, 1.0], out),
        60 => append_box(block_x, block_y, block_z, [0.0, 0.0, 0.0], [1.0, 0.9375, 1.0], out),
        64 | 71 | 193 | 194 | 195 | 196 | 197 => {
            let lower_meta = if (meta & 0x8) != 0 {
                let below = world.block_at(block_x, block_y - 1, block_z);
                if block_state_id(below) == block_id {
                    block_state_meta(below)
                } else {
                    0
                }
            } else {
                meta
            };
            let facing = lower_meta & 0x3;
            let is_open = (lower_meta & 0x4) != 0;
            let t = 3.0 / 16.0;
            let (min, max) = if !is_open {
                match facing {
                    0 => ([0.0, 0.0, 0.0], [t, 1.0, 1.0]),
                    1 => ([0.0, 0.0, 0.0], [1.0, 1.0, t]),
                    2 => ([1.0 - t, 0.0, 0.0], [1.0, 1.0, 1.0]),
                    _ => ([0.0, 0.0, 1.0 - t], [1.0, 1.0, 1.0]),
                }
            } else {
                match facing {
                    0 => ([0.0, 0.0, 1.0 - t], [1.0, 1.0, 1.0]),
                    1 => ([0.0, 0.0, 0.0], [t, 1.0, 1.0]),
                    2 => ([0.0, 0.0, 0.0], [1.0, 1.0, t]),
                    _ => ([1.0 - t, 0.0, 0.0], [1.0, 1.0, 1.0]),
                }
            };
            append_box(block_x, block_y, block_z, min, max, out);
        }
        65 => {
            let t = 1.0 / 16.0;
            let (min, max) = match meta & 0x7 {
                2 => ([0.0, 0.0, 1.0 - t], [1.0, 1.0, 1.0]),
                3 => ([0.0, 0.0, 0.0], [1.0, 1.0, t]),
                4 => ([1.0 - t, 0.0, 0.0], [1.0, 1.0, 1.0]),
                5 => ([0.0, 0.0, 0.0], [t, 1.0, 1.0]),
                _ => ([0.0, 0.0, 0.0], [1.0, 1.0, t]),
            };
            append_box(block_x, block_y, block_z, min, max, out);
        }
        78 => {
            let layers = (meta & 0x7) + 1;
            let h = (layers as f32 / 8.0).clamp(0.125, 1.0);
            append_box(block_x, block_y, block_z, [0.0, 0.0, 0.0], [1.0, h, 1.0], out);
        }
        81 => append_box(block_x, block_y, block_z, [1.0 / 16.0, 0.0, 1.0 / 16.0], [15.0 / 16.0, 1.0, 15.0 / 16.0], out),
        88 => append_box(block_x, block_y, block_z, [0.0, 0.0, 0.0], [1.0, 0.875, 1.0], out),
        96 => {
            let is_open = (meta & 0x4) != 0;
            let is_top = (meta & 0x8) != 0;
            let t = 3.0 / 16.0;
            let (min, max) = if is_open {
                match meta & 0x3 {
                    0 => ([0.0, 0.0, 1.0 - t], [1.0, 1.0, 1.0]),
                    1 => ([0.0, 0.0, 0.0], [1.0, 1.0, t]),
                    2 => ([1.0 - t, 0.0, 0.0], [1.0, 1.0, 1.0]),
                    _ => ([0.0, 0.0, 0.0], [t, 1.0, 1.0]),
                }
            } else if is_top {
                ([0.0, 1.0 - t, 0.0], [1.0, 1.0, 1.0])
            } else {
                ([0.0, 0.0, 0.0], [1.0, t, 1.0])
            };
            append_box(block_x, block_y, block_z, min, max, out);
        }
        107 | 183 | 184 | 185 | 186 | 187 => {
            let facing = meta & 0x3;
            let is_open = (meta & 0x4) != 0;
            let x_aligned = matches!(facing, 0 | 2);
            let t = 0.125;
            let rail_min = 0.375;
            let rail_max = 0.625;
            if !is_open {
                let (panel_min, panel_max) = if x_aligned {
                    ([0.0, 0.0, rail_min], [1.0, 1.0, rail_max])
                } else {
                    ([rail_min, 0.0, 0.0], [rail_max, 1.0, 1.0])
                };
                append_box(block_x, block_y, block_z, panel_min, panel_max, out);
            }
            if x_aligned {
                append_box(block_x, block_y, block_z, [0.0, 0.0, 0.4375], [t, 1.0, 0.5625], out);
                append_box(block_x, block_y, block_z, [1.0 - t, 0.0, 0.4375], [1.0, 1.0, 0.5625], out);
            } else {
                append_box(block_x, block_y, block_z, [0.4375, 0.0, 0.0], [0.5625, 1.0, t], out);
                append_box(block_x, block_y, block_z, [0.4375, 0.0, 1.0 - t], [0.5625, 1.0, 1.0], out);
            }
        }
        139 => {
            let connect_east = wall_connects_to(world.block_at(block_x + 1, block_y, block_z));
            let connect_west = wall_connects_to(world.block_at(block_x - 1, block_y, block_z));
            let connect_south = wall_connects_to(world.block_at(block_x, block_y, block_z + 1));
            let connect_north = wall_connects_to(world.block_at(block_x, block_y, block_z - 1));
            let has_x = connect_east || connect_west;
            let has_z = connect_north || connect_south;
            let center_tall = !has_x || !has_z;
            append_box(block_x, block_y, block_z, [0.25, 0.0, 0.25], [0.75, if center_tall { 1.0 } else { 0.8125 }, 0.75], out);
            if connect_north {
                append_box(block_x, block_y, block_z, [0.3125, 0.0, 0.0], [0.6875, 0.8125, 0.5], out);
            }
            if connect_south {
                append_box(block_x, block_y, block_z, [0.3125, 0.0, 0.5], [0.6875, 0.8125, 1.0], out);
            }
            if connect_west {
                append_box(block_x, block_y, block_z, [0.0, 0.0, 0.3125], [0.5, 0.8125, 0.6875], out);
            }
            if connect_east {
                append_box(block_x, block_y, block_z, [0.5, 0.0, 0.3125], [1.0, 0.8125, 0.6875], out);
            }
        }
        171 => append_box(block_x, block_y, block_z, [0.0, 0.0, 0.0], [1.0, 1.0 / 16.0, 1.0], out),
        _ => {}
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

fn wall_connects_to(neighbor_state: u16) -> bool {
    let neighbor_id = block_state_id(neighbor_state);
    if neighbor_id == 0 || matches!(neighbor_id, 8 | 9 | 10 | 11) {
        return false;
    }
    if neighbor_id == 139 {
        return true;
    }
    if matches!(block_model_kind(neighbor_id), BlockModelKind::Fence) {
        return true;
    }
    if matches!(neighbor_id, 107 | 183 | 184 | 185 | 186 | 187) {
        return true;
    }
    is_solid(neighbor_state)
}

pub fn collision_parity_expected_box_count(
    world: &WorldCollision,
    block_state: u16,
    block_x: i32,
    block_y: i32,
    block_z: i32,
) -> Option<usize> {
    let block_id = block_state_id(block_state);
    let meta = block_state_meta(block_state);
    match block_model_kind(block_id) {
        BlockModelKind::Stairs => Some(match stair_shape(world, block_state, block_x, block_y, block_z) {
            StairShape::InnerLeft | StairShape::InnerRight => 3,
            StairShape::Straight | StairShape::OuterLeft | StairShape::OuterRight => 2,
        }),
        BlockModelKind::Fence => {
            let connect_east = fence_connects_to(world.block_at(block_x + 1, block_y, block_z));
            let connect_west = fence_connects_to(world.block_at(block_x - 1, block_y, block_z));
            let connect_south = fence_connects_to(world.block_at(block_x, block_y, block_z + 1));
            let connect_north = fence_connects_to(world.block_at(block_x, block_y, block_z - 1));
            Some(1 + usize::from(connect_east) + usize::from(connect_west) + usize::from(connect_south) + usize::from(connect_north))
        }
        BlockModelKind::Pane => {
            let connect_east = pane_connects_to(world.block_at(block_x + 1, block_y, block_z));
            let connect_west = pane_connects_to(world.block_at(block_x - 1, block_y, block_z));
            let connect_south = pane_connects_to(world.block_at(block_x, block_y, block_z + 1));
            let connect_north = pane_connects_to(world.block_at(block_x, block_y, block_z - 1));
            let has_x = connect_east || connect_west;
            let has_z = connect_north || connect_south;
            let center = usize::from(!has_x || !has_z);
            Some(center + usize::from(connect_east) + usize::from(connect_west) + usize::from(connect_south) + usize::from(connect_north))
        }
        BlockModelKind::Custom => {
            if matches!(block_id, 64 | 71 | 193 | 194 | 195 | 196 | 197) {
                return Some(1);
            }
            if matches!(block_id, 107 | 183 | 184 | 185 | 186 | 187) {
                return Some(if (meta & 0x4) != 0 { 2 } else { 3 });
            }
            if block_id == 139 {
                let connect_east = wall_connects_to(world.block_at(block_x + 1, block_y, block_z));
                let connect_west = wall_connects_to(world.block_at(block_x - 1, block_y, block_z));
                let connect_south = wall_connects_to(world.block_at(block_x, block_y, block_z + 1));
                let connect_north = wall_connects_to(world.block_at(block_x, block_y, block_z - 1));
                return Some(1 + usize::from(connect_east) + usize::from(connect_west) + usize::from(connect_south) + usize::from(connect_north));
            }
            None
        }
        _ => None,
    }
}

pub fn debug_block_collision_boxes(
    world: &WorldCollision,
    block_state: u16,
    block_x: i32,
    block_y: i32,
    block_z: i32,
) -> Vec<(Vec3, Vec3)> {
    let mut out = Vec::new();
    append_block_collision_boxes(world, block_state, block_x, block_y, block_z, &mut out);
    out.into_iter().map(|bb| (bb.min, bb.max)).collect()
}

pub(crate) fn is_water_state(block_state: u16) -> bool {
    matches!(block_state_id(block_state), 8 | 9)
}

pub(crate) fn step_toward_zero(v: f32) -> f32 {
    if v > 0.0 {
        (v - SNEAK_EDGE_STEP).max(0.0)
    } else {
        (v + SNEAK_EDGE_STEP).min(0.0)
    }
}

pub(crate) fn block_slipperiness(block_state: u16) -> f32 {
    match block_state_id(block_state) {
        79 | 174 => SLIPPERINESS_ICE,
        165 => SLIPPERINESS_SLIME,
        _ => SLIPPERINESS_DEFAULT,
    }
}
