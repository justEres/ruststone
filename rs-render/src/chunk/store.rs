use super::*;

pub fn update_store(store: &mut ChunkStore, chunk: ChunkData) {
    let key = (chunk.x, chunk.z);
    let column = store.chunks.entry(key).or_insert_with(ChunkColumn::new);

    if chunk.full {
        column.set_full();
    }

    if let Some(biomes) = chunk.biomes {
        column.biomes = Some(biomes);
    }

    for section in chunk.sections {
        column.set_section(
            section.y,
            section.blocks,
            section.block_light,
            section.sky_light,
        );
    }
}

pub fn apply_block_update(store: &mut ChunkStore, update: BlockUpdate) -> Vec<(i32, i32)> {
    if !(0..WORLD_HEIGHT).contains(&update.y) {
        return Vec::new();
    }

    let chunk_x = update.x.div_euclid(CHUNK_SIZE);
    let chunk_z = update.z.div_euclid(CHUNK_SIZE);
    let local_x = update.x.rem_euclid(CHUNK_SIZE) as usize;
    let local_z = update.z.rem_euclid(CHUNK_SIZE) as usize;

    let column = store
        .chunks
        .entry((chunk_x, chunk_z))
        .or_insert_with(ChunkColumn::new);
    column.set_block(local_x, update.y, local_z, update.block_id);

    let mut touched = vec![(chunk_x, chunk_z)];
    if local_x == 0 {
        touched.push((chunk_x - 1, chunk_z));
    }
    if local_x == (CHUNK_SIZE as usize - 1) {
        touched.push((chunk_x + 1, chunk_z));
    }
    if local_z == 0 {
        touched.push((chunk_x, chunk_z - 1));
    }
    if local_z == (CHUNK_SIZE as usize - 1) {
        touched.push((chunk_x, chunk_z + 1));
    }
    touched
}

pub fn snapshot_for_chunk(store: &ChunkStore, key: (i32, i32)) -> ChunkColumnSnapshot {
    let mut columns = HashMap::new();
    for dz in -1..=1 {
        for dx in -1..=1 {
            let neighbor_key = (key.0 + dx, key.1 + dz);
            if let Some(column) = store.chunks.get(&neighbor_key) {
                columns.insert(neighbor_key, column.clone());
            }
        }
    }
    ChunkColumnSnapshot {
        center_key: key,
        columns,
    }
}

pub(super) fn build_chunk_occlusion_data(
    snapshot: &ChunkColumnSnapshot,
    chunk_x: i32,
    chunk_z: i32,
) -> ChunkOcclusionData {
    let Some(column) = snapshot.columns.get(&(chunk_x, chunk_z)) else {
        return ChunkOcclusionData::fully_open();
    };

    let mut out = ChunkOcclusionData::default();
    let mut visited = vec![false; (CHUNK_SIZE * WORLD_HEIGHT * CHUNK_SIZE) as usize];
    let mut queue = VecDeque::new();

    let idx_of = |x: i32, y: i32, z: i32| -> usize {
        ((y * CHUNK_SIZE * CHUNK_SIZE) + (z * CHUNK_SIZE) + x) as usize
    };
    let local_block = |x: i32, y: i32, z: i32| -> u16 {
        let section_idx = (y / SECTION_HEIGHT) as usize;
        let local_y = (y % SECTION_HEIGHT) as usize;
        column
            .sections
            .get(section_idx)
            .and_then(|section| section.as_ref())
            .map(|section| {
                let block_idx = local_y * 16 * 16 + z as usize * 16 + x as usize;
                section[block_idx]
            })
            .unwrap_or(0)
    };
    let is_local_passable = |x: i32, y: i32, z: i32| -> bool {
        let block = local_block(x, y, z);
        !is_occluding_block(block)
    };
    let face_at = |x: i32, y: i32, z: i32| -> u8 {
        let mut mask = 0u8;
        if x == 0 {
            mask |= ChunkFace::NegX.bit();
        }
        if x == CHUNK_SIZE - 1 {
            mask |= ChunkFace::PosX.bit();
        }
        if y == 0 {
            mask |= ChunkFace::NegY.bit();
        }
        if y == WORLD_HEIGHT - 1 {
            mask |= ChunkFace::PosY.bit();
        }
        if z == 0 {
            mask |= ChunkFace::NegZ.bit();
        }
        if z == CHUNK_SIZE - 1 {
            mask |= ChunkFace::PosZ.bit();
        }
        mask
    };
    let mut connect_component_faces = |component_faces: u8| {
        if component_faces == 0 {
            return;
        }
        out.face_open_mask |= component_faces;
        for face in ChunkFace::ALL {
            if (component_faces & face.bit()) == 0 {
                continue;
            }
            out.face_connections[face.index()] |= component_faces;
        }
    };

    for y in 0..WORLD_HEIGHT {
        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let boundary_mask = face_at(x, y, z);
                if boundary_mask == 0 || !is_local_passable(x, y, z) {
                    continue;
                }
                let seed_idx = idx_of(x, y, z);
                if visited[seed_idx] {
                    continue;
                }

                visited[seed_idx] = true;
                queue.push_back((x, y, z));
                let mut component_faces = 0u8;

                while let Some((cx, cy, cz)) = queue.pop_front() {
                    component_faces |= face_at(cx, cy, cz);
                    for (dx, dy, dz) in [
                        (-1, 0, 0),
                        (1, 0, 0),
                        (0, -1, 0),
                        (0, 1, 0),
                        (0, 0, -1),
                        (0, 0, 1),
                    ] {
                        let nx = cx + dx;
                        let ny = cy + dy;
                        let nz = cz + dz;
                        if !(0..CHUNK_SIZE).contains(&nx)
                            || !(0..WORLD_HEIGHT).contains(&ny)
                            || !(0..CHUNK_SIZE).contains(&nz)
                        {
                            continue;
                        }
                        let nidx = idx_of(nx, ny, nz);
                        if visited[nidx] || !is_local_passable(nx, ny, nz) {
                            continue;
                        }
                        visited[nidx] = true;
                        queue.push_back((nx, ny, nz));
                    }
                }

                connect_component_faces(component_faces);
            }
        }
    }

    out
}
