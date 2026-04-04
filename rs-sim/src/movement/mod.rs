mod aabb;
mod block_shapes;
mod simulate;
mod world;

pub use block_shapes::{collision_parity_expected_box_count, debug_block_collision_boxes};
pub use simulate::{effective_sprint, simulate_tick};
pub use world::WorldCollision;

pub(crate) const PLAYER_HALF_WIDTH: f32 = 0.3;
pub(crate) const PLAYER_HEIGHT: f32 = 1.8;
pub(crate) const PLAYER_STEP_HEIGHT: f32 = 0.6;
pub(crate) const COLLISION_EPS: f32 = 1e-5;

pub(crate) const GRAVITY: f32 = -0.08;
pub(crate) const AIR_DRAG: f32 = 0.98;
pub(crate) const WATER_GRAVITY: f32 = -0.02;
pub(crate) const WATER_DRAG: f32 = 0.8;
pub(crate) const WATER_SURFACE_STEP: f32 = 0.3;
pub(crate) const JUMP_VEL: f32 = 0.42;
pub(crate) const BASE_MOVE_SPEED: f32 = 0.1;
pub(crate) const SPEED_IN_AIR: f32 = 0.02;
pub(crate) const WATER_MOVE_SPEED: f32 = 0.02;
pub(crate) const SWIM_UP_ACCEL: f32 = 0.04;
pub(crate) const SLIPPERINESS_DEFAULT: f32 = 0.6;
pub(crate) const SLIPPERINESS_ICE: f32 = 0.98;
pub(crate) const SLIPPERINESS_SLIME: f32 = 0.8;
pub(crate) const SNEAK_EDGE_STEP: f32 = 0.05;
pub(crate) const SNEAK_INPUT_SCALE: f32 = 0.3;
pub(crate) const FLY_VERTICAL_ACCEL_MULT: f32 = 3.0;
pub(crate) const FLY_HORIZONTAL_DAMPING: f32 = 0.91;
pub(crate) const FLY_VERTICAL_DAMPING: f32 = 0.6;
pub(crate) const FLY_SPRINT_MULT: f32 = 2.0;
pub(crate) const SOUL_SAND_SLOWDOWN: f32 = 0.4;
pub(crate) const SPRINT_FORWARD_THRESHOLD: f32 = 0.8;
pub(crate) const MOVE_INPUT_DAMPING: f32 = 0.98;
