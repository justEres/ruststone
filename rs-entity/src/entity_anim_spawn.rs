use super::*;

pub fn rebuild_remote_player_meshes_on_texture_debug_change(
    settings: Res<PlayerTextureDebugSettings>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    query: Query<
        (
            &RemotePlayerModelParts,
            &RemotePlayerSkinMaterials,
            &RemotePlayerSkinModel,
        ),
        With<RemotePlayer>,
    >,
    children_query: Query<&Children>,
) {
    if !settings.is_changed() {
        return;
    }
    for (parts, mats, skin_model) in &query {
        let Some(base_material) = mats.0.first() else {
            continue;
        };
        rebuild_part_children(
            &mut commands,
            &mut meshes,
            &children_query,
            parts.head,
            base_material,
            player_head_meshes(&settings),
            head_child_offset(),
        );
        rebuild_part_children(
            &mut commands,
            &mut meshes,
            &children_query,
            parts.body,
            base_material,
            player_body_meshes(&settings),
            torso_child_offset(),
        );
        rebuild_part_children(
            &mut commands,
            &mut meshes,
            &children_query,
            parts.arm_left,
            base_material,
            player_left_arm_meshes(skin_model.0, &settings),
            Vec3::new(
                player_arm_child_offset_x(skin_model.0, false),
                limb_child_offset().y,
                0.0,
            ),
        );
        rebuild_part_children(
            &mut commands,
            &mut meshes,
            &children_query,
            parts.arm_right,
            base_material,
            player_right_arm_meshes(skin_model.0, &settings),
            Vec3::new(
                player_arm_child_offset_x(skin_model.0, true),
                limb_child_offset().y,
                0.0,
            ),
        );
        rebuild_part_children(
            &mut commands,
            &mut meshes,
            &children_query,
            parts.leg_left,
            base_material,
            player_left_leg_meshes(&settings),
            limb_child_offset(),
        );
        rebuild_part_children(
            &mut commands,
            &mut meshes,
            &children_query,
            parts.leg_right,
            base_material,
            player_right_leg_meshes(&settings),
            limb_child_offset(),
        );
    }
}

pub(super) fn rebuild_part_children(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    children_query: &Query<&Children>,
    pivot: Entity,
    base_material: &Handle<StandardMaterial>,
    part_meshes: Vec<Mesh>,
    child_offset: Vec3,
) {
    if let Ok(children) = children_query.get(pivot) {
        for child in children.iter() {
            commands.entity(child).despawn_recursive();
        }
    }
    for mesh in part_meshes {
        let child = commands
            .spawn((
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(base_material.clone()),
                Transform::from_translation(child_offset),
                GlobalTransform::default(),
                Visibility::Visible,
                InheritedVisibility::default(),
                ViewVisibility::default(),
            ))
            .id();
        commands.entity(pivot).add_child(child);
    }
}

