use super::*;

pub(super) fn player_head_meshes(texture_debug: &PlayerTextureDebugSettings) -> Vec<Mesh> {
    vec![
        make_skin_box_with_faces(8.0, 8.0, 8.0, 0.0, head_base_face_rects(), texture_debug),
        make_skin_box_with_faces(8.0, 8.0, 8.0, 0.5, head_outer_face_rects(), texture_debug),
    ]
}

pub(super) fn player_body_meshes(texture_debug: &PlayerTextureDebugSettings) -> Vec<Mesh> {
    vec![
        make_skin_box_with_faces(8.0, 12.0, 4.0, 0.0, torso_base_face_rects(), texture_debug),
        make_skin_box_with_faces(
            8.0,
            12.0,
            4.0,
            0.25,
            torso_outer_face_rects(),
            texture_debug,
        ),
    ]
}

pub(super) fn player_arm_pivot_x(skin_model: PlayerSkinModel) -> f32 {
    // Vanilla uses +/-5px rotation points for both classic and slim arm models.
    let _ = skin_model;
    5.0 / 16.0
}

pub(super) fn player_arm_child_offset_x(skin_model: PlayerSkinModel, is_right: bool) -> f32 {
    // Slim (Alex) arms are 3px wide and shifted by 0.5px so they still connect to the body
    // the same way as classic arms.
    if skin_model != PlayerSkinModel::Slim {
        return 0.0;
    }
    let offset = 0.5 / 16.0;
    if is_right { offset } else { -offset }
}

pub(super) fn player_leg_pivot_x() -> f32 {
    1.9 / 16.0
}

pub(super) fn player_leg_pivot_y() -> f32 {
    12.0 / 16.0
}

pub(super) fn player_body_pivot_y() -> f32 {
    24.0 / 16.0
}

pub(super) fn player_arm_pivot_y() -> f32 {
    // Pivot at the shoulder height; mesh is offset so the arm "hangs" from this point.
    24.0 / 16.0
}

pub(super) fn player_head_pivot_y() -> f32 {
    // Neck (top of torso).
    24.0 / 16.0
}

pub(super) fn head_child_offset() -> Vec3 {
    // Pivot at neck; head cube extends upward.
    Vec3::new(0.0, 4.0 / 16.0, 0.0)
}

pub(super) fn torso_child_offset() -> Vec3 {
    // Pivot at shoulders (top of torso); cube extends downward.
    Vec3::new(0.0, -(6.0 / 16.0), 0.0)
}

pub(super) fn limb_child_offset() -> Vec3 {
    // Pivot at top; cube extends downward.
    Vec3::new(0.0, -(6.0 / 16.0), 0.0)
}

pub(super) fn first_person_arm_child_offset() -> Vec3 {
    // Pivot at shoulder; cube extends downward (12px total).
    Vec3::new(0.0, -(6.0 / 16.0), 0.0)
}

pub(super) fn player_head_pivot_y_sneak(amount: f32) -> f32 {
    // `ModelBiped`: head rotationPointY = 1.0 when sneaking (y-positive is down in MC model space),
    // so in our y-up space this is a small downward shift.
    player_head_pivot_y() - (1.0 / 16.0) * amount
}

pub(super) fn player_leg_pivot_y_sneak(amount: f32) -> f32 {
    // `ModelBiped`: legs rotationPointY = 9.0 when sneaking (from 12.0), i.e. +3px up in y-up space.
    player_leg_pivot_y() + (3.0 / 16.0) * amount
}

pub(super) fn player_leg_pivot_z_sneak(amount: f32) -> f32 {
    // `ModelBiped`: legs rotationPointZ = 4.0 when sneaking; in our "forward is -Z" space this is -4px.
    let stand = -0.1 / 16.0;
    let sneak = -4.0 / 16.0;
    stand.lerp(sneak, amount)
}

pub(super) fn player_right_arm_meshes(
    skin_model: PlayerSkinModel,
    texture_debug: &PlayerTextureDebugSettings,
) -> Vec<Mesh> {
    let arm_width = match skin_model {
        PlayerSkinModel::Slim => 3.0,
        PlayerSkinModel::Classic => 4.0,
    };
    vec![
        make_skin_box_with_faces(
            arm_width,
            12.0,
            4.0,
            0.0,
            right_arm_base_face_rects(skin_model),
            texture_debug,
        ),
        make_skin_box_with_faces(
            arm_width,
            12.0,
            4.0,
            0.25,
            right_arm_outer_face_rects(skin_model),
            texture_debug,
        ),
    ]
}

