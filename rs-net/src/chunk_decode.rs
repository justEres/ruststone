use rs_utils::{ChunkData, ChunkSection};

const SECTION_BLOCK_COUNT: usize = 16 * 16 * 16;
const SECTION_META_BYTES: usize = 2048;
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

    if full {
        let biomes_end = consumed + BIOME_BYTES;
        if biomes_end > data.len() {
            return Err("Chunk biome data underflow".to_string());
        }
        consumed = biomes_end;
    }

    Ok((
        ChunkData {
            x,
            z,
            full,
            sections,
        },
        consumed,
    ))
}

fn decode_sections(
    bitmask: u16,
    data: &[u8],
    skylight: bool,
) -> Result<(Vec<ChunkSection>, usize), String> {
    let mut sections = Vec::new();
    let mut offset = 0usize;

    for y in 0..16u8 {
        if (bitmask & (1 << y)) == 0 {
            continue;
        }

        let blocks_end = offset + SECTION_BLOCK_COUNT;
        if blocks_end > data.len() {
            return Err("Chunk section block data underflow".to_string());
        }
        let blocks = data[offset..blocks_end].to_vec();
        offset = blocks_end;

        let meta_end = offset + SECTION_META_BYTES;
        if meta_end > data.len() {
            return Err("Chunk section metadata underflow".to_string());
        }
        offset = meta_end;

        let light_end = offset + SECTION_LIGHT_BYTES;
        if light_end > data.len() {
            return Err("Chunk section block light underflow".to_string());
        }
        offset = light_end;

        if skylight {
            let sky_end = offset + SECTION_LIGHT_BYTES;
            if sky_end > data.len() {
                return Err("Chunk section sky light underflow".to_string());
            }
            offset = sky_end;
        }

        sections.push(ChunkSection { y, blocks });
    }

    Ok((sections, offset))
}
