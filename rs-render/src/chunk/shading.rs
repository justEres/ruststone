use super::*;

fn vanilla_face_shade_exact(face: Face) -> f32 {
    match face {
        Face::PosY => 1.0,
        Face::NegY => 0.5,
        Face::PosZ | Face::NegZ => 0.8,
        Face::PosX | Face::NegX => 0.6,
    }
}

fn vanilla_face_shade(face: Face, vanilla_bake: VanillaBakeSettings) -> f32 {
    let target = match face {
        Face::PosY => 1.0,
        Face::NegY => 0.62,
        Face::PosZ | Face::NegZ => 0.86,
        Face::PosX | Face::NegX => 0.78,
    };
    let strength = vanilla_bake.face_shading_strength.clamp(0.0, 1.0);
    1.0 - strength + target * strength
}

fn vanilla_light_mix(sky_level: f32, block_level: f32, vanilla_bake: VanillaBakeSettings) -> f32 {
    let sky =
        (sky_level / 15.0).clamp(0.0, 1.0) * vanilla_bake.sky_light_strength.clamp(0.0, 2.0);
    let block =
        (block_level / 15.0).clamp(0.0, 1.0) * vanilla_bake.block_light_strength.clamp(0.0, 2.0);
    let ambient = vanilla_bake.ambient_floor.clamp(0.0, 0.95);
    let mixed = (ambient + sky + block).clamp(0.0, 1.0);
    let curve = vanilla_bake.light_curve.clamp(0.35, 2.5);
    mixed.powf(1.0 / curve)
}

pub(super) fn is_softened_vanilla_foliage(block_id: u16) -> bool {
    if is_leaves_block(block_type(block_id)) {
        return true;
    }
    matches!(
        classify_tint(block_id, None),
        TintClass::Grass | TintClass::Foliage | TintClass::FoliageFixed(_)
    )
}

fn is_vanilla_leaf_block(block_id: u16) -> bool {
    is_leaves_block(block_type(block_id))
}

fn vanilla_leaf_face_shade(face: Face) -> f32 {
    match face {
        Face::PosY => 1.0,
        Face::NegY => 0.84,
        Face::PosZ | Face::NegZ => 0.93,
        Face::PosX | Face::NegX => 0.89,
    }
}

fn vanilla_block_shadow_factor(
    snapshot: &ChunkColumnSnapshot,
    chunk_x: i32,
    chunk_z: i32,
    x: i32,
    y: i32,
    z: i32,
    face: Face,
    sky_level: f32,
    vanilla_bake: VanillaBakeSettings,
) -> f32 {
    if matches!(vanilla_bake.block_shadow_mode, VanillaBlockShadowMode::Off) {
        return 1.0;
    }
    let strength = vanilla_bake.block_shadow_strength.clamp(0.0, 1.0);
    if strength <= 0.001 {
        return 1.0;
    }

    let raw_occlusion = 1.0 - (sky_level / 15.0).clamp(0.0, 1.0);
    let skylight_occlusion = raw_occlusion * raw_occlusion * (3.0 - 2.0 * raw_occlusion);
    let mut shadow = skylight_occlusion;

    if matches!(
        vanilla_bake.block_shadow_mode,
        VanillaBlockShadowMode::SkylightPlusSunTrace
    ) && matches!(face, Face::PosY | Face::PosX | Face::NegX | Face::PosZ | Face::NegZ)
    {
        let trace = trace_sun_shadow(snapshot, chunk_x, chunk_z, x, y, z, face, vanilla_bake);
        shadow = shadow.max(trace);
        if matches!(face, Face::PosY) {
            shadow = (shadow - vanilla_bake.top_face_sun_bias.clamp(0.0, 0.5)).max(0.0);
        }
    }

    (1.0 - shadow * strength * 0.82).clamp(0.24, 1.0)
}

