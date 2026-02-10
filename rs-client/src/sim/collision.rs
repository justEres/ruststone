use std::collections::HashMap;

use bevy::prelude::Resource;
use rs_utils::{BlockUpdate, ChunkData};

const CHUNK_SIZE: i32 = 16;
const SECTION_HEIGHT: i32 = 16;
const WORLD_HEIGHT: i32 = 256;

#[derive(Clone, Default)]
struct ChunkColumn {
    sections: Vec<Option<Vec<u16>>>,
    full: bool,
}

impl ChunkColumn {
    fn new(full: bool) -> Self {
        Self {
            sections: vec![None; (WORLD_HEIGHT / SECTION_HEIGHT) as usize],
            full,
        }
    }

    fn set_section(&mut self, y: u8, blocks: Vec<u16>) {
        let idx = y as usize;
        if idx >= self.sections.len() {
            return;
        }
        self.sections[idx] = Some(blocks);
    }

    fn set_block(&mut self, local_x: usize, y: i32, local_z: usize, block_id: u16) {
        if !(0..WORLD_HEIGHT).contains(&y) {
            return;
        }
        let section_index = (y / SECTION_HEIGHT) as usize;
        let local_y = (y % SECTION_HEIGHT) as usize;
        if section_index >= self.sections.len() {
            return;
        }
        let section = self.sections[section_index].get_or_insert_with(|| vec![0; 16 * 16 * 16]);
        let idx = local_y * 16 * 16 + local_z * 16 + local_x;
        if let Some(slot) = section.get_mut(idx) {
            *slot = block_id;
        }
    }
}

#[derive(Resource, Default)]
pub struct WorldCollisionMap {
    chunks: HashMap<(i32, i32), ChunkColumn>,
}

impl WorldCollisionMap {
    pub fn update_chunk(&mut self, chunk: ChunkData) {
        let key = (chunk.x, chunk.z);
        let entry = self
            .chunks
            .entry(key)
            .or_insert_with(|| ChunkColumn::new(chunk.full));
        if chunk.full {
            *entry = ChunkColumn::new(true);
        }
        for section in chunk.sections {
            entry.set_section(section.y, section.blocks);
        }
    }

    pub fn block_at(&self, x: i32, y: i32, z: i32) -> u16 {
        if y < 0 {
            return 1;
        }
        if y >= WORLD_HEIGHT {
            return 0;
        }

        let chunk_x = x.div_euclid(CHUNK_SIZE);
        let chunk_z = z.div_euclid(CHUNK_SIZE);
        let local_x = x.rem_euclid(CHUNK_SIZE);
        let local_z = z.rem_euclid(CHUNK_SIZE);

        let Some(column) = self.chunks.get(&(chunk_x, chunk_z)) else {
            return 0;
        };

        let section_index = (y / SECTION_HEIGHT) as usize;
        let local_y = (y % SECTION_HEIGHT) as usize;
        let Some(section) = column.sections.get(section_index).and_then(|v| v.as_ref()) else {
            return 0;
        };

        let idx = local_y * 16 * 16 + local_z as usize * 16 + local_x as usize;
        *section.get(idx).unwrap_or(&0)
    }

    pub fn apply_block_update(&mut self, update: BlockUpdate) {
        if !(0..WORLD_HEIGHT).contains(&update.y) {
            return;
        }
        let chunk_x = update.x.div_euclid(CHUNK_SIZE);
        let chunk_z = update.z.div_euclid(CHUNK_SIZE);
        let local_x = update.x.rem_euclid(CHUNK_SIZE) as usize;
        let local_z = update.z.rem_euclid(CHUNK_SIZE) as usize;
        let column = self
            .chunks
            .entry((chunk_x, chunk_z))
            .or_insert_with(|| ChunkColumn::new(false));
        column.set_block(local_x, update.y, local_z, update.block_id);
    }
}

pub fn is_solid(block_id: u16) -> bool {
    match block_id {
        0 => false,       // air
        8 | 9 => false,   // water
        10 | 11 => false, // lava
        _ => true,
    }
}
