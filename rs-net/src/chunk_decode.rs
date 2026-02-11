use rs_utils::{ChunkData, ChunkSection};

const SECTION_BLOCK_COUNT: usize = 16 * 16 * 16;
const SECTION_BLOCK_BYTES: usize = 8192;
const SECTION_LIGHT_BYTES: usize = 2048;
const BIOME_BYTES: usize = 256;

pub fn decode_chunk(
    x: i32,
    z: i32,
    full: bool,
    bitmask: u16,
    data: &[u8],
    skylight: bool,
) -> Result<(ChunkData, usize), String> {
    let (sections, mut consumed) = decode_sections(bitmask, data, skylight)?;
    let mut biomes = None;

    if full {
        let biomes_end = consumed + BIOME_BYTES;
        if biomes_end > data.len() {
            return Err("Chunk biome data underflow".to_string());
        }
        biomes = Some(data[consumed..biomes_end].to_vec());
        consumed = biomes_end;
    }

    Ok((
        ChunkData {
            x,
            z,
            full,
            sections,
            biomes,
        },
        consumed,
    ))
}

fn decode_sections(
    bitmask: u16,
    data: &[u8],
    skylight: bool,
) -> Result<(Vec<ChunkSection>, usize), String> {
    let section_count = bitmask.count_ones() as usize;
    let blocks_total = section_count * SECTION_BLOCK_BYTES;
    let block_light_total = section_count * SECTION_LIGHT_BYTES;
    let sky_light_total = if skylight {
        section_count * SECTION_LIGHT_BYTES
    } else {
        0
    };
    let expected_min = blocks_total + block_light_total + sky_light_total;
    if expected_min > data.len() {
        return Err("Chunk section data underflow".to_string());
    }

    let blocks_slice = &data[..blocks_total];

    let mut sections = Vec::new();
    let mut blocks_offset = 0usize;

    for y in 0..16u8 {
        if (bitmask & (1 << y)) == 0 {
            continue;
        }

        let block_end = blocks_offset + SECTION_BLOCK_BYTES;
        let section_bytes = &blocks_slice[blocks_offset..block_end];
        blocks_offset = block_end;

        let blocks = decode_block_ids(section_bytes)?;
        sections.push(ChunkSection { y, blocks });
    }

    Ok((sections, expected_min))
}

fn decode_block_ids(bytes: &[u8]) -> Result<Vec<u16>, String> {
    if bytes.len() != SECTION_BLOCK_BYTES {
        return Err("Unexpected block data size".to_string());
    }

    let mut blocks = Vec::with_capacity(SECTION_BLOCK_COUNT);
    for i in 0..SECTION_BLOCK_COUNT {
        let low = bytes[i * 2];
        let high = bytes[i * 2 + 1];
        // 1.8 chunk section stores block state as little-endian u16:
        // low 4 bits metadata, high 12 bits block id.
        let state = ((high as u16) << 8) | (low as u16);
        blocks.push(state);
    }

    Ok(blocks)
}
