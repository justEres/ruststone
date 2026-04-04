use super::*;

pub fn first_person_viewmodel_system(
    mut commands: Commands,
    app_state: Res<AppState>,
    ui_state: Res<rs_utils::UiState>,
    player_status: Res<rs_utils::PlayerStatus>,
    render_debug: Res<RenderDebugSettings>,
    perspective: Res<CameraPerspectiveState>,
    inventory: Res<rs_utils::InventoryState>,
    mut item_textures: ResMut<ItemTextureCache>,
    item_sprite_mesh: Res<ItemSpriteMesh>,
    texture_debug: Res<PlayerTextureDebugSettings>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    camera: Query<Entity, With<PlayerCamera>>,
    local_player_skin: Query<
        (&LocalPlayerSkinMaterial, &LocalPlayerSkinModel),
        With<LocalPlayerModel>,
    >,
    existing: Query<(Entity, &FirstPersonViewModelParts), With<FirstPersonViewModel>>,
) {
    let held = inventory.hotbar_item(inventory.selected_hotbar_slot);
    let active = matches!(app_state.0, ApplicationState::Connected)
        && !ui_state.chat_open
        && !ui_state.paused
        && !ui_state.inventory_open
        && !player_status.dead
        && render_debug.render_held_items
        && render_debug.render_first_person_arms
        && held.is_some()
        && matches!(perspective.mode, CameraPerspectiveMode::FirstPerson);

    if !active {
        for (e, _) in &existing {
            commands.entity(e).despawn_recursive();
        }
        return;
    }

    let Ok(cam_entity) = camera.get_single() else {
        return;
    };

    let Ok((skin_mat, skin_model)) = local_player_skin.get_single() else {
        // Local model is also our "skin/material authority". Keep it present.
        return;
    };

    if let Some(stack) = held.as_ref() {
        item_textures.request_stack(&stack);
    }

    let base_pose_rotation = Quat::from_rotation_y(std::f32::consts::PI)
        * Quat::from_rotation_x(-1.835)
        * Quat::from_rotation_y(0.32)
        * Quat::from_rotation_z(-0.12);
    let hand_offset = Vec3::new(0.0, -(14.0 / 16.0), -(1.0 / 16.0));
    // Target hand position in viewmodel space; pivot is computed from this and the arm rotation.
    let hand_target = Vec3::new(0.75, -0.30, -0.75);
    let base_pose_translation = hand_target - (base_pose_rotation * hand_offset);

    // Recreate if missing or if the skin model changed (classic vs slim affects arm geometry).
    if let Ok((root, parts)) = existing.get_single() {
        if parts.skin_model != skin_model.0 {
            commands.entity(root).despawn_recursive();
        } else {
            // Update held item stack without rebuilding.
            if let Ok(mut item_entity) = commands.get_entity(parts.item) {
                match held.clone() {
                    Some(stack) => {
                        item_entity.insert((ItemSpriteStack(stack), Visibility::Visible));
                    }
                    None => {
                        item_entity.remove::<ItemSpriteStack>();
                        item_entity.insert(Visibility::Hidden);
                    }
                }
            }
            return;
        }
    }

    let root = commands
        .spawn((
            Name::new("FirstPersonViewModel"),
            FirstPersonViewModel,
            Transform::IDENTITY,
            GlobalTransform::default(),
            Visibility::Inherited,
            InheritedVisibility::default(),
            ViewVisibility::default(),
        ))
        .id();
    commands.entity(cam_entity).add_child(root);

    let arm_child_offset = Vec3::new(
        player_arm_child_offset_x(skin_model.0, true),
        first_person_arm_child_offset().y,
        0.0,
    );
    let arm_right = spawn_player_part(
        &mut commands,
        &mut meshes,
        &mut materials,
        &skin_mat.0,
        player_right_arm_meshes(skin_model.0, &texture_debug),
        base_pose_translation,
        arm_child_offset,
        None,
    );
    if let Ok(mut arm_cmd) = commands.get_entity(arm_right) {
        arm_cmd.insert(Transform {
            translation: base_pose_translation,
            rotation: base_pose_rotation,
            ..Default::default()
        });
    }
    commands.entity(root).add_child(arm_right);

    let hand_anchor = commands
        .spawn((
            Name::new("FirstPersonHandAnchor"),
            // Hand at the bottom of the arm (12px from the shoulder pivot).
            Transform::from_translation(hand_offset),
            GlobalTransform::default(),
            Visibility::Inherited,
            InheritedVisibility::default(),
            ViewVisibility::default(),
        ))
        .id();
    commands.entity(arm_right).add_child(hand_anchor);

    let item_placeholder = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        alpha_mode: AlphaMode::Mask(0.5),
        cull_mode: None,
        unlit: true,
        perceptual_roughness: 1.0,
        metallic: 0.0,
        ..Default::default()
    });
    let item = commands
        .spawn((
            Name::new("FirstPersonHeldItem"),
            Mesh3d(item_sprite_mesh.0.clone()),
            MeshMaterial3d(item_placeholder),
            Transform {
                translation: Vec3::new(0.05, 0.1, 0.38),
                rotation: Quat::from_rotation_y(std::f32::consts::PI)
                    * Quat::from_rotation_x(0.35)
                    * Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
                scale: Vec3::splat(0.72),
            },
            GlobalTransform::default(),
            if held.is_some() {
                Visibility::Visible
            } else {
                Visibility::Hidden
            },
            InheritedVisibility::default(),
            ViewVisibility::default(),
        ))
        .id();
    if let Some(stack) = held {
        commands.entity(item).insert(ItemSpriteStack(stack));
    }
    commands.entity(hand_anchor).add_child(item);

    commands.entity(root).insert(FirstPersonViewModelParts {
        arm_right,
        item,
        skin_model: skin_model.0,
    });
}

