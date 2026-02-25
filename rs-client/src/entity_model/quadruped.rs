use super::ModelDef;
use crate::{cube, part};

pub const QUADRUPED_HEAD: usize = 0;
pub const QUADRUPED_BODY: usize = 1;
pub const QUADRUPED_LEG_FRONT_RIGHT: usize = 2;
pub const QUADRUPED_LEG_FRONT_LEFT: usize = 3;
pub const QUADRUPED_LEG_BACK_RIGHT: usize = 4;
pub const QUADRUPED_LEG_BACK_LEFT: usize = 5;

pub static PIG_MODEL_TEX32: ModelDef = ModelDef {
    tex_size: [64, 32],
    root_offset_px: [0.0, 24.0, 0.0],
    parts: &[
        part! {
            name: "head",
            parent: None,
            // ModelPig extends ModelQuadruped(legHeight=6), so pivots are offset by (24 - legHeight).
            pivot: (0.0, 12.0, -6.0),
            cubes: [
                cube! { uv: (0, 0), from: (-4.0, -4.0, -8.0), size: (8.0, 8.0, 8.0), inflate: 0.0, mirror: false },
                cube! { uv: (16, 16), from: (-2.0, 0.0, -9.0), size: (4.0, 3.0, 1.0), inflate: 0.0, mirror: false },
            ],
        },
        part! {
            name: "body",
            parent: None,
            pivot: (0.0, 11.0, 2.0),
            cubes: [
                cube! { uv: (28, 8), from: (-5.0, -10.0, -7.0), size: (10.0, 16.0, 8.0), inflate: 0.0, mirror: false },
            ],
        },
        part! {
            name: "leg_front_right",
            parent: None,
            pivot: (-3.0, 18.0, 7.0),
            cubes: [
                cube! { uv: (0, 16), from: (-2.0, 0.0, -2.0), size: (4.0, 6.0, 4.0), inflate: 0.0, mirror: false },
            ],
        },
        part! {
            name: "leg_front_left",
            parent: None,
            pivot: (3.0, 18.0, 7.0),
            cubes: [
                cube! { uv: (0, 16), from: (-2.0, 0.0, -2.0), size: (4.0, 6.0, 4.0), inflate: 0.0, mirror: true },
            ],
        },
        part! {
            name: "leg_back_right",
            parent: None,
            pivot: (-3.0, 18.0, -5.0),
            cubes: [
                cube! { uv: (0, 16), from: (-2.0, 0.0, -2.0), size: (4.0, 6.0, 4.0), inflate: 0.0, mirror: false },
            ],
        },
        part! {
            name: "leg_back_left",
            parent: None,
            pivot: (3.0, 18.0, -5.0),
            cubes: [
                cube! { uv: (0, 16), from: (-2.0, 0.0, -2.0), size: (4.0, 6.0, 4.0), inflate: 0.0, mirror: true },
            ],
        },
    ],
};

pub static SHEEP_MODEL_TEX32: ModelDef = ModelDef {
    tex_size: [64, 32],
    root_offset_px: [0.0, 24.0, 0.0],
    parts: &[
        part! {
            name: "head",
            parent: None,
            pivot: (0.0, 6.0, -8.0),
            cubes: [
                cube! { uv: (0, 0), from: (-3.0, -4.0, -6.0), size: (6.0, 6.0, 8.0), inflate: 0.0, mirror: false },
            ],
        },
        part! {
            name: "body",
            parent: None,
            pivot: (0.0, 5.0, 2.0),
            cubes: [
                cube! { uv: (28, 8), from: (-4.0, -10.0, -7.0), size: (8.0, 16.0, 6.0), inflate: 0.0, mirror: false },
            ],
        },
        part! {
            name: "leg_front_right",
            parent: None,
            pivot: (-3.0, 12.0, 7.0),
            cubes: [
                cube! { uv: (0, 16), from: (-2.0, 0.0, -2.0), size: (4.0, 12.0, 4.0), inflate: 0.0, mirror: false },
            ],
        },
        part! {
            name: "leg_front_left",
            parent: None,
            pivot: (3.0, 12.0, 7.0),
            cubes: [
                cube! { uv: (0, 16), from: (-2.0, 0.0, -2.0), size: (4.0, 12.0, 4.0), inflate: 0.0, mirror: true },
            ],
        },
        part! {
            name: "leg_back_right",
            parent: None,
            pivot: (-3.0, 12.0, -5.0),
            cubes: [
                cube! { uv: (0, 16), from: (-2.0, 0.0, -2.0), size: (4.0, 12.0, 4.0), inflate: 0.0, mirror: false },
            ],
        },
        part! {
            name: "leg_back_left",
            parent: None,
            pivot: (3.0, 12.0, -5.0),
            cubes: [
                cube! { uv: (0, 16), from: (-2.0, 0.0, -2.0), size: (4.0, 12.0, 4.0), inflate: 0.0, mirror: true },
            ],
        },
    ],
};

pub static COW_MODEL_TEX32: ModelDef = ModelDef {
    tex_size: [64, 32],
    root_offset_px: [0.0, 24.0, 0.0],
    parts: &[
        part! {
            name: "head",
            parent: None,
            pivot: (0.0, 4.0, -8.0),
            cubes: [
                cube! { uv: (0, 0), from: (-4.0, -4.0, -6.0), size: (8.0, 8.0, 6.0), inflate: 0.0, mirror: false },
                cube! { uv: (22, 0), from: (-5.0, -5.0, -4.0), size: (1.0, 3.0, 1.0), inflate: 0.0, mirror: false },
                cube! { uv: (22, 0), from: (4.0, -5.0, -4.0), size: (1.0, 3.0, 1.0), inflate: 0.0, mirror: false },
            ],
        },
        part! {
            name: "body",
            parent: None,
            pivot: (0.0, 5.0, 2.0),
            cubes: [
                cube! { uv: (18, 4), from: (-6.0, -10.0, -7.0), size: (12.0, 18.0, 10.0), inflate: 0.0, mirror: false },
                cube! { uv: (52, 0), from: (-2.0, 2.0, -8.0), size: (4.0, 6.0, 1.0), inflate: 0.0, mirror: false },
            ],
        },
        part! {
            name: "leg_front_right",
            parent: None,
            pivot: (-4.0, 12.0, 7.0),
            cubes: [
                cube! { uv: (0, 16), from: (-2.0, 0.0, -2.0), size: (4.0, 12.0, 4.0), inflate: 0.0, mirror: false },
            ],
        },
        part! {
            name: "leg_front_left",
            parent: None,
            pivot: (4.0, 12.0, 7.0),
            cubes: [
                cube! { uv: (0, 16), from: (-2.0, 0.0, -2.0), size: (4.0, 12.0, 4.0), inflate: 0.0, mirror: true },
            ],
        },
        part! {
            name: "leg_back_right",
            parent: None,
            pivot: (-4.0, 12.0, -6.0),
            cubes: [
                cube! { uv: (0, 16), from: (-2.0, 0.0, -2.0), size: (4.0, 12.0, 4.0), inflate: 0.0, mirror: false },
            ],
        },
        part! {
            name: "leg_back_left",
            parent: None,
            pivot: (4.0, 12.0, -6.0),
            cubes: [
                cube! { uv: (0, 16), from: (-2.0, 0.0, -2.0), size: (4.0, 12.0, 4.0), inflate: 0.0, mirror: true },
            ],
        },
    ],
};
