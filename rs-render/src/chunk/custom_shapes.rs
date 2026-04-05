use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn add_named_custom_block(
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
    voxel_ao_foliage_boost: f32,
) {
    let id = block_type(block_id);
    match id {
        54 | 130 | 146 => add_box(
            batch,
            Some((snapshot, chunk_x, chunk_z, x, y, z, block_id)),
            texture_mapping,
            biome_tints,
            x,
            y,
            z,
            [1.0 / 16.0, 0.0, 1.0 / 16.0],
            [15.0 / 16.0, 14.0 / 16.0, 15.0 / 16.0],
            block_id,
            tint,
        ),
        26 => add_box(
            batch,
            Some((snapshot, chunk_x, chunk_z, x, y, z, block_id)),
            texture_mapping,
            biome_tints,
            x,
            y,
            z,
            [0.0, 0.0, 0.0],
            [1.0, 9.0 / 16.0, 1.0],
            block_id,
            tint,
        ),
        60 => add_box(
            batch,
            Some((snapshot, chunk_x, chunk_z, x, y, z, block_id)),
            texture_mapping,
            biome_tints,
            x,
            y,
            z,
            [0.0, 0.0, 0.0],
            [1.0, 0.9375, 1.0],
            block_id,
            tint,
        ),
        27 | 28 | 66 | 157 => add_box(
            batch,
            Some((snapshot, chunk_x, chunk_z, x, y, z, block_id)),
            texture_mapping,
            biome_tints,
            x,
            y,
            z,
            [0.0, 0.0, 0.0],
            [1.0, 1.0 / 16.0, 1.0],
            block_id,
            tint,
        ),
        64 | 71 | 193 | 194 | 195 | 196 | 197 => {
            let meta = block_meta(block_id);
            let lower_meta = if (meta & 0x8) != 0 {
                let below = block_at(snapshot, chunk_x, chunk_z, x, y - 1, z);
                if block_type(below) == id {
                    block_meta(below)
                } else {
                    0
                }
            } else {
                meta
            };
            let facing = lower_meta & 0x3;
            let is_open = (lower_meta & 0x4) != 0;
            let t = 3.0 / 16.0;
            let (min, max) = if !is_open {
                match facing {
                    0 => ([0.0, 0.0, 0.0], [t, 1.0, 1.0]),
                    1 => ([0.0, 0.0, 0.0], [1.0, 1.0, t]),
                    2 => ([1.0 - t, 0.0, 0.0], [1.0, 1.0, 1.0]),
                    _ => ([0.0, 0.0, 1.0 - t], [1.0, 1.0, 1.0]),
                }
            } else {
                match facing {
                    0 => ([0.0, 0.0, 1.0 - t], [1.0, 1.0, 1.0]),
                    1 => ([0.0, 0.0, 0.0], [t, 1.0, 1.0]),
                    2 => ([0.0, 0.0, 0.0], [1.0, 1.0, t]),
                    _ => ([1.0 - t, 0.0, 0.0], [1.0, 1.0, 1.0]),
                }
            };
            add_box(
                batch,
                Some((snapshot, chunk_x, chunk_z, x, y, z, block_id)),
                texture_mapping,
                biome_tints,
                x,
                y,
                z,
                min,
                max,
                block_id,
                tint,
            );
        }
        65 => {
            let t = 1.0 / 16.0;
            let (min, max) = match block_meta(block_id) & 0x7 {
                2 => ([0.0, 0.0, 1.0 - t], [1.0, 1.0, 1.0]),
                3 => ([0.0, 0.0, 0.0], [1.0, 1.0, t]),
                4 => ([1.0 - t, 0.0, 0.0], [1.0, 1.0, 1.0]),
                5 => ([0.0, 0.0, 0.0], [t, 1.0, 1.0]),
                _ => ([0.0, 0.0, 0.0], [1.0, 1.0, t]),
            };
            add_box(
                batch,
                Some((snapshot, chunk_x, chunk_z, x, y, z, block_id)),
                texture_mapping,
                biome_tints,
                x,
                y,
                z,
                min,
                max,
                block_id,
                tint,
            );
        }
        107 | 183 | 184 | 185 | 186 | 187 => {
            let meta = block_meta(block_id);
            let facing = meta & 0x3;
            let is_open = (meta & 0x4) != 0;
            let x_aligned = matches!(facing, 0 | 2);
            let t = 0.125;
            let rail_min = 0.375;
            let rail_max = 0.625;

            let (panel_min, panel_max) = if !is_open {
                if x_aligned {
                    ([0.0, 0.0, rail_min], [1.0, 1.0, rail_max])
                } else {
                    ([rail_min, 0.0, 0.0], [rail_max, 1.0, 1.0])
                }
            } else if x_aligned {
                ([rail_min, 0.0, 0.0], [rail_max, 1.0, 1.0])
            } else {
                ([0.0, 0.0, rail_min], [1.0, 1.0, rail_max])
            };
            add_box(
                batch,
                Some((snapshot, chunk_x, chunk_z, x, y, z, block_id)),
                texture_mapping,
                biome_tints,
                x,
                y,
                z,
                panel_min,
                panel_max,
                block_id,
                tint,
            );

            if x_aligned {
                add_box(
                    batch,
                    None,
                    texture_mapping,
                    biome_tints,
                    x,
                    y,
                    z,
                    [0.0, 0.0, 0.4375],
                    [t, 1.0, 0.5625],
                    block_id,
                    tint,
                );
                add_box(
                    batch,
                    None,
                    texture_mapping,
                    biome_tints,
                    x,
                    y,
                    z,
                    [1.0 - t, 0.0, 0.4375],
                    [1.0, 1.0, 0.5625],
                    block_id,
                    tint,
                );
            } else {
                add_box(
                    batch,
                    None,
                    texture_mapping,
                    biome_tints,
                    x,
                    y,
                    z,
                    [0.4375, 0.0, 0.0],
                    [0.5625, 1.0, t],
                    block_id,
                    tint,
                );
                add_box(
                    batch,
                    None,
                    texture_mapping,
                    biome_tints,
                    x,
                    y,
                    z,
                    [0.4375, 0.0, 1.0 - t],
                    [0.5625, 1.0, 1.0],
                    block_id,
                    tint,
                );
            }
        }
        139 => add_wall_block(
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
        78 => {
            let layers = (block_meta(block_id) & 0x7) + 1;
            let h = (layers as f32 / 8.0).clamp(0.125, 1.0);
            add_box(
                batch,
                Some((snapshot, chunk_x, chunk_z, x, y, z, block_id)),
                texture_mapping,
                biome_tints,
                x,
                y,
                z,
                [0.0, 0.0, 0.0],
                [1.0, h, 1.0],
                block_id,
                tint,
            );
        }
        81 => add_box(
            batch,
            Some((snapshot, chunk_x, chunk_z, x, y, z, block_id)),
            texture_mapping,
            biome_tints,
            x,
            y,
            z,
            [1.0 / 16.0, 0.0, 1.0 / 16.0],
            [15.0 / 16.0, 1.0, 15.0 / 16.0],
            block_id,
            tint,
        ),
        88 => add_box(
            batch,
            Some((snapshot, chunk_x, chunk_z, x, y, z, block_id)),
            texture_mapping,
            biome_tints,
            x,
            y,
            z,
            [0.0, 0.0, 0.0],
            [1.0, 0.875, 1.0],
            block_id,
            tint,
        ),
        96 => {
            let meta = block_meta(block_id);
            let is_open = (meta & 0x4) != 0;
            let is_top = (meta & 0x8) != 0;
            let t = 3.0 / 16.0;
            let (min, max) = if is_open {
                match meta & 0x3 {
                    0 => ([0.0, 0.0, 1.0 - t], [1.0, 1.0, 1.0]),
                    1 => ([0.0, 0.0, 0.0], [1.0, 1.0, t]),
                    2 => ([1.0 - t, 0.0, 0.0], [1.0, 1.0, 1.0]),
                    _ => ([0.0, 0.0, 0.0], [t, 1.0, 1.0]),
                }
            } else if is_top {
                ([0.0, 1.0 - t, 0.0], [1.0, 1.0, 1.0])
            } else {
                ([0.0, 0.0, 0.0], [1.0, t, 1.0])
            };
            add_box(
                batch,
                Some((snapshot, chunk_x, chunk_z, x, y, z, block_id)),
                texture_mapping,
                biome_tints,
                x,
                y,
                z,
                min,
                max,
                block_id,
                tint,
            );
        }
        171 => add_box(
            batch,
            Some((snapshot, chunk_x, chunk_z, x, y, z, block_id)),
            texture_mapping,
            biome_tints,
            x,
            y,
            z,
            [0.0, 0.0, 0.0],
            [1.0, 1.0 / 16.0, 1.0],
            block_id,
            tint,
        ),
        69 => {
            add_box(
                batch,
                None,
                texture_mapping,
                biome_tints,
                x,
                y,
                z,
                [0.3125, 0.0, 0.3125],
                [0.6875, 0.1875, 0.6875],
                block_id,
                tint,
            );
            add_box(
                batch,
                None,
                texture_mapping,
                biome_tints,
                x,
                y,
                z,
                [0.4375, 0.1875, 0.4375],
                [0.5625, 0.75, 0.5625],
                block_id,
                tint,
            );
        }
        63 => add_sign_post_block(batch, texture_mapping, biome_tints, x, y, z, block_id, tint),
        68 => add_wall_sign_block(batch, texture_mapping, biome_tints, x, y, z, block_id, tint),
        140 => add_box(
            batch,
            None,
            texture_mapping,
            biome_tints,
            x,
            y,
            z,
            [0.3125, 0.0, 0.3125],
            [0.6875, 0.375, 0.6875],
            block_id,
            tint,
        ),
        144 => {
            let (min, max) = if (block_meta(block_id) & 0x7) == 1 {
                ([0.25, 0.25, 0.5], [0.75, 0.75, 1.0])
            } else {
                ([0.25, 0.0, 0.25], [0.75, 0.5, 0.75])
            };
            add_box(
                batch,
                None,
                texture_mapping,
                biome_tints,
                x,
                y,
                z,
                min,
                max,
                block_id,
                tint,
            );
        }
        166 => {
            if barrier_billboard {
                add_cross_plant(
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
                    voxel_ao_foliage_boost,
                );
            } else {
                add_box(
                    batch,
                    None,
                    texture_mapping,
                    biome_tints,
                    x,
                    y,
                    z,
                    [0.0, 0.0, 0.0],
                    [1.0, 1.0, 1.0],
                    block_id,
                    tint,
                );
            }
        }
        51 => add_cross_plant(
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
            voxel_ao_foliage_boost,
        ),
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn add_wall_block(
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
    let connect_east = wall_connects_to(block_at(snapshot, chunk_x, chunk_z, x + 1, y, z));
    let connect_west = wall_connects_to(block_at(snapshot, chunk_x, chunk_z, x - 1, y, z));
    let connect_south = wall_connects_to(block_at(snapshot, chunk_x, chunk_z, x, y, z + 1));
    let connect_north = wall_connects_to(block_at(snapshot, chunk_x, chunk_z, x, y, z - 1));
    let has_x = connect_east || connect_west;
    let has_z = connect_north || connect_south;
    let center_tall = !has_x || !has_z;

    add_box(
        batch,
        None,
        texture_mapping,
        biome_tints,
        x,
        y,
        z,
        [0.25, 0.0, 0.25],
        [0.75, if center_tall { 1.0 } else { 0.8125 }, 0.75],
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
            [0.3125, 0.0, 0.0],
            [0.6875, 0.8125, 0.5],
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
            [0.3125, 0.0, 0.5],
            [0.6875, 0.8125, 1.0],
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
            [0.0, 0.0, 0.3125],
            [0.5, 0.8125, 0.6875],
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
            [0.5, 0.0, 0.3125],
            [1.0, 0.8125, 0.6875],
            block_id,
            tint,
        );
    }
}

fn add_sign_post_block(
    batch: &mut MeshBatch,
    texture_mapping: &AtlasBlockMapping,
    biome_tints: &BiomeTintResolver,
    x: i32,
    y: i32,
    z: i32,
    block_id: u16,
    tint: BiomeTint,
) {
    let yaw = (block_meta(block_id) as f32) * std::f32::consts::TAU / 16.0;
    let sx = yaw.sin().abs();
    let sz = yaw.cos().abs();
    let half_thickness = 0.0625 + 0.0625 * sx.max(sz);
    let min_x = (0.5 - half_thickness).clamp(0.0, 1.0);
    let max_x = (0.5 + half_thickness).clamp(0.0, 1.0);
    let min_z = (0.5 - half_thickness).clamp(0.0, 1.0);
    let max_z = (0.5 + half_thickness).clamp(0.0, 1.0);
    add_box(
        batch,
        None,
        texture_mapping,
        biome_tints,
        x,
        y,
        z,
        [min_x, 0.5, min_z],
        [max_x, 1.0, max_z],
        block_id,
        tint,
    );
    add_box(
        batch,
        None,
        texture_mapping,
        biome_tints,
        x,
        y,
        z,
        [0.4375, 0.0, 0.4375],
        [0.5625, 0.5, 0.5625],
        block_id,
        tint,
    );
}

fn add_wall_sign_block(
    batch: &mut MeshBatch,
    texture_mapping: &AtlasBlockMapping,
    biome_tints: &BiomeTintResolver,
    x: i32,
    y: i32,
    z: i32,
    block_id: u16,
    tint: BiomeTint,
) {
    let (min, max) = match block_meta(block_id) & 0x7 {
        2 => ([0.0, 0.25, 0.875], [1.0, 0.875, 0.9375]),
        3 => ([0.0, 0.25, 0.0625], [1.0, 0.875, 0.125]),
        4 => ([0.875, 0.25, 0.0], [0.9375, 0.875, 1.0]),
        _ => ([0.0625, 0.25, 0.0], [0.125, 0.875, 1.0]),
    };
    add_box(
        batch,
        None,
        texture_mapping,
        biome_tints,
        x,
        y,
        z,
        min,
        max,
        block_id,
        tint,
    );
}