fn trace_sun_shadow(
    snapshot: &ChunkColumnSnapshot,
    chunk_x: i32,
    chunk_z: i32,
    x: i32,
    y: i32,
    z: i32,
    face: Face,
    vanilla_bake: VanillaBakeSettings,
) -> f32 {
    let samples = vanilla_bake.sun_trace_samples.clamp(1, 8) as i32;
    let max_distance = vanilla_bake.sun_trace_distance.clamp(1.0, 12.0);
    let origin = Vec3::new(x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5);
    let bias = match face {
        Face::PosY => Vec3::new(0.0, 0.55, 0.0),
        Face::NegY => Vec3::new(0.0, -0.1, 0.0),
        Face::PosX => Vec3::new(0.55, 0.2, 0.0),
        Face::NegX => Vec3::new(-0.55, 0.2, 0.0),
        Face::PosZ => Vec3::new(0.0, 0.2, 0.55),
        Face::NegZ => Vec3::new(0.0, 0.2, -0.55),
    };
    let start = origin + bias;
    let sun_dir = Vec3::new(-0.55, 1.0, -0.35).normalize();
    let mut occluded = 0.0;

    for step in 1..=samples {
        let t = max_distance * (step as f32 / samples as f32);
        let sample_pos = start + sun_dir * t;
        let block = block_at(
            snapshot,
            chunk_x,
            chunk_z,
            sample_pos.x.floor() as i32,
            sample_pos.y.floor() as i32,
            sample_pos.z.floor() as i32,
        );
        if is_ao_occluder(block) {
            occluded += 1.0;
        }
    }

    (occluded / samples as f32).clamp(0.0, 1.0)
}

fn light_factor_from_level_with_floor(level: f32, floor: f32) -> f32 {
    let floor = floor.clamp(0.0, 0.95);
    (floor + (level / 15.0) * (1.0 - floor)).clamp(0.0, 1.0)
}

