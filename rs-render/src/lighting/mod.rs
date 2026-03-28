mod materials;
mod postprocess;
mod presets;
mod uniforms;

pub use materials::{
    apply_lighting_quality, update_material_debug_stats, update_water_animation,
};
pub use postprocess::{apply_antialiasing, apply_depth_prepass_for_ssr, apply_ssao_quality};
pub use presets::{LightingQualityPreset, ShadowQualityPreset, uses_shadowed_pbr_path};
pub use uniforms::{effective_sun_direction, lighting_uniform_for_mode};
#[allow(unused_imports)]
pub use uniforms::vanilla_celestial_angle;