pub(super) fn player_left_arm_meshes(
    skin_model: PlayerSkinModel,
    texture_debug: &PlayerTextureDebugSettings,
) -> Vec<Mesh> {
    let arm_width = match skin_model {
        PlayerSkinModel::Slim => 3.0,
        PlayerSkinModel::Classic => 4.0,
    };
    vec![
        make_skin_box_with_faces(
            arm_width,
            12.0,
            4.0,
            0.0,
            left_arm_base_face_rects(skin_model),
            texture_debug,
        ),
        make_skin_box_with_faces(
            arm_width,
            12.0,
            4.0,
            0.25,
            left_arm_outer_face_rects(skin_model),
            texture_debug,
        ),
    ]
}

pub(super) fn player_right_leg_meshes(texture_debug: &PlayerTextureDebugSettings) -> Vec<Mesh> {
    vec![
        make_skin_box_with_faces(
            4.0,
            12.0,
            4.0,
            0.0,
            right_leg_base_face_rects(),
            texture_debug,
        ),
        make_skin_box_with_faces(
            4.0,
            12.0,
            4.0,
            0.25,
            right_leg_outer_face_rects(),
            texture_debug,
        ),
    ]
}

pub(super) fn player_left_leg_meshes(texture_debug: &PlayerTextureDebugSettings) -> Vec<Mesh> {
    vec![
        make_skin_box_with_faces(
            4.0,
            12.0,
            4.0,
            0.0,
            left_leg_base_face_rects(),
            texture_debug,
        ),
        make_skin_box_with_faces(
            4.0,
            12.0,
            4.0,
            0.25,
            left_leg_outer_face_rects(),
            texture_debug,
        ),
    ]
}