pub(super) fn can_apply_vertex_shading(block_id: u16, voxel_ao_cutout: bool) -> bool {
    match render_group_for_block(block_id) {
        MaterialGroup::Opaque => true,
        MaterialGroup::Cutout | MaterialGroup::CutoutCulled => voxel_ao_cutout,
        MaterialGroup::Transparent => false,
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn compute_vertex_shade(
    snapshot: &ChunkColumnSnapshot,
    chunk_x: i32,
    chunk_z: i32,
    x: i32,
    y: i32,
    z: i32,
    face: Face,
    vertex: [f32; 3],
    block_id: u16,
    voxel_ao_enabled: bool,
    voxel_ao_strength: f32,
    voxel_ao_cutout: bool,
    vanilla_bake: VanillaBakeSettings,
) -> f32 {
    if !can_apply_vertex_shading(block_id, voxel_ao_cutout) {
        return 1.0;
    }
    let leaf_block = is_vanilla_leaf_block(block_id);
    if leaf_block {
        return vanilla_leaf_face_baked_shade(
            snapshot,
            chunk_x,
            chunk_z,
            x,
            y,
            z,
            face,
            voxel_ao_enabled,
            voxel_ao_strength,
            vanilla_bake,
        );
    }
    let softened_foliage = is_softened_vanilla_foliage(block_id);
    let (ao, sky_level, block_level) =
        face_vertex_light_ao(snapshot, chunk_x, chunk_z, x, y, z, face, vertex);
    let ao_term = if voxel_ao_enabled {
        let mut s = voxel_ao_strength.clamp(0.0, 1.0);
        if leaf_block {
            s *= 0.10;
        } else if softened_foliage {
            s *= 0.38;
        }
        1.0 - s + ao * s
    } else {
        1.0
    };
    let mut shadow_term = vanilla_block_shadow_factor(
        snapshot,
        chunk_x,
        chunk_z,
        x,
        y,
        z,
        face,
        sky_level,
        vanilla_bake,
    );
    shadow_term = if softened_foliage {
        if leaf_block {
            shadow_term * 0.18 + 0.82
        } else {
            shadow_term * 0.40 + 0.60
        }
    } else {
        shadow_term * 0.70 + 0.30
    };
    let ao_shadow_term = if voxel_ao_enabled {
        let mut blend = vanilla_bake.ao_shadow_blend.clamp(0.0, 1.0);
        if leaf_block {
            blend *= 0.15;
        } else if softened_foliage {
            blend *= 0.45;
        }
        ao_term * (1.0 - blend) + (ao_term * shadow_term) * blend
    } else {
        shadow_term
    };
    let mut face_term = vanilla_face_shade(face, vanilla_bake);
    if leaf_block {
        let leaf_target = vanilla_leaf_face_shade(face);
        face_term = face_term * 0.25 + leaf_target * 0.75;
    } else if softened_foliage {
        face_term = face_term * 0.28 + 0.72;
    }
    let mut light = vanilla_light_mix(sky_level, block_level, vanilla_bake);
    let base_floor = 0.18 + vanilla_bake.ambient_floor.clamp(0.0, 0.95) * 0.72;
    let foliage_floor = 0.34 + vanilla_bake.ambient_floor.clamp(0.0, 0.95) * 0.66;
    if leaf_block {
        let leaf_floor = 0.52 + vanilla_bake.ambient_floor.clamp(0.0, 0.95) * 0.34;
        light = (light * 1.12 + 0.04).max(leaf_floor);
    } else if softened_foliage {
        light = light.max(foliage_floor);
    } else {
        light = light.max(base_floor);
    }
    let shade = light * face_term * ao_shadow_term;
    let min_final = if leaf_block {
        0.52 + vanilla_bake.ambient_floor.clamp(0.0, 0.95) * 0.20
    } else if softened_foliage {
        foliage_floor
    } else {
        base_floor * 0.92
    };
    shade.max(min_final).clamp(0.0, 1.0)
}

pub(super) fn face_light_factor(
    snapshot: &ChunkColumnSnapshot,
    chunk_x: i32,
    chunk_z: i32,
    x: i32,
    y: i32,
    z: i32,
    face: Face,
) -> f32 {
    let (dx, dy, dz) = match face {
        Face::PosX => (1, 0, 0),
        Face::NegX => (-1, 0, 0),
        Face::PosY => (0, 1, 0),
        Face::NegY => (0, -1, 0),
        Face::PosZ => (0, 0, 1),
        Face::NegZ => (0, 0, -1),
    };
    let a = light_at(snapshot, chunk_x, chunk_z, x, y, z);
    let b = light_at(snapshot, chunk_x, chunk_z, x + dx, y + dy, z + dz);
    let level = (f32::from(a.block.max(a.sky)) + f32::from(b.block.max(b.sky))) * 0.5;
    light_factor_from_level_with_floor(level, 0.18)
}

pub(super) fn block_light_factor(
    snapshot: &ChunkColumnSnapshot,
    chunk_x: i32,
    chunk_z: i32,
    x: i32,
    y: i32,
    z: i32,
) -> f32 {
    let l0 = light_at(snapshot, chunk_x, chunk_z, x, y, z);
    let l1 = light_at(snapshot, chunk_x, chunk_z, x, y + 1, z);
    let level = (f32::from(l0.block.max(l0.sky)) + f32::from(l1.block.max(l1.sky))) * 0.5;
    light_factor_from_level_with_floor(level, 0.18)
}

fn face_light_levels(
    snapshot: &ChunkColumnSnapshot,
    chunk_x: i32,
    chunk_z: i32,
    x: i32,
    y: i32,
    z: i32,
    face: Face,
) -> (f32, f32) {
    let (dx, dy, dz) = match face {
        Face::PosX => (1, 0, 0),
        Face::NegX => (-1, 0, 0),
        Face::PosY => (0, 1, 0),
        Face::NegY => (0, -1, 0),
        Face::PosZ => (0, 0, 1),
        Face::NegZ => (0, 0, -1),
    };
    let a = light_at(snapshot, chunk_x, chunk_z, x, y, z);
    let b = light_at(snapshot, chunk_x, chunk_z, x + dx, y + dy, z + dz);
    (
        (f32::from(a.sky) + f32::from(b.sky)) * 0.5,
        (f32::from(a.block) + f32::from(b.block)) * 0.5,
    )
}

fn vanilla_leaf_face_baked_shade(
    snapshot: &ChunkColumnSnapshot,
    chunk_x: i32,
    chunk_z: i32,
    x: i32,
    y: i32,
    z: i32,
    face: Face,
    voxel_ao_enabled: bool,
    voxel_ao_strength: f32,
    vanilla_bake: VanillaBakeSettings,
) -> f32 {
    let (sky_level, block_level) = face_light_levels(snapshot, chunk_x, chunk_z, x, y, z, face);
    let face_term = vanilla_face_shade_exact(face);
    let ao_term = if voxel_ao_enabled {
        apply_ao_strength(
            averaged_face_vanilla_ao(snapshot, chunk_x, chunk_z, x, y, z, face),
            voxel_ao_strength,
        )
    } else {
        1.0
    };
    let light = vanilla_light_mix(sky_level, block_level, vanilla_bake);
    (light * face_term * ao_term).clamp(0.0, 1.0)
}
