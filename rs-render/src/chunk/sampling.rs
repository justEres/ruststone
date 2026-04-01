use super::*;

pub(super) fn biome_at(
    snapshot: &ChunkColumnSnapshot,
    chunk_x: i32,
    chunk_z: i32,
    x: i32,
    z: i32,
) -> u8 {
    let Some(column) = snapshot.columns.get(&(chunk_x, chunk_z)) else {
        return 1;
    };
    let Some(biomes) = column.biomes.as_ref() else {
        return 1;
    };
    let idx = (z as usize & 15) * 16 + (x as usize & 15);
    *biomes.get(idx).unwrap_or(&1)
}

pub(super) fn biome_tint_at(
    snapshot: &ChunkColumnSnapshot,
    chunk_x: i32,
    chunk_z: i32,
    x: i32,
    z: i32,
    resolver: &BiomeTintResolver,
) -> BiomeTint {
    let mut grass = [0.0f32; 3];
    let mut foliage = [0.0f32; 3];
    let mut water = [0.0f32; 3];
    let mut count = 0.0f32;

    for dz in -1..=1 {
        for dx in -1..=1 {
            let wx = x + dx;
            let wz = z + dz;
            let mut cx = chunk_x;
            let mut cz = chunk_z;
            let mut lx = wx;
            let mut lz = wz;
            if lx < 0 {
                cx -= 1;
                lx += 16;
            } else if lx >= 16 {
                cx += 1;
                lx -= 16;
            }
            if lz < 0 {
                cz -= 1;
                lz += 16;
            } else if lz >= 16 {
                cz += 1;
                lz -= 16;
            }
            let bt = resolver.tint_for_biome(biome_at(snapshot, cx, cz, lx, lz));
            grass[0] += bt.grass[0];
            grass[1] += bt.grass[1];
            grass[2] += bt.grass[2];
            foliage[0] += bt.foliage[0];
            foliage[1] += bt.foliage[1];
            foliage[2] += bt.foliage[2];
            water[0] += bt.water[0];
            water[1] += bt.water[1];
            water[2] += bt.water[2];
            count += 1.0;
        }
    }

    BiomeTint {
        grass: [grass[0] / count, grass[1] / count, grass[2] / count, 1.0],
        foliage: [
            foliage[0] / count,
            foliage[1] / count,
            foliage[2] / count,
            1.0,
        ],
        water: [water[0] / count, water[1] / count, water[2] / count, 1.0],
    }
}

pub(super) fn resolve_chunk_coords(
    chunk_x: i32,
    chunk_z: i32,
    x: i32,
    z: i32,
) -> (i32, i32, i32, i32) {
    let mut target_chunk_x = chunk_x;
    let mut target_chunk_z = chunk_z;
    let mut local_x = x;
    let mut local_z = z;

    if local_x < 0 {
        target_chunk_x -= 1;
        local_x += CHUNK_SIZE;
    } else if local_x >= CHUNK_SIZE {
        target_chunk_x += 1;
        local_x -= CHUNK_SIZE;
    }

    if local_z < 0 {
        target_chunk_z -= 1;
        local_z += CHUNK_SIZE;
    } else if local_z >= CHUNK_SIZE {
        target_chunk_z += 1;
        local_z -= CHUNK_SIZE;
    }

    (target_chunk_x, target_chunk_z, local_x, local_z)
}

pub(super) fn block_at(
    snapshot: &ChunkColumnSnapshot,
    chunk_x: i32,
    chunk_z: i32,
    x: i32,
    y: i32,
    z: i32,
) -> u16 {
    if !(0..WORLD_HEIGHT).contains(&y) {
        return 0;
    }

    let (target_chunk_x, target_chunk_z, local_x, local_z) =
        resolve_chunk_coords(chunk_x, chunk_z, x, z);

    let Some(column) = snapshot.columns.get(&(target_chunk_x, target_chunk_z)) else {
        return 0;
    };

    let section_index = (y / SECTION_HEIGHT) as usize;
    let local_y = (y % SECTION_HEIGHT) as usize;

    let Some(section) = column.sections.get(section_index).and_then(|v| v.as_ref()) else {
        return 0;
    };

    let idx = local_y * 16 * 16 + local_z as usize * 16 + local_x as usize;
    section[idx]
}

#[derive(Clone, Copy, Default)]
pub(super) struct VoxelLight {
    pub block: u8,
    pub sky: u8,
}

pub(super) fn light_at(
    snapshot: &ChunkColumnSnapshot,
    chunk_x: i32,
    chunk_z: i32,
    x: i32,
    y: i32,
    z: i32,
) -> VoxelLight {
    if !(0..WORLD_HEIGHT).contains(&y) {
        return VoxelLight::default();
    }

    let (target_chunk_x, target_chunk_z, local_x, local_z) =
        resolve_chunk_coords(chunk_x, chunk_z, x, z);
    let Some(column) = snapshot.columns.get(&(target_chunk_x, target_chunk_z)) else {
        return VoxelLight::default();
    };
    let section_index = (y / SECTION_HEIGHT) as usize;
    let local_y = (y % SECTION_HEIGHT) as usize;
    let idx = local_y * 16 * 16 + local_z as usize * 16 + local_x as usize;

    let block = column
        .block_light_sections
        .get(section_index)
        .and_then(|v| v.as_ref())
        .and_then(|s| s.get(idx))
        .copied()
        .unwrap_or(0);
    let sky = column
        .sky_light_sections
        .get(section_index)
        .and_then(|v| v.as_ref())
        .and_then(|s| s.get(idx))
        .copied()
        .unwrap_or(0);
    VoxelLight { block, sky }
}

pub(super) fn face_vertices(face: Face) -> [[f32; 3]; 4] {
    match face {
        Face::PosX => [
            [1.0, 0.0, 0.0],
            [1.0, 0.0, 1.0],
            [1.0, 1.0, 1.0],
            [1.0, 1.0, 0.0],
        ],
        Face::NegX => [
            [0.0, 0.0, 1.0],
            [0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 1.0, 1.0],
        ],
        Face::PosY => [
            [0.0, 1.0, 0.0],
            [1.0, 1.0, 0.0],
            [1.0, 1.0, 1.0],
            [0.0, 1.0, 1.0],
        ],
        Face::NegY => [
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 1.0],
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
        ],
        Face::PosZ => [
            [1.0, 0.0, 1.0],
            [0.0, 0.0, 1.0],
            [0.0, 1.0, 1.0],
            [1.0, 1.0, 1.0],
        ],
        Face::NegZ => [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ],
    }
}
