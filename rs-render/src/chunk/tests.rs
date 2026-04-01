use super::*;
use rs_utils::ChunkSection;

fn full_chunk_with_section(y: u8, fill: u16) -> ChunkData {
    ChunkData {
        x: 0,
        z: 0,
        full: true,
        sections: vec![ChunkSection {
            y,
            blocks: vec![fill; 16 * 16 * 16],
            block_light: vec![0; 16 * 16 * 16],
            sky_light: Some(vec![15; 16 * 16 * 16]),
        }],
        biomes: None,
    }
}

#[test]
fn full_chunk_update_replaces_old_render_sections() {
    let mut store = ChunkStore::default();
    update_store(&mut store, full_chunk_with_section(5, 1 << 4));
    let chunk = store.chunks.get(&(0, 0)).unwrap();
    assert_eq!(chunk.sections[5].as_ref().unwrap()[0], 1 << 4);

    update_store(&mut store, full_chunk_with_section(0, 2 << 4));
    let chunk = store.chunks.get(&(0, 0)).unwrap();
    assert_eq!(chunk.sections[0].as_ref().unwrap()[0], 2 << 4);
    assert_eq!(chunk.sections[5].as_ref().unwrap()[0], 0);
}

#[test]
fn snow_layers_do_not_occlude_ao_neighbors() {
    assert!(!is_ao_occluder(78 << 4));
    assert!(is_ao_occluder(80 << 4));
}

#[test]
fn box_face_uvs_scale_and_offset_for_partial_boxes() {
    assert_eq!(
        box_face_uvs(Face::PosX, [0.0, 0.0, 0.0], [1.0, 0.5, 1.0]),
        [[0.0, 0.0], [1.0, 0.0], [1.0, 0.5], [0.0, 0.5]]
    );
    assert_eq!(
        box_face_uvs(Face::PosX, [0.0, 0.5, 0.0], [1.0, 1.0, 1.0]),
        [[0.0, 0.5], [1.0, 0.5], [1.0, 1.0], [0.0, 1.0]]
    );
    assert_eq!(
        box_face_uvs(Face::PosZ, [0.375, 0.0, 0.0], [0.625, 1.0, 1.0]),
        [[0.375, 0.0], [0.625, 0.0], [0.625, 1.0], [0.375, 1.0]]
    );
}
