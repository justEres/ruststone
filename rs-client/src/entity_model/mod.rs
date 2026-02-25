//! Vanilla-style (1.8.9) entity model definitions.
//!
//! Key constraints for this project:
//! - No runtime JSON model loading.
//! - Models are hardcoded as Rust static data (parts + cuboids).
//! - Only textures are loaded at runtime (from the texture pack / skin URLs).

mod biped;
mod creeper;
mod mesh;
mod quadruped;
mod textures;
mod types;

pub use biped::*;
pub use creeper::*;
pub use mesh::*;
pub use quadruped::*;
pub use textures::*;
pub use types::*;

// Small DSL macros to make defining cuboid models less painful.
// Intentionally kept as `macro_rules!` (no proc-macro / extra deps).

#[macro_export]
macro_rules! cube {
    (
        uv: ($u:expr, $v:expr),
        from: ($x:expr, $y:expr, $z:expr),
        size: ($w:expr, $h:expr, $d:expr),
        inflate: $inflate:expr,
        mirror: $mirror:expr $(,)?
    ) => {
        $crate::entity_model::CubeDef {
            uv: [$u as u32, $v as u32],
            from: [$x as f32, $y as f32, $z as f32],
            size: [$w as f32, $h as f32, $d as f32],
            inflate: $inflate as f32,
            mirror: $mirror,
        }
    };
}

#[macro_export]
macro_rules! part {
    (
        name: $name:expr,
        parent: $parent:expr,
        pivot: ($x:expr, $y:expr, $z:expr),
        cubes: [ $($cube:expr),* $(,)? ] $(,)?
    ) => {
        $crate::entity_model::PartDef {
            name: $name,
            parent: $parent,
            pivot: [$x as f32, $y as f32, $z as f32],
            cubes: &[$($cube),*],
        }
    };
}