pub fn animate_first_person_viewmodel_system(
    time: Res<Time>,
    swing_state: Res<LocalArmSwing>,
    query: Query<&FirstPersonViewModelParts, With<FirstPersonViewModel>>,
    mut transforms: Query<&mut Transform>,
) {
    let Ok(parts) = query.get_single() else {
        return;
    };

    let dt = time.delta_secs().clamp(0.0, 0.05);
    let p = swing_state.progress.clamp(0.0, 1.0);
    let swing = if p < 1.0 {
        // Very rough approximation of vanilla first-person swing.
        let s = (p * std::f32::consts::PI).sin();
        let s2 = (p * std::f32::consts::PI).sin().powf(2.0);
        (s, s2)
    } else {
        (0.0, 0.0)
    };

    let (s, s2) = swing;
    let base_r = Quat::from_rotation_y(std::f32::consts::PI)
        * Quat::from_rotation_x(-1.835)
        * Quat::from_rotation_y(0.32)
        * Quat::from_rotation_z(-0.12);
    let hand_offset = Vec3::new(0.0, -(14.0 / 16.0), -(1.0 / 16.0));
    let hand_target = Vec3::new(0.75, -0.30, -0.75);
    let base_t = hand_target - (base_r * hand_offset);

    // Small idle damping so it doesn't snap if the transform was recreated.
    let alpha = 1.0 - (-18.0 * dt).exp();
    if let Ok(mut arm_t) = transforms.get_mut(parts.arm_right) {
        let target_t = base_t;
        let target_r = base_r
            * Quat::from_rotation_x(1.25 * s)
            * Quat::from_rotation_y(-0.55 * s2)
            * Quat::from_rotation_z(0.25 * s2);
        let current_t = arm_t.translation;
        arm_t.translation = current_t + (target_t - current_t) * alpha;
        arm_t.rotation = arm_t.rotation.slerp(target_r, alpha);
    }
}

pub fn suppress_first_person_viewmodel_near_geometry_system(
    collision_map: Res<WorldCollisionMap>,
    camera_query: Query<&GlobalTransform, With<PlayerCamera>>,
    mut viewmodel_query: Query<&mut Visibility, With<FirstPersonViewModel>>,
) {
    let Ok(camera_transform) = camera_query.get_single() else {
        return;
    };
    let Ok(mut visibility) = viewmodel_query.get_single_mut() else {
        return;
    };

    let camera_pos = camera_transform.translation();
    let camera_rot = camera_transform.compute_transform().rotation;
    // Probe the arm/item volume in camera-local space.
    let probes = [
        Vec3::new(0.05, -0.05, 0.12),
        Vec3::new(0.30, -0.18, -0.38),
        Vec3::new(0.55, -0.24, -0.58),
        Vec3::new(0.78, -0.34, -0.82),
    ];

    let colliding = probes.into_iter().any(|probe| {
        let world = camera_pos + camera_rot * probe;
        let cell = world.floor().as_ivec3();
        is_solid(collision_map.block_at(cell.x, cell.y, cell.z))
    });

    *visibility = if colliding {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };
}
