use super::ModelDef;
use crate::{cube, part};

pub static CREEPER_MODEL_TEX64: ModelDef = ModelDef {
    tex_size: [64, 32],
    root_offset_px: [0.0, 24.0, 0.0],
    parts: &[
        part! {
            name: "head",
            parent: None,
            pivot: (0.0, 6.0, 0.0),
            cubes: [
                cube! { uv: (0, 0), from: (-4.0, -8.0, -4.0), size: (8.0, 8.0, 8.0), inflate: 0.0, mirror: false },
            ],
        },
        part! {
            name: "body",
            parent: None,
            pivot: (0.0, 6.0, 0.0),
            cubes: [
                cube! { uv: (16, 16), from: (-4.0, 0.0, -2.0), size: (8.0, 12.0, 4.0), inflate: 0.0, mirror: false },
            ],
        },
        part! {
            name: "leg_front_right",
            parent: None,
            pivot: (-2.0, 18.0, 4.0),
            cubes: [
                cube! { uv: (0, 16), from: (-2.0, 0.0, -2.0), size: (4.0, 6.0, 4.0), inflate: 0.0, mirror: false },
            ],
        },
        part! {
            name: "leg_front_left",
            parent: None,
            pivot: (2.0, 18.0, 4.0),
            cubes: [
                cube! { uv: (0, 16), from: (-2.0, 0.0, -2.0), size: (4.0, 6.0, 4.0), inflate: 0.0, mirror: true },
            ],
        },
        part! {
            name: "leg_back_right",
            parent: None,
            pivot: (-2.0, 18.0, -4.0),
            cubes: [
                cube! { uv: (0, 16), from: (-2.0, 0.0, -2.0), size: (4.0, 6.0, 4.0), inflate: 0.0, mirror: false },
            ],
        },
        part! {
            name: "leg_back_left",
            parent: None,
            pivot: (2.0, 18.0, -4.0),
            cubes: [
                cube! { uv: (0, 16), from: (-2.0, 0.0, -2.0), size: (4.0, 6.0, 4.0), inflate: 0.0, mirror: true },
            ],
        },
    ],
};