pub fn animate_remote_player_models(
    time: Res<Time>,
    mut roots: Query<
        (
            &Transform,
            &RemoteEntityLook,
            &RemotePoseState,
            &RemotePlayerModelParts,
            &RemotePlayerSkinModel,
            &mut RemotePlayerAnimation,
        ),
        With<RemotePlayer>,
    >,
    mut part_transforms: Query<&mut Transform, Without<RemotePlayer>>,
) {
    let dt = time.delta_secs().max(1e-4);
    for (root_transform, look, pose, parts, skin_model, mut anim) in &mut roots {
        let pos = root_transform.translation;
        let horizontal_delta = Vec2::new(pos.x - anim.previous_pos.x, pos.z - anim.previous_pos.z);
        let speed = (horizontal_delta.length() / dt).min(8.0);
        let stride = (speed / 4.0).clamp(0.0, 1.0);
        anim.walk_phase += speed * dt * 2.5;
        anim.swing_progress = (anim.swing_progress + dt * 3.6).min(1.0);
        anim.hurt_progress = (anim.hurt_progress + dt * 4.0).min(1.0);
        anim.previous_pos = pos;

        let swing = anim.walk_phase.sin() * 0.7 * stride;
        let head_pitch = look.pitch.clamp(-1.4, 1.4);
        let mut head_yaw_delta = look.head_yaw - look.yaw;
        head_yaw_delta = (head_yaw_delta + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;
        let sneak_amount = if pose.sneaking { 1.0 } else { 0.0 };
        let arm_attack = if anim.swing_progress < 1.0 {
            (anim.swing_progress * std::f32::consts::PI).sin() * 1.2
        } else {
            0.0
        };
        let hurt_tilt = if anim.hurt_progress < 1.0 {
            (1.0 - anim.hurt_progress) * 0.12
        } else {
            0.0
        };
        let arm_x = player_arm_pivot_x(skin_model.0);
        let leg_x = player_leg_pivot_x();
        let leg_y = player_leg_pivot_y_sneak(sneak_amount);
        let leg_z = player_leg_pivot_z_sneak(sneak_amount);

        if let Ok(mut t) = part_transforms.get_mut(parts.head) {
            t.translation = Vec3::new(0.0, player_head_pivot_y_sneak(sneak_amount), 0.0);
            t.rotation = Quat::from_rotation_y(head_yaw_delta)
                * Quat::from_rotation_x(-head_pitch - 0.2 * sneak_amount);
        }
        if let Ok(mut t) = part_transforms.get_mut(parts.body) {
            t.translation = Vec3::new(0.0, player_body_pivot_y(), 0.0);
            t.rotation =
                Quat::from_rotation_x(0.5 * sneak_amount) * Quat::from_rotation_z(hurt_tilt);
        }
        if let Ok(mut t) = part_transforms.get_mut(parts.arm_left) {
            t.translation = Vec3::new(-arm_x, player_arm_pivot_y(), 0.0);
            t.rotation = Quat::from_rotation_x(swing + 0.4 * sneak_amount);
        }
        if let Ok(mut t) = part_transforms.get_mut(parts.arm_right) {
            t.translation = Vec3::new(arm_x, player_arm_pivot_y(), 0.0);
            t.rotation = Quat::from_rotation_x(-swing - arm_attack + 0.4 * sneak_amount);
        }
        if let Ok(mut t) = part_transforms.get_mut(parts.leg_left) {
            t.translation = Vec3::new(-leg_x, leg_y, leg_z);
            t.rotation = Quat::from_rotation_x(-swing * (1.0 - 0.6 * sneak_amount));
        }
        if let Ok(mut t) = part_transforms.get_mut(parts.leg_right) {
            t.translation = Vec3::new(leg_x, leg_y, leg_z);
            t.rotation = Quat::from_rotation_x(swing * (1.0 - 0.6 * sneak_amount));
        }
    }
}

pub fn animate_remote_biped_models(
    time: Res<Time>,
    mut roots: Query<
        (
            &Transform,
            &RemoteEntityLook,
            &RemotePoseState,
            &RemoteBipedModelParts,
            &mut RemoteBipedAnimation,
        ),
        With<RemoteBipedModelParts>,
    >,
    mut part_transforms: Query<&mut Transform, Without<RemoteBipedModelParts>>,
) {
    // Core 1.8.9 `ModelBiped#setRotationAngles` behavior for remote entities.
    let dt = time.delta_secs().max(1e-4);
    let px = 1.0 / 16.0;

    for (root_transform, look, pose, parts, mut anim) in &mut roots {
        let pos = root_transform.translation;
        let horizontal_delta = Vec2::new(pos.x - anim.previous_pos.x, pos.z - anim.previous_pos.z);
        let speed = (horizontal_delta.length() / dt).min(10.0);
        anim.previous_pos = pos;

        anim.limb_swing_amount = (speed / 4.0).clamp(0.0, 1.0);
        anim.limb_swing += speed * dt * 1.3;

        if anim.swing_progress < 1.0 {
            anim.swing_progress = (anim.swing_progress + dt * 3.6).min(1.0);
        }

        let limb_swing = anim.limb_swing;
        let limb_swing_amount = anim.limb_swing_amount;

        let head_pitch = look.pitch.clamp(-1.4, 1.4);
        let mut head_yaw_delta = look.head_yaw - look.yaw;
        head_yaw_delta = (head_yaw_delta + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;

        // Vanilla constants.
        let right_arm_x =
            (limb_swing * 0.6662 + std::f32::consts::PI).cos() * 2.0 * limb_swing_amount * 0.5;
        let left_arm_x = (limb_swing * 0.6662).cos() * 2.0 * limb_swing_amount * 0.5;
        let right_leg_x = (limb_swing * 0.6662).cos() * 1.4 * limb_swing_amount;
        let left_leg_x =
            (limb_swing * 0.6662 + std::f32::consts::PI).cos() * 1.4 * limb_swing_amount;

        let mut body_yaw = 0.0f32;
        let mut arm_r_yaw = 0.0f32;
        let mut arm_l_yaw = 0.0f32;
        let mut arm_r_z = 0.0f32;
        let arm_l_z = 0.0f32;
        let mut arm_r_x = right_arm_x;
        let mut arm_l_x = left_arm_x;

        // Swing attack (main hand).
        if anim.swing_progress < 1.0 {
            let f = anim.swing_progress;
            body_yaw = (f.sqrt() * std::f32::consts::PI * 2.0).sin() * 0.2;
            arm_r_yaw += body_yaw;
            arm_l_yaw += body_yaw;
            arm_l_x += body_yaw;

            let mut f0 = 1.0 - f;
            f0 = f0 * f0;
            f0 = f0 * f0;
            f0 = 1.0 - f0;
            let f1 = (f0 * std::f32::consts::PI).sin();
            let f2 = (f * std::f32::consts::PI).sin() * -(-head_pitch - 0.7) * 0.75;
            arm_r_x = arm_r_x - (f1 * 1.2 + f2);
            arm_r_yaw += body_yaw * 2.0;
            arm_r_z += (f * std::f32::consts::PI).sin() * -0.4;
        }

        let is_sneak = pose.sneaking;
        let body_x = if is_sneak { 0.5 } else { 0.0 };
        if is_sneak {
            arm_r_x += 0.4;
            arm_l_x += 0.4;
        }

        // Pivots (vanilla model pixels; +Y down => bevy Y is negative).
        let (arm_y, leg_y, leg_z, head_y) = if is_sneak {
            (2.0, 9.0, 4.0, 1.0)
        } else {
            (2.0, 12.0, 0.1, 0.0)
        };

        if let Ok(mut t) = part_transforms.get_mut(parts.head) {
            t.translation = Vec3::new(0.0, -head_y * px, 0.0);
            t.rotation = Quat::from_rotation_y(head_yaw_delta) * Quat::from_rotation_x(-head_pitch);
        }
        if let Ok(mut t) = part_transforms.get_mut(parts.body) {
            t.translation = Vec3::ZERO;
            t.rotation = Quat::from_rotation_y(body_yaw) * Quat::from_rotation_x(body_x);
        }
        if let Ok(mut t) = part_transforms.get_mut(parts.arm_right) {
            t.translation = Vec3::new(-5.0 * px, -arm_y * px, 0.0);
            t.rotation = Quat::from_rotation_y(arm_r_yaw)
                * Quat::from_rotation_z(arm_r_z)
                * Quat::from_rotation_x(arm_r_x);
        }
        if let Ok(mut t) = part_transforms.get_mut(parts.arm_left) {
            t.translation = Vec3::new(5.0 * px, -arm_y * px, 0.0);
            t.rotation = Quat::from_rotation_y(arm_l_yaw)
                * Quat::from_rotation_z(arm_l_z)
                * Quat::from_rotation_x(arm_l_x);
        }
        if let Ok(mut t) = part_transforms.get_mut(parts.leg_right) {
            t.translation = Vec3::new(-1.9 * px, -leg_y * px, leg_z * px);
            t.rotation = Quat::from_rotation_x(right_leg_x);
        }
        if let Ok(mut t) = part_transforms.get_mut(parts.leg_left) {
            t.translation = Vec3::new(1.9 * px, -leg_y * px, leg_z * px);
            t.rotation = Quat::from_rotation_x(left_leg_x);
        }
    }
}

pub fn animate_remote_quadruped_models(
    time: Res<Time>,
    mut roots: Query<
        (
            &Transform,
            &RemoteEntityLook,
            &RemoteQuadrupedModelParts,
            &RemoteQuadrupedAnimTuning,
            &mut RemoteQuadrupedAnimation,
        ),
        With<RemoteQuadrupedModelParts>,
    >,
    mut part_transforms: Query<&mut Transform, Without<RemoteQuadrupedModelParts>>,
) {
    let dt = time.delta_secs().max(1e-4);
    let pose_alpha = 1.0 - (-22.0 * dt).exp();
    let swing_alpha = 1.0 - (-12.0 * dt).exp();

    for (root_transform, look, parts, tuning, mut anim) in &mut roots {
        let pos = root_transform.translation;
        let horizontal_delta = Vec2::new(pos.x - anim.previous_pos.x, pos.z - anim.previous_pos.z);
        let speed = (horizontal_delta.length() / dt).min(10.0);
        anim.previous_pos = pos;

        let target_limb_swing_amount = (speed / 4.0).clamp(0.0, 1.0);
        anim.limb_swing_amount += (target_limb_swing_amount - anim.limb_swing_amount) * swing_alpha;
        anim.limb_swing += speed * dt * 1.3;

        let head_pitch = look.pitch.clamp(-1.4, 1.4);
        let mut head_yaw_delta = look.head_yaw - look.yaw;
        head_yaw_delta = (head_yaw_delta + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;

        let swing_scale = 1.4 * tuning.leg_swing_scale;
        let leg_front_right_x =
            (anim.limb_swing * 0.6662).cos() * swing_scale * anim.limb_swing_amount;
        let leg_front_left_x = (anim.limb_swing * 0.6662 + std::f32::consts::PI).cos()
            * swing_scale
            * anim.limb_swing_amount;
        let leg_back_right_x = (anim.limb_swing * 0.6662 + std::f32::consts::PI).cos()
            * swing_scale
            * anim.limb_swing_amount;
        let leg_back_left_x =
            (anim.limb_swing * 0.6662).cos() * swing_scale * anim.limb_swing_amount;

        let head_target =
            Quat::from_rotation_y(head_yaw_delta) * Quat::from_rotation_x(-head_pitch);
        let body_target = Quat::from_rotation_x(tuning.body_pitch);
        let leg_fr_target = Quat::from_rotation_x(leg_front_right_x);
        let leg_fl_target = Quat::from_rotation_x(leg_front_left_x);
        let leg_br_target = Quat::from_rotation_x(leg_back_right_x);
        let leg_bl_target = Quat::from_rotation_x(leg_back_left_x);

        if let Ok(mut t) = part_transforms.get_mut(parts.head) {
            t.rotation = t.rotation.slerp(head_target, pose_alpha);
        }
        if let Ok(mut t) = part_transforms.get_mut(parts.body) {
            t.rotation = t.rotation.slerp(body_target, pose_alpha);
        }
        if let Ok(mut t) = part_transforms.get_mut(parts.leg_front_right) {
            t.rotation = t.rotation.slerp(leg_fr_target, pose_alpha);
        }
        if let Ok(mut t) = part_transforms.get_mut(parts.leg_front_left) {
            t.rotation = t.rotation.slerp(leg_fl_target, pose_alpha);
        }
        if let Ok(mut t) = part_transforms.get_mut(parts.leg_back_right) {
            t.rotation = t.rotation.slerp(leg_br_target, pose_alpha);
        }
        if let Ok(mut t) = part_transforms.get_mut(parts.leg_back_left) {
            t.rotation = t.rotation.slerp(leg_bl_target, pose_alpha);
        }
    }
}

pub(super) fn spawn_sheep_wool_layer(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    quadruped_parts: Vec<Entity>,
    material: Handle<StandardMaterial>,
) -> [Entity; 6] {
    let mut wool_mesh_entities = [Entity::PLACEHOLDER; 6];
    for (idx, part) in SHEEP_WOOL_MODEL_TEX32.parts.iter().enumerate() {
        let target_part = quadruped_parts[idx];
        let mesh = meshes.add(part_mesh(&SHEEP_WOOL_MODEL_TEX32, part));
        let wool_mesh_entity = commands
            .spawn((
                Name::new(format!("EntityModelMesh[{}]", part.name)),
                Mesh3d(mesh),
                MeshMaterial3d(material.clone()),
                Transform::IDENTITY,
                GlobalTransform::default(),
                Visibility::Visible,
                InheritedVisibility::default(),
                ViewVisibility::default(),
            ))
            .id();
        commands.entity(target_part).add_child(wool_mesh_entity);
        wool_mesh_entities[idx] = wool_mesh_entity;
    }
    wool_mesh_entities
}

pub(super) fn sheep_fleece_rgb(fleece_color: u8) -> [f32; 3] {
    match fleece_color & 0x0F {
        0 => [1.0, 1.0, 1.0],
        1 => [0.85, 0.5, 0.2],
        2 => [0.7, 0.3, 0.85],
        3 => [0.4, 0.6, 0.85],
        4 => [0.9, 0.9, 0.2],
        5 => [0.5, 0.8, 0.1],
        6 => [0.95, 0.5, 0.65],
        7 => [0.3, 0.3, 0.3],
        8 => [0.6, 0.6, 0.6],
        9 => [0.3, 0.5, 0.6],
        10 => [0.5, 0.25, 0.7],
        11 => [0.2, 0.3, 0.7],
        12 => [0.4, 0.3, 0.2],
        13 => [0.4, 0.5, 0.2],
        14 => [0.6, 0.2, 0.2],
        15 => [0.1, 0.1, 0.1],
        _ => [1.0, 1.0, 1.0],
    }
}

pub(super) fn spawn_remote_player_model(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    player_skin: Option<Handle<Image>>,
    skin_model: PlayerSkinModel,
    texture_debug: &PlayerTextureDebugSettings,
) -> (RemotePlayerModelParts, Vec<Handle<StandardMaterial>>) {
    let base_mat = materials.add(StandardMaterial {
        base_color: if player_skin.is_some() {
            Color::WHITE
        } else {
            Color::srgb(0.85, 0.78, 0.72)
        },
        base_color_texture: player_skin.clone(),
        emissive_texture: player_skin,
        alpha_mode: AlphaMode::Mask(0.5),
        perceptual_roughness: 0.95,
        metallic: 0.0,
        ..Default::default()
    });

    let head = spawn_player_part(
        commands,
        meshes,
        materials,
        &base_mat,
        player_head_meshes(texture_debug),
        Vec3::new(0.0, player_head_pivot_y(), 0.0),
        head_child_offset(),
        None,
    );
    let body = spawn_player_part(
        commands,
        meshes,
        materials,
        &base_mat,
        player_body_meshes(texture_debug),
        Vec3::new(0.0, player_body_pivot_y(), 0.0),
        torso_child_offset(),
        None,
    );
    let arm_left = spawn_player_part(
        commands,
        meshes,
        materials,
        &base_mat,
        player_left_arm_meshes(skin_model, texture_debug),
        Vec3::new(-player_arm_pivot_x(skin_model), player_arm_pivot_y(), 0.0),
        Vec3::new(
            player_arm_child_offset_x(skin_model, false),
            limb_child_offset().y,
            0.0,
        ),
        None,
    );
    let arm_right = spawn_player_part(
        commands,
        meshes,
        materials,
        &base_mat,
        player_right_arm_meshes(skin_model, texture_debug),
        Vec3::new(player_arm_pivot_x(skin_model), player_arm_pivot_y(), 0.0),
        Vec3::new(
            player_arm_child_offset_x(skin_model, true),
            limb_child_offset().y,
            0.0,
        ),
        None,
    );
    let leg_left = spawn_player_part(
        commands,
        meshes,
        materials,
        &base_mat,
        player_left_leg_meshes(texture_debug),
        Vec3::new(
            -player_leg_pivot_x(),
            player_leg_pivot_y_sneak(0.0),
            player_leg_pivot_z_sneak(0.0),
        ),
        limb_child_offset(),
        None,
    );
    let leg_right = spawn_player_part(
        commands,
        meshes,
        materials,
        &base_mat,
        player_right_leg_meshes(texture_debug),
        Vec3::new(
            player_leg_pivot_x(),
            player_leg_pivot_y_sneak(0.0),
            player_leg_pivot_z_sneak(0.0),
        ),
        limb_child_offset(),
        None,
    );

    (
        RemotePlayerModelParts {
            head,
            body,
            arm_left,
            arm_right,
            leg_left,
            leg_right,
        },
        vec![base_mat],
    )
}

pub(super) fn spawn_player_model_with_material(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    base_material: &Handle<StandardMaterial>,
    skin_model: PlayerSkinModel,
    texture_debug: &PlayerTextureDebugSettings,
    render_layer: Option<usize>,
) -> LocalPlayerModelParts {
    let head = spawn_player_part(
        commands,
        meshes,
        materials,
        base_material,
        player_head_meshes(texture_debug),
        Vec3::new(0.0, player_head_pivot_y(), 0.0),
        head_child_offset(),
        render_layer,
    );
    let body = spawn_player_part(
        commands,
        meshes,
        materials,
        base_material,
        player_body_meshes(texture_debug),
        Vec3::new(0.0, player_body_pivot_y(), 0.0),
        torso_child_offset(),
        render_layer,
    );
    let arm_left = spawn_player_part(
        commands,
        meshes,
        materials,
        base_material,
        player_left_arm_meshes(skin_model, texture_debug),
        Vec3::new(-player_arm_pivot_x(skin_model), player_arm_pivot_y(), 0.0),
        Vec3::new(
            player_arm_child_offset_x(skin_model, false),
            limb_child_offset().y,
            0.0,
        ),
        render_layer,
    );
    let arm_right = spawn_player_part(
        commands,
        meshes,
        materials,
        base_material,
        player_right_arm_meshes(skin_model, texture_debug),
        Vec3::new(player_arm_pivot_x(skin_model), player_arm_pivot_y(), 0.0),
        Vec3::new(
            player_arm_child_offset_x(skin_model, true),
            limb_child_offset().y,
            0.0,
        ),
        render_layer,
    );
    let leg_left = spawn_player_part(
        commands,
        meshes,
        materials,
        base_material,
        player_left_leg_meshes(texture_debug),
        Vec3::new(
            -player_leg_pivot_x(),
            player_leg_pivot_y_sneak(0.0),
            player_leg_pivot_z_sneak(0.0),
        ),
        limb_child_offset(),
        render_layer,
    );
    let leg_right = spawn_player_part(
        commands,
        meshes,
        materials,
        base_material,
        player_right_leg_meshes(texture_debug),
        Vec3::new(
            player_leg_pivot_x(),
            player_leg_pivot_y_sneak(0.0),
            player_leg_pivot_z_sneak(0.0),
        ),
        limb_child_offset(),
        render_layer,
    );

    LocalPlayerModelParts {
        head,
        body,
        arm_left,
        arm_right,
        leg_left,
        leg_right,
    }
}

pub(super) fn spawn_player_part(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
    base_material: &Handle<StandardMaterial>,
    part_meshes: Vec<Mesh>,
    translation: Vec3,
    child_offset: Vec3,
    render_layer: Option<usize>,
) -> Entity {
    let mut children = Vec::new();
    for mesh in part_meshes {
        let mut child = commands.spawn((
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(base_material.clone()),
            Transform::from_translation(child_offset),
            GlobalTransform::default(),
            Visibility::Visible,
            InheritedVisibility::default(),
            ViewVisibility::default(),
        ));
        if let Some(layer) = render_layer {
            child.insert(bevy::render::view::RenderLayers::layer(layer));
        }
        let child = child.id();
        children.push(child);
    }

    let mut pivot = commands.spawn((
        Transform::from_translation(translation),
        GlobalTransform::default(),
        Visibility::Visible,
        InheritedVisibility::default(),
        ViewVisibility::default(),
    ));
    if let Some(layer) = render_layer {
        pivot.insert(bevy::render::view::RenderLayers::layer(layer));
    }
    let pivot = pivot.id();

    for child in children {
        commands.entity(pivot).add_child(child);
    }
    pivot
}
