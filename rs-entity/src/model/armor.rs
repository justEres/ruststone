use super::ModelDef;
use crate::{cube, part};

pub static BIPED_ARMOR_OUTER_MODEL: ModelDef = ModelDef {
    tex_size: [64, 32],
    root_offset_px: [0.0, 24.0, 0.0],
    parts: &[
        part! {
            name: "head",
            parent: None,
            pivot: (0.0, 0.0, 0.0),
            cubes: [
                cube! { uv: (0, 0), from: (-4.0, -8.0, -4.0), size: (8.0, 8.0, 8.0), inflate: 1.0, mirror: false },
            ],
        },
        part! {
            name: "headwear",
            parent: Some(0),
            pivot: (0.0, 0.0, 0.0),
            cubes: [
                cube! { uv: (32, 0), from: (-4.0, -8.0, -4.0), size: (8.0, 8.0, 8.0), inflate: 1.5, mirror: false },
            ],
        },
        part! {
            name: "body",
            parent: None,
            pivot: (0.0, 0.0, 0.0),
            cubes: [
                cube! { uv: (16, 16), from: (-4.0, 0.0, -2.0), size: (8.0, 12.0, 4.0), inflate: 1.0, mirror: false },
            ],
        },
        part! {
            name: "right_arm",
            parent: None,
            pivot: (-5.0, 2.0, 0.0),
            cubes: [
                cube! { uv: (40, 16), from: (-3.0, -2.0, -2.0), size: (4.0, 12.0, 4.0), inflate: 1.0, mirror: false },
            ],
        },
        part! {
            name: "left_arm",
            parent: None,
            pivot: (5.0, 2.0, 0.0),
            cubes: [
                cube! { uv: (40, 16), from: (-1.0, -2.0, -2.0), size: (4.0, 12.0, 4.0), inflate: 1.0, mirror: true },
            ],
        },
        part! {
            name: "right_leg",
            parent: None,
            pivot: (-1.9, 12.0, 0.0),
            cubes: [
                cube! { uv: (0, 16), from: (-2.0, 0.0, -2.0), size: (4.0, 12.0, 4.0), inflate: 1.0, mirror: false },
            ],
        },
        part! {
            name: "left_leg",
            parent: None,
            pivot: (1.9, 12.0, 0.0),
            cubes: [
                cube! { uv: (0, 16), from: (-2.0, 0.0, -2.0), size: (4.0, 12.0, 4.0), inflate: 1.0, mirror: true },
            ],
        },
    ],
};

pub static BIPED_ARMOR_INNER_MODEL: ModelDef = ModelDef {
    tex_size: [64, 32],
    root_offset_px: [0.0, 24.0, 0.0],
    parts: &[
        part! {
            name: "head",
            parent: None,
            pivot: (0.0, 0.0, 0.0),
            cubes: [
                cube! { uv: (0, 0), from: (-4.0, -8.0, -4.0), size: (8.0, 8.0, 8.0), inflate: 0.5, mirror: false },
            ],
        },
        part! {
            name: "headwear",
            parent: Some(0),
            pivot: (0.0, 0.0, 0.0),
            cubes: [
                cube! { uv: (32, 0), from: (-4.0, -8.0, -4.0), size: (8.0, 8.0, 8.0), inflate: 1.0, mirror: false },
            ],
        },
        part! {
            name: "body",
            parent: None,
            pivot: (0.0, 0.0, 0.0),
            cubes: [
                cube! { uv: (16, 16), from: (-4.0, 0.0, -2.0), size: (8.0, 12.0, 4.0), inflate: 0.5, mirror: false },
            ],
        },
        part! {
            name: "right_arm",
            parent: None,
            pivot: (-5.0, 2.0, 0.0),
            cubes: [
                cube! { uv: (40, 16), from: (-3.0, -2.0, -2.0), size: (4.0, 12.0, 4.0), inflate: 0.5, mirror: false },
            ],
        },
        part! {
            name: "left_arm",
            parent: None,
            pivot: (5.0, 2.0, 0.0),
            cubes: [
                cube! { uv: (40, 16), from: (-1.0, -2.0, -2.0), size: (4.0, 12.0, 4.0), inflate: 0.5, mirror: true },
            ],
        },
        part! {
            name: "right_leg",
            parent: None,
            pivot: (-1.9, 12.0, 0.0),
            cubes: [
                cube! { uv: (0, 16), from: (-2.0, 0.0, -2.0), size: (4.0, 12.0, 4.0), inflate: 0.5, mirror: false },
            ],
        },
        part! {
            name: "left_leg",
            parent: None,
            pivot: (1.9, 12.0, 0.0),
            cubes: [
                cube! { uv: (0, 16), from: (-2.0, 0.0, -2.0), size: (4.0, 12.0, 4.0), inflate: 0.5, mirror: true },
            ],
        },
    ],
};