pub(super) fn make_skin_box_with_faces(
    w_px: f32,
    h_px: f32,
    d_px: f32,
    inflate_px: f32,
    faces: SkinFaceMap,
    texture_debug: &PlayerTextureDebugSettings,
) -> Mesh {
    let px = 1.0 / 16.0;
    let inflate = inflate_px * px;
    let hw = w_px * px * 0.5 + inflate;
    let hh = h_px * px * 0.5 + inflate;
    let hd = d_px * px * 0.5 + inflate;

    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    add_face(
        &mut positions,
        &mut normals,
        &mut uvs,
        &mut indices,
        [
            [hw, -hh, hd],
            [-hw, -hh, hd],
            [-hw, -hh, -hd],
            [hw, -hh, -hd],
        ],
        [0.0, -1.0, 0.0],
        faces.down,
        texture_debug,
    );
    add_face(
        &mut positions,
        &mut normals,
        &mut uvs,
        &mut indices,
        [[-hw, hh, -hd], [hw, hh, -hd], [hw, hh, hd], [-hw, hh, hd]],
        [0.0, 1.0, 0.0],
        faces.up,
        texture_debug,
    );
    add_face(
        &mut positions,
        &mut normals,
        &mut uvs,
        &mut indices,
        [
            [-hw, -hh, -hd],
            [hw, -hh, -hd],
            [hw, hh, -hd],
            [-hw, hh, -hd],
        ],
        [0.0, 0.0, -1.0],
        faces.north,
        texture_debug,
    );
    add_face(
        &mut positions,
        &mut normals,
        &mut uvs,
        &mut indices,
        [[hw, -hh, hd], [-hw, -hh, hd], [-hw, hh, hd], [hw, hh, hd]],
        [0.0, 0.0, 1.0],
        faces.south,
        texture_debug,
    );
    add_face(
        &mut positions,
        &mut normals,
        &mut uvs,
        &mut indices,
        [
            [-hw, -hh, -hd],
            [-hw, -hh, hd],
            [-hw, hh, hd],
            [-hw, hh, -hd],
        ],
        [-1.0, 0.0, 0.0],
        faces.west,
        texture_debug,
    );
    add_face(
        &mut positions,
        &mut normals,
        &mut uvs,
        &mut indices,
        [[hw, -hh, hd], [hw, -hh, -hd], [hw, hh, -hd], [hw, hh, hd]],
        [1.0, 0.0, 0.0],
        faces.east,
        texture_debug,
    );

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

#[derive(Clone, Copy)]
pub(super) struct SkinUvRect {
    u: f32,
    v: f32,
    w: f32,
    h: f32,
}

impl SkinUvRect {
    fn new(u: f32, v: f32, w: f32, h: f32) -> Self {
        Self { u, v, w, h }
    }
}

#[derive(Clone, Copy)]
pub(super) struct SkinFaceMap {
    down: SkinUvRect,
    up: SkinUvRect,
    north: SkinUvRect,
    south: SkinUvRect,
    west: SkinUvRect,
    east: SkinUvRect,
}

pub(super) fn rect(x1: f32, y1: f32, x2: f32, y2: f32) -> SkinUvRect {
    SkinUvRect::new(x1, y1, x2 - x1, y2 - y1)
}

pub(super) fn map_from_named_faces(
    top: SkinUvRect,
    bottom: SkinUvRect,
    left: SkinUvRect,
    front: SkinUvRect,
    right: SkinUvRect,
    back: SkinUvRect,
) -> SkinFaceMap {
    // Cube axes to named skin faces.
    // -Y -> bottom, +Y -> top, -Z -> front, +Z -> back, -X -> left, +X -> right
    SkinFaceMap {
        down: bottom,
        up: top,
        // Model root is rotated 180deg, so swap front/back UV assignment here.
        north: back,
        south: front,
        west: left,
        east: right,
    }
}

pub(super) fn head_base_face_rects() -> SkinFaceMap {
    map_from_named_faces(
        rect(8.0, 0.0, 16.0, 8.0),
        rect(16.0, 0.0, 24.0, 8.0),
        rect(0.0, 8.0, 8.0, 16.0),
        rect(8.0, 8.0, 16.0, 16.0),
        rect(16.0, 8.0, 24.0, 16.0),
        rect(24.0, 8.0, 32.0, 16.0),
    )
}

pub(super) fn head_outer_face_rects() -> SkinFaceMap {
    map_from_named_faces(
        rect(40.0, 0.0, 48.0, 8.0),
        rect(48.0, 0.0, 56.0, 8.0),
        rect(32.0, 8.0, 40.0, 16.0),
        rect(40.0, 8.0, 48.0, 16.0),
        rect(48.0, 8.0, 56.0, 16.0),
        rect(56.0, 8.0, 64.0, 16.0),
    )
}

pub(super) fn torso_base_face_rects() -> SkinFaceMap {
    map_from_named_faces(
        rect(20.0, 16.0, 28.0, 20.0),
        rect(28.0, 16.0, 36.0, 20.0),
        rect(16.0, 20.0, 20.0, 32.0),
        rect(20.0, 20.0, 28.0, 32.0),
        rect(28.0, 20.0, 32.0, 32.0),
        rect(32.0, 20.0, 40.0, 32.0),
    )
}

pub(super) fn torso_outer_face_rects() -> SkinFaceMap {
    map_from_named_faces(
        rect(20.0, 32.0, 28.0, 36.0),
        rect(28.0, 32.0, 36.0, 36.0),
        rect(16.0, 36.0, 20.0, 48.0),
        rect(20.0, 36.0, 28.0, 48.0),
        rect(28.0, 36.0, 32.0, 48.0),
        rect(32.0, 36.0, 40.0, 48.0),
    )
}

pub(super) fn right_arm_base_face_rects(model: PlayerSkinModel) -> SkinFaceMap {
    match model {
        PlayerSkinModel::Classic => map_from_named_faces(
            rect(44.0, 16.0, 48.0, 20.0),
            rect(48.0, 16.0, 52.0, 20.0),
            rect(40.0, 20.0, 44.0, 32.0),
            rect(44.0, 20.0, 48.0, 32.0),
            rect(48.0, 20.0, 52.0, 32.0),
            rect(52.0, 20.0, 56.0, 32.0),
        ),
        PlayerSkinModel::Slim => map_from_named_faces(
            rect(44.0, 16.0, 47.0, 20.0),
            rect(47.0, 16.0, 50.0, 20.0),
            rect(40.0, 20.0, 43.0, 32.0),
            rect(44.0, 20.0, 47.0, 32.0),
            rect(47.0, 20.0, 50.0, 32.0),
            rect(50.0, 20.0, 53.0, 32.0),
        ),
    }
}

pub(super) fn right_arm_outer_face_rects(model: PlayerSkinModel) -> SkinFaceMap {
    match model {
        PlayerSkinModel::Classic => map_from_named_faces(
            rect(44.0, 32.0, 48.0, 36.0),
            rect(48.0, 32.0, 52.0, 36.0),
            rect(40.0, 36.0, 44.0, 48.0),
            rect(44.0, 36.0, 48.0, 48.0),
            rect(48.0, 36.0, 52.0, 48.0),
            rect(52.0, 36.0, 56.0, 48.0),
        ),
        PlayerSkinModel::Slim => map_from_named_faces(
            rect(44.0, 32.0, 47.0, 36.0),
            rect(47.0, 32.0, 50.0, 36.0),
            rect(40.0, 36.0, 43.0, 48.0),
            rect(44.0, 36.0, 47.0, 48.0),
            rect(47.0, 36.0, 50.0, 48.0),
            rect(50.0, 36.0, 53.0, 48.0),
        ),
    }
}

pub(super) fn left_arm_base_face_rects(model: PlayerSkinModel) -> SkinFaceMap {
    match model {
        PlayerSkinModel::Classic => map_from_named_faces(
            rect(36.0, 48.0, 40.0, 52.0),
            rect(40.0, 48.0, 44.0, 52.0),
            rect(32.0, 52.0, 36.0, 64.0),
            rect(36.0, 52.0, 40.0, 64.0),
            rect(40.0, 52.0, 44.0, 64.0),
            rect(44.0, 52.0, 48.0, 64.0),
        ),
        PlayerSkinModel::Slim => map_from_named_faces(
            rect(36.0, 48.0, 39.0, 52.0),
            rect(39.0, 48.0, 42.0, 52.0),
            rect(32.0, 52.0, 35.0, 64.0),
            rect(36.0, 52.0, 39.0, 64.0),
            rect(39.0, 52.0, 42.0, 64.0),
            rect(42.0, 52.0, 45.0, 64.0),
        ),
    }
}

pub(super) fn left_arm_outer_face_rects(model: PlayerSkinModel) -> SkinFaceMap {
    match model {
        PlayerSkinModel::Classic => map_from_named_faces(
            rect(52.0, 48.0, 56.0, 52.0),
            rect(56.0, 48.0, 60.0, 52.0),
            rect(48.0, 52.0, 52.0, 64.0),
            rect(52.0, 52.0, 56.0, 64.0),
            rect(56.0, 52.0, 60.0, 64.0),
            rect(60.0, 52.0, 64.0, 64.0),
        ),
        PlayerSkinModel::Slim => map_from_named_faces(
            rect(52.0, 48.0, 55.0, 52.0),
            rect(55.0, 48.0, 58.0, 52.0),
            rect(48.0, 52.0, 51.0, 64.0),
            rect(52.0, 52.0, 55.0, 64.0),
            rect(55.0, 52.0, 58.0, 64.0),
            rect(58.0, 52.0, 61.0, 64.0),
        ),
    }
}

pub(super) fn right_leg_base_face_rects() -> SkinFaceMap {
    map_from_named_faces(
        rect(4.0, 16.0, 8.0, 20.0),
        rect(8.0, 16.0, 12.0, 20.0),
        rect(0.0, 20.0, 4.0, 32.0),
        rect(4.0, 20.0, 8.0, 32.0),
        rect(8.0, 20.0, 12.0, 32.0),
        rect(12.0, 20.0, 16.0, 32.0),
    )
}

pub(super) fn right_leg_outer_face_rects() -> SkinFaceMap {
    map_from_named_faces(
        rect(4.0, 32.0, 8.0, 36.0),
        rect(8.0, 32.0, 12.0, 36.0),
        rect(0.0, 36.0, 4.0, 48.0),
        rect(4.0, 36.0, 8.0, 48.0),
        rect(8.0, 36.0, 12.0, 48.0),
        rect(12.0, 36.0, 16.0, 48.0),
    )
}

pub(super) fn left_leg_base_face_rects() -> SkinFaceMap {
    map_from_named_faces(
        rect(20.0, 48.0, 24.0, 52.0),
        rect(24.0, 48.0, 28.0, 52.0),
        rect(16.0, 52.0, 20.0, 64.0),
        rect(20.0, 52.0, 24.0, 64.0),
        rect(24.0, 52.0, 28.0, 64.0),
        rect(28.0, 52.0, 32.0, 64.0),
    )
}

pub(super) fn left_leg_outer_face_rects() -> SkinFaceMap {
    map_from_named_faces(
        rect(4.0, 48.0, 8.0, 52.0),
        rect(8.0, 48.0, 12.0, 52.0),
        rect(0.0, 52.0, 4.0, 64.0),
        rect(4.0, 52.0, 8.0, 64.0),
        rect(8.0, 52.0, 12.0, 64.0),
        rect(12.0, 52.0, 16.0, 64.0),
    )
}

pub(super) fn add_face(
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    uvs: &mut Vec<[f32; 2]>,
    indices: &mut Vec<u32>,
    verts: [[f32; 3]; 4],
    normal: [f32; 3],
    rect: SkinUvRect,
    _texture_debug: &PlayerTextureDebugSettings,
) {
    let mut verts = verts;
    let mut uv = uv_rect(rect);

    // Keep face winding consistent with the provided normal so both triangles
    // are front-facing together (fixes diagonal half-quad culling).
    let a = Vec3::from_array(verts[0]);
    let b = Vec3::from_array(verts[1]);
    let c = Vec3::from_array(verts[2]);
    let actual = (b - a).cross(c - a);
    let expected = Vec3::from_array(normal);
    if actual.dot(expected) < 0.0 {
        verts = [verts[0], verts[3], verts[2], verts[1]];
        uv = [uv[0], uv[3], uv[2], uv[1]];
    }

    let base = positions.len() as u32;
    for i in 0..4 {
        positions.push(verts[i]);
        normals.push(normal);
        uvs.push(uv[i]);
    }
    indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
}

pub(super) fn uv_rect(rect: SkinUvRect) -> [[f32; 2]; 4] {
    let u0 = rect.u / 64.0;
    let u1 = (rect.u + rect.w) / 64.0;
    let v0 = rect.v / 64.0;
    let v1 = (rect.v + rect.h) / 64.0;
    [[u0, v1], [u1, v1], [u1, v0], [u0, v0]]
}

pub(super) fn player_root_rotation(yaw: f32) -> Quat {
    // Align model forward/back with Minecraft protocol-facing direction.
    Quat::from_axis_angle(Vec3::Y, yaw + std::f32::consts::PI)
}

pub(super) fn entity_root_rotation(kind: NetEntityKind, yaw: f32) -> Quat {
    match kind {
        // Player skin model pipeline has its own historic 180deg alignment.
        NetEntityKind::Player => player_root_rotation(yaw),
        // Vanilla mob models use protocol yaw directly.
        NetEntityKind::Mob(m) if mob_uses_entity_model(m) => Quat::from_axis_angle(Vec3::Y, yaw),
        _ => Quat::from_axis_angle(Vec3::Y, yaw),
    }
}

pub(super) fn visual_for_kind(kind: NetEntityKind) -> RemoteVisual {
    RemoteVisual {
        y_offset: visual_spec(kind).y_offset,
        name_y_offset: visual_spec(kind).name_y_offset,
    }
}

pub(super) fn mob_biped_model(mob: MobKind) -> &'static crate::model::ModelDef {
    match mob_biped_model_kind(mob) {
        // Vanilla uses mixed 64x32 and 64x64 biped textures in 1.8.9.
        BipedModelKind::Tex32 => &BIPED_MODEL_TEX32,
        BipedModelKind::Tex64 => &BIPED_MODEL_TEX64,
    }
}

pub(super) fn mob_quadruped_model(mob: MobKind) -> &'static crate::model::ModelDef {
    match mob_quadruped_model_kind(mob) {
        QuadrupedModelKind::PigTex32 => &PIG_MODEL_TEX32,
        QuadrupedModelKind::SheepTex32 => &SHEEP_MODEL_TEX32,
        QuadrupedModelKind::CowTex32 => &COW_MODEL_TEX32,
        QuadrupedModelKind::CreeperTex64 => &CREEPER_MODEL_TEX64,
    }
}
