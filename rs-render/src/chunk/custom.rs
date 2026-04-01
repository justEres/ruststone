use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn add_custom_block(
    batch: &mut MeshBatch,
    snapshot: &ChunkColumnSnapshot,
    texture_mapping: &AtlasBlockMapping,
    biome_tints: &BiomeTintResolver,
    chunk_x: i32,
    chunk_z: i32,
    x: i32,
    y: i32,
    z: i32,
    block_id: u16,
    tint: BiomeTint,
    barrier_billboard: bool,
    voxel_ao_enabled: bool,
    voxel_ao_strength: f32,
    voxel_ao_cutout: bool,
) {
    match block_model_kind(block_type(block_id)) {
        BlockModelKind::Cross | BlockModelKind::TorchLike => add_cross_plant(
            batch,
            snapshot,
            texture_mapping,
            biome_tints,
            chunk_x,
            chunk_z,
            x,
            y,
            z,
            block_id,
            tint,
            voxel_ao_enabled,
            voxel_ao_strength,
            voxel_ao_cutout,
        ),
        BlockModelKind::Slab => add_slab_block(
            batch,
            snapshot,
            texture_mapping,
            biome_tints,
            chunk_x,
            chunk_z,
            x,
            y,
            z,
            block_id,
            tint,
        ),
        BlockModelKind::Stairs => add_stairs_block(
            batch,
            snapshot,
            texture_mapping,
            biome_tints,
            chunk_x,
            chunk_z,
            x,
            y,
            z,
            block_id,
            tint,
        ),
        BlockModelKind::Fence => add_fence_block(
            batch,
            snapshot,
            texture_mapping,
            biome_tints,
            chunk_x,
            chunk_z,
            x,
            y,
            z,
            block_id,
            tint,
        ),
        BlockModelKind::Pane => add_pane_block(
            batch,
            snapshot,
            texture_mapping,
            biome_tints,
            chunk_x,
            chunk_z,
            x,
            y,
            z,
            block_id,
            tint,
        ),
        BlockModelKind::Custom => add_named_custom_block(
            batch,
            snapshot,
            texture_mapping,
            biome_tints,
            chunk_x,
            chunk_z,
            x,
            y,
            z,
            block_id,
            tint,
            barrier_billboard,
            voxel_ao_enabled,
            voxel_ao_strength,
            voxel_ao_cutout,
        ),
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn add_slab_block(
    batch: &mut MeshBatch,
    snapshot: &ChunkColumnSnapshot,
    texture_mapping: &AtlasBlockMapping,
    biome_tints: &BiomeTintResolver,
    chunk_x: i32,
    chunk_z: i32,
    x: i32,
    y: i32,
    z: i32,
    block_id: u16,
    tint: BiomeTint,
) {
    add_box(
        batch,
        Some((snapshot, chunk_x, chunk_z, x, y, z, block_id)),
        texture_mapping,
        biome_tints,
        x,
        y,
        z,
        if (block_meta(block_id) & 0x8) != 0 {
            [0.0, 0.5, 0.0]
        } else {
            [0.0, 0.0, 0.0]
        },
        if (block_meta(block_id) & 0x8) != 0 {
            [1.0, 1.0, 1.0]
        } else {
            [1.0, 0.5, 1.0]
        },
        block_id,
        tint,
    );
}

#[allow(clippy::too_many_arguments)]
fn add_stairs_block(
    batch: &mut MeshBatch,
    snapshot: &ChunkColumnSnapshot,
    texture_mapping: &AtlasBlockMapping,
    biome_tints: &BiomeTintResolver,
    chunk_x: i32,
    chunk_z: i32,
    x: i32,
    y: i32,
    z: i32,
    block_id: u16,
    tint: BiomeTint,
) {
    let meta = block_meta(block_id);
    let top = (meta & 0x4) != 0;
    let facing = meta & 0x3;

    add_box(
        batch,
        Some((snapshot, chunk_x, chunk_z, x, y, z, block_id)),
        texture_mapping,
        biome_tints,
        x,
        y,
        z,
        if top {
            [0.0, 0.5, 0.0]
        } else {
            [0.0, 0.0, 0.0]
        },
        if top {
            [1.0, 1.0, 1.0]
        } else {
            [1.0, 0.5, 1.0]
        },
        block_id,
        tint,
    );

    let (min_x, max_x, min_z, max_z) = match facing {
        0 => (0.5, 1.0, 0.0, 1.0),
        1 => (0.0, 0.5, 0.0, 1.0),
        2 => (0.0, 1.0, 0.5, 1.0),
        _ => (0.0, 1.0, 0.0, 0.5),
    };
    add_box(
        batch,
        Some((snapshot, chunk_x, chunk_z, x, y, z, block_id)),
        texture_mapping,
        biome_tints,
        x,
        y,
        z,
        if top {
            [min_x, 0.0, min_z]
        } else {
            [min_x, 0.5, min_z]
        },
        if top {
            [max_x, 0.5, max_z]
        } else {
            [max_x, 1.0, max_z]
        },
        block_id,
        tint,
    );
}

#[allow(clippy::too_many_arguments)]
fn add_fence_block(
    batch: &mut MeshBatch,
    snapshot: &ChunkColumnSnapshot,
    texture_mapping: &AtlasBlockMapping,
    biome_tints: &BiomeTintResolver,
    chunk_x: i32,
    chunk_z: i32,
    x: i32,
    y: i32,
    z: i32,
    block_id: u16,
    tint: BiomeTint,
) {
    let connect_east = fence_connects_to(block_at(snapshot, chunk_x, chunk_z, x + 1, y, z));
    let connect_west = fence_connects_to(block_at(snapshot, chunk_x, chunk_z, x - 1, y, z));
    let connect_south = fence_connects_to(block_at(snapshot, chunk_x, chunk_z, x, y, z + 1));
    let connect_north = fence_connects_to(block_at(snapshot, chunk_x, chunk_z, x, y, z - 1));
    add_box(
        batch,
        None,
        texture_mapping,
        biome_tints,
        x,
        y,
        z,
        [0.375, 0.0, 0.375],
        [0.625, 1.0, 0.625],
        block_id,
        tint,
    );
    if connect_north {
        add_box(
            batch,
            None,
            texture_mapping,
            biome_tints,
            x,
            y,
            z,
            [0.4375, 0.375, 0.0],
            [0.5625, 0.8125, 0.5],
            block_id,
            tint,
        );
    }
    if connect_south {
        add_box(
            batch,
            None,
            texture_mapping,
            biome_tints,
            x,
            y,
            z,
            [0.4375, 0.375, 0.5],
            [0.5625, 0.8125, 1.0],
            block_id,
            tint,
        );
    }
    if connect_west {
        add_box(
            batch,
            None,
            texture_mapping,
            biome_tints,
            x,
            y,
            z,
            [0.0, 0.375, 0.4375],
            [0.5, 0.8125, 0.5625],
            block_id,
            tint,
        );
    }
    if connect_east {
        add_box(
            batch,
            None,
            texture_mapping,
            biome_tints,
            x,
            y,
            z,
            [0.5, 0.375, 0.4375],
            [1.0, 0.8125, 0.5625],
            block_id,
            tint,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn add_pane_block(
    batch: &mut MeshBatch,
    snapshot: &ChunkColumnSnapshot,
    texture_mapping: &AtlasBlockMapping,
    biome_tints: &BiomeTintResolver,
    chunk_x: i32,
    chunk_z: i32,
    x: i32,
    y: i32,
    z: i32,
    block_id: u16,
    tint: BiomeTint,
) {
    let connect_east = pane_connects_to(block_at(snapshot, chunk_x, chunk_z, x + 1, y, z));
    let connect_west = pane_connects_to(block_at(snapshot, chunk_x, chunk_z, x - 1, y, z));
    let connect_south = pane_connects_to(block_at(snapshot, chunk_x, chunk_z, x, y, z + 1));
    let connect_north = pane_connects_to(block_at(snapshot, chunk_x, chunk_z, x, y, z - 1));
    let has_x = connect_east || connect_west;
    let has_z = connect_north || connect_south;
    let add_center = !has_x || !has_z;

    if add_center {
        add_box(
            batch,
            None,
            texture_mapping,
            biome_tints,
            x,
            y,
            z,
            [0.4375, 0.0, 0.4375],
            [0.5625, 1.0, 0.5625],
            block_id,
            tint,
        );
    }

    if has_z {
        if connect_north {
            add_box(
                batch,
                None,
                texture_mapping,
                biome_tints,
                x,
                y,
                z,
                [0.4375, 0.0, 0.0],
                [0.5625, 1.0, 0.5],
                block_id,
                tint,
            );
        }
        if connect_south {
            add_box(
                batch,
                None,
                texture_mapping,
                biome_tints,
                x,
                y,
                z,
                [0.4375, 0.0, 0.5],
                [0.5625, 1.0, 1.0],
                block_id,
                tint,
            );
        }
    }

    if has_x {
        if connect_west {
            add_box(
                batch,
                None,
                texture_mapping,
                biome_tints,
                x,
                y,
                z,
                [0.0, 0.0, 0.4375],
                [0.5, 1.0, 0.5625],
                block_id,
                tint,
            );
        }
        if connect_east {
            add_box(
                batch,
                None,
                texture_mapping,
                biome_tints,
                x,
                y,
                z,
                [0.5, 0.0, 0.4375],
                [1.0, 1.0, 0.5625],
                block_id,
                tint,
            );
        }
    }
}
