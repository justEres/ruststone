#[derive(Debug, Clone, Copy)]
pub struct CubeDef {
    /// Texture offset in pixels (u, v), using vanilla ModelBox layout rules.
    pub uv: [u32; 2],
    /// Lower corner (x, y, z) in "model pixels" (vanilla coordinates; +Y is down).
    pub from: [f32; 3],
    /// Dimensions (w, h, d) in model pixels.
    pub size: [f32; 3],
    /// Inflate amount in model pixels (vanilla `addBox(..., modelSize)`).
    pub inflate: f32,
    /// Vanilla mirror flag (affects both geometry and UV winding).
    pub mirror: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct PartDef {
    pub name: &'static str,
    /// Index of the parent part, if any.
    pub parent: Option<usize>,
    /// Rotation point / pivot in model pixels (vanilla coordinates; +Y is down).
    pub pivot: [f32; 3],
    pub cubes: &'static [CubeDef],
}

#[derive(Debug, Clone, Copy)]
pub struct ModelDef {
    /// Texture dimensions in pixels (typically 64x32 or 64x64 in 1.8.9).
    pub tex_size: [u32; 2],
    /// Offset applied at the model root to place the model's feet at origin.
    ///
    /// This is in *bevy* coordinates, expressed in model pixels and applied before scaling by 1/16.
    /// For vanilla bipeds, `root_offset_px.y = 24.0` matches Minecraft's default model origin.
    pub root_offset_px: [f32; 3],
    pub parts: &'static [PartDef],
}
