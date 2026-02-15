use super::{ModelDef, PartDef};
use crate::{cube, part};

// Part indices for `BIPED_MODEL` and `BIPED_MODEL_WITH_HEADWEAR`.
pub const BIPED_HEAD: usize = 0;
pub const BIPED_HEADWEAR: usize = 1;
pub const BIPED_BODY: usize = 2;
pub const BIPED_RIGHT_ARM: usize = 3;
pub const BIPED_LEFT_ARM: usize = 4;
pub const BIPED_RIGHT_LEG: usize = 5;
pub const BIPED_LEFT_LEG: usize = 6;

pub static BIPED_MODEL_WITH_HEADWEAR: ModelDef = ModelDef {
    tex_size: [64, 32],
    // Vanilla biped origin is near the shoulders. Lift by 24px so feet sit at Y=0 in world.
    root_offset_px: [0.0, 24.0, 0.0],
    parts: &[
        // Head
        part! {
            name: "head",
            parent: None,
            pivot: (0.0, 0.0, 0.0),
            cubes: [
                cube! { uv: (0, 0), from: (-4.0, -8.0, -4.0), size: (8.0, 8.0, 8.0), inflate: 0.0, mirror: false },
            ],
        },
        // Headwear (hat / outer layer)
        part! {
            name: "headwear",
            parent: Some(BIPED_HEAD),
            pivot: (0.0, 0.0, 0.0),
            cubes: [
                cube! { uv: (32, 0), from: (-4.0, -8.0, -4.0), size: (8.0, 8.0, 8.0), inflate: 0.5, mirror: false },
            ],
        },
        // Body
        part! {
            name: "body",
            parent: None,
            pivot: (0.0, 0.0, 0.0),
            cubes: [
                cube! { uv: (16, 16), from: (-4.0, 0.0, -2.0), size: (8.0, 12.0, 4.0), inflate: 0.0, mirror: false },
            ],
        },
        // Right arm
        part! {
            name: "right_arm",
            parent: None,
            pivot: (-5.0, 2.0, 0.0),
            cubes: [
                cube! { uv: (40, 16), from: (-3.0, -2.0, -2.0), size: (4.0, 12.0, 4.0), inflate: 0.0, mirror: false },
            ],
        },
        // Left arm (mirrored)
        part! {
            name: "left_arm",
            parent: None,
            pivot: (5.0, 2.0, 0.0),
            cubes: [
                cube! { uv: (40, 16), from: (-1.0, -2.0, -2.0), size: (4.0, 12.0, 4.0), inflate: 0.0, mirror: true },
            ],
        },
        // Right leg
        part! {
            name: "right_leg",
            parent: None,
            pivot: (-1.9, 12.0, 0.0),
            cubes: [
                cube! { uv: (0, 16), from: (-2.0, 0.0, -2.0), size: (4.0, 12.0, 4.0), inflate: 0.0, mirror: false },
            ],
        },
        // Left leg (mirrored)
        part! {
            name: "left_leg",
            parent: None,
            pivot: (1.9, 12.0, 0.0),
            cubes: [
                cube! { uv: (0, 16), from: (-2.0, 0.0, -2.0), size: (4.0, 12.0, 4.0), inflate: 0.0, mirror: true },
            ],
        },
    ],
};

pub static BIPED_MODEL: ModelDef = ModelDef {
    tex_size: [64, 32],
    root_offset_px: [0.0, 24.0, 0.0],
    parts: &[
        // Head
        part! {
            name: "head",
            parent: None,
            pivot: (0.0, 0.0, 0.0),
            cubes: [
                cube! { uv: (0, 0), from: (-4.0, -8.0, -4.0), size: (8.0, 8.0, 8.0), inflate: 0.0, mirror: false },
            ],
        },
        // Headwear placeholder (no cubes) to keep indices stable.
        PartDef {
            name: "headwear",
            parent: Some(BIPED_HEAD),
            pivot: [0.0, 0.0, 0.0],
            cubes: &[],
        },
        // Body
        part! {
            name: "body",
            parent: None,
            pivot: (0.0, 0.0, 0.0),
            cubes: [
                cube! { uv: (16, 16), from: (-4.0, 0.0, -2.0), size: (8.0, 12.0, 4.0), inflate: 0.0, mirror: false },
            ],
        },
        // Right arm
        part! {
            name: "right_arm",
            parent: None,
            pivot: (-5.0, 2.0, 0.0),
            cubes: [
                cube! { uv: (40, 16), from: (-3.0, -2.0, -2.0), size: (4.0, 12.0, 4.0), inflate: 0.0, mirror: false },
            ],
        },
        // Left arm (mirrored)
        part! {
            name: "left_arm",
            parent: None,
            pivot: (5.0, 2.0, 0.0),
            cubes: [
                cube! { uv: (40, 16), from: (-1.0, -2.0, -2.0), size: (4.0, 12.0, 4.0), inflate: 0.0, mirror: true },
            ],
        },
        // Right leg
        part! {
            name: "right_leg",
            parent: None,
            pivot: (-1.9, 12.0, 0.0),
            cubes: [
                cube! { uv: (0, 16), from: (-2.0, 0.0, -2.0), size: (4.0, 12.0, 4.0), inflate: 0.0, mirror: false },
            ],
        },
        // Left leg (mirrored)
        part! {
            name: "left_leg",
            parent: None,
            pivot: (1.9, 12.0, 0.0),
            cubes: [
                cube! { uv: (0, 16), from: (-2.0, 0.0, -2.0), size: (4.0, 12.0, 4.0), inflate: 0.0, mirror: true },
            ],
        },
    ],
};
