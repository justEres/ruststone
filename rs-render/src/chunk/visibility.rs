use super::*;

pub(super) fn block_type(block_state: u16) -> u16 {
    block_state_id(block_state)
}

pub(super) fn block_meta(block_state: u16) -> u8 {
    block_state_meta(block_state)
}

pub(super) const fn block_state_from_id(block_id: u16) -> u16 {
    block_id << 4
}

pub(super) fn is_liquid(block_id: u16) -> bool {
    matches!(block_type(block_id), 8 | 9 | 10 | 11)
}

pub(super) fn should_apply_prebaked_shade(block_id: u16) -> bool {
    !matches!(
        render_group_for_block(block_id),
        MaterialGroup::Cutout | MaterialGroup::CutoutCulled
    )
}

pub(super) fn render_group_for_block(block_id: u16) -> MaterialGroup {
    let id = block_type(block_id);
    if is_transparent_block(id) {
        return MaterialGroup::Transparent;
    }
    if is_leaves_block(id) {
        return MaterialGroup::CutoutCulled;
    }
    if matches!(id, 20 | 95 | 160) {
        return MaterialGroup::CutoutCulled;
    }
    if matches!(
        block_model_kind(block_type(block_id)),
        BlockModelKind::Cross | BlockModelKind::Pane | BlockModelKind::TorchLike
    ) {
        return MaterialGroup::Cutout;
    }
    if matches!(
        id,
        26 | 27
            | 28
            | 51
            | 63
            | 64
            | 65
            | 66
            | 68
            | 69
            | 71
            | 96
            | 106
            | 140
            | 144
            | 157
            | 166
            | 193
            | 194
            | 195
            | 196
            | 197
    ) {
        return MaterialGroup::Cutout;
    }
    MaterialGroup::Opaque
}

pub(super) fn is_occluding_block(block_id: u16) -> bool {
    let id = block_type(block_id);
    if id == 0 {
        return false;
    }
    if is_liquid(block_id) {
        return true;
    }
    if is_alpha_cutout_cube(id) {
        return false;
    }
    !is_custom_block(block_id)
}

pub(super) fn is_alpha_cutout_cube(id: u16) -> bool {
    is_leaves_block(id) || matches!(id, 20 | 95 | 160)
}

pub(super) fn fence_connects_to(neighbor_state: u16) -> bool {
    let neighbor_id = block_type(neighbor_state);
    if neighbor_id == 0 || is_liquid(neighbor_state) {
        return false;
    }
    if matches!(block_model_kind(neighbor_id), BlockModelKind::Fence) {
        return true;
    }
    if matches!(neighbor_id, 107 | 183 | 184 | 185 | 186 | 187) {
        return true;
    }
    is_occluding_block(neighbor_state)
}

pub(super) fn pane_connects_to(neighbor_state: u16) -> bool {
    let neighbor_id = block_type(neighbor_state);
    if neighbor_id == 0 || is_liquid(neighbor_state) {
        return false;
    }
    if matches!(block_model_kind(neighbor_id), BlockModelKind::Pane) {
        return true;
    }
    if matches!(neighbor_id, 20 | 95 | 101 | 102 | 160) {
        return true;
    }
    is_occluding_block(neighbor_state)
}

pub(super) fn wall_connects_to(neighbor_state: u16) -> bool {
    let neighbor_id = block_type(neighbor_state);
    if neighbor_id == 0 || is_liquid(neighbor_state) {
        return false;
    }
    if neighbor_id == 139 {
        return true;
    }
    if matches!(block_model_kind(neighbor_id), BlockModelKind::Fence) {
        return true;
    }
    if matches!(neighbor_id, 107 | 183 | 184 | 185 | 186 | 187) {
        return true;
    }
    is_occluding_block(neighbor_state)
}

pub(super) fn face_is_occluded(
    block_id: u16,
    neighbor_id: u16,
    leaf_depth_layer_faces: bool,
) -> bool {
    if block_type(neighbor_id) == 0 {
        return false;
    }
    if is_liquid(block_id) {
        return true;
    }
    if is_liquid(neighbor_id) {
        return is_liquid(block_id);
    }
    let this_type = block_type(block_id);
    let neighbor_type = block_type(neighbor_id);
    if is_transparent_block(this_type) || is_transparent_block(neighbor_type) {
        return this_type == neighbor_type && block_id == neighbor_id;
    }

    if leaf_depth_layer_faces && is_leaves_block(this_type) && is_leaves_block(neighbor_type) {
        return false;
    }

    if is_alpha_cutout_cube(neighbor_type) {
        return this_type == neighbor_type && block_id == neighbor_id;
    }
    is_occluding_block(neighbor_id)
}
