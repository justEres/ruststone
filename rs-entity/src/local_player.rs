use super::*;

pub fn spawn_local_player_model_system(
    mut commands: Commands,
    app_state: Res<AppState>,
    render_debug: Res<RenderDebugSettings>,
    connect_ui: Res<ConnectUiState>,
    registry: Res<RemoteEntityRegistry>,
    mut skin_downloader: ResMut<RemoteSkinDownloader>,
    mut entity_textures: ResMut<EntityTextureCache>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    texture_debug: Res<PlayerTextureDebugSettings>,
    player_query: Query<Entity, With<Player>>,
    existing: Query<Entity, With<LocalPlayerModel>>,
) {
    let Ok(player_entity) = player_query.get_single() else {
        return;
    };

    let connected = matches!(app_state.0, ApplicationState::Connected);
    if !connected || (!render_debug.render_self_model && !render_debug.render_first_person_arms) {
        for e in &existing {
            commands.entity(e).despawn_recursive();
        }
        return;
    }

    if !existing.is_empty() {
        return;
    }

    // Resolve skin from PlayerInfo (online) or fall back to built-in steve texture from the pack.
    let mut skin_handle: Option<Handle<Image>> = None;
    let mut skin_model = PlayerSkinModel::Classic;

    if connect_ui.auth_mode == rs_utils::AuthMode::Authenticated
        && connect_ui.selected_auth_account < connect_ui.auth_accounts.len()
    {
        if let Ok(uuid) = connect_ui.auth_accounts[connect_ui.selected_auth_account]
            .uuid
            .parse::<rs_protocol::protocol::UUID>()
        {
            skin_model = registry
                .player_skin_model_by_uuid
                .get(&uuid)
                .copied()
                .unwrap_or(PlayerSkinModel::Classic);
            if let Some(url) = registry.player_skin_url_by_uuid.get(&uuid) {
                skin_downloader.request(url.clone());
                skin_handle = skin_downloader.skin_handle(url);
            }
        }
    }

    if skin_handle.is_none() {
        const STEVE: &str = "entity/steve.png";
        entity_textures.request(STEVE);
        skin_handle = entity_textures.texture(STEVE);
    }

    let base_mat = materials.add(StandardMaterial {
        base_color: if skin_handle.is_some() {
            Color::WHITE
        } else {
            Color::srgb(0.85, 0.78, 0.72)
        },
        base_color_texture: skin_handle.clone(),
        emissive_texture: skin_handle,
        emissive: player_shadow_emissive_strength(render_debug.player_shadow_opacity),
        alpha_mode: AlphaMode::Mask(0.5),
        perceptual_roughness: 0.95,
        metallic: 0.0,
        ..Default::default()
    });

    let model_root = commands
        .spawn((
            Name::new("LocalPlayerModel"),
            LocalPlayerModel,
            RenderLayers::layer(LOCAL_PLAYER_RENDER_LAYER),
            // Match the remote player model facing (player root doesn't include the +PI).
            Transform::from_rotation(Quat::from_rotation_y(std::f32::consts::PI)),
            GlobalTransform::default(),
            Visibility::Inherited,
            InheritedVisibility::default(),
            ViewVisibility::default(),
            LocalPlayerSkinMaterial(base_mat.clone()),
            LocalPlayerSkinModel(skin_model),
        ))
        .id();
    commands.entity(player_entity).add_child(model_root);

    let parts = spawn_player_model_with_material(
        &mut commands,
        &mut meshes,
        &mut materials,
        &base_mat,
        skin_model,
        &texture_debug,
        Some(LOCAL_PLAYER_RENDER_LAYER),
    );
    commands.entity(model_root).add_child(parts.head);
    commands.entity(model_root).add_child(parts.body);
    commands.entity(model_root).add_child(parts.arm_left);
    commands.entity(model_root).add_child(parts.arm_right);
    commands.entity(model_root).add_child(parts.leg_left);
    commands.entity(model_root).add_child(parts.leg_right);

    commands.entity(model_root).insert((
        parts,
        HumanoidRigParts {
            kind: HumanoidRigKind::Player,
            model_root,
            head: parts.head,
            body: parts.body,
            arm_right: parts.arm_right,
            arm_left: parts.arm_left,
            leg_right: parts.leg_right,
            leg_left: parts.leg_left,
            render_layer: Some(LOCAL_PLAYER_RENDER_LAYER),
        },
        HumanoidArmorState::default(),
        HumanoidArmorLayerEntities::default(),
        LocalPlayerAnimation {
            walk_phase: 0.0,
            swing_progress: 1.0,
            hurt_progress: 1.0,
        },
    ));
}

pub fn apply_local_player_model_visibility_system(
    render_debug: Res<RenderDebugSettings>,
    perspective: Res<CameraPerspectiveState>,
    freecam: Res<FreecamState>,
    children_query: Query<&Children>,
    mut vis_query: Query<&mut Visibility>,
    mut camera_layers_query: Query<&mut RenderLayers, With<PlayerCamera>>,
    model_query: Query<Entity, With<LocalPlayerModel>>,
) {
    let Ok(model_root) = model_query.get_single() else {
        return;
    };

    let should_show = render_debug.render_self_model
        && (freecam.active || !matches!(perspective.mode, CameraPerspectiveMode::FirstPerson));
    let target = if should_show {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };

    // Force visibility for the whole subtree (pivots + meshes). This avoids cases where some
    // descendants have `Visibility::Visible` and still render when we expect them not to.
    let mut stack = vec![model_root];
    while let Some(e) = stack.pop() {
        if let Ok(mut v) = vis_query.get_mut(e) {
            *v = target;
        }
        if let Ok(children) = children_query.get(e) {
            for child in children.iter() {
                stack.push(child);
            }
        }
    }

    let should_render_local_model_in_camera = render_debug.render_self_model
        && (freecam.active || !matches!(perspective.mode, CameraPerspectiveMode::FirstPerson));
    let mut camera_layers = RenderLayers::layer(MAIN_RENDER_LAYER)
        .with(CHUNK_OPAQUE_RENDER_LAYER)
        .with(CHUNK_CUTOUT_RENDER_LAYER)
        .with(CHUNK_TRANSPARENT_RENDER_LAYER);
    if should_render_local_model_in_camera {
        camera_layers = camera_layers.with(LOCAL_PLAYER_RENDER_LAYER);
    }
    for mut layers in &mut camera_layers_query {
        *layers = camera_layers.clone();
    }
}

pub fn update_local_player_skin_system(
    app_state: Res<AppState>,
    connect_ui: Res<ConnectUiState>,
    registry: Res<RemoteEntityRegistry>,
    render_debug: Res<RenderDebugSettings>,
    mut downloader: ResMut<RemoteSkinDownloader>,
    mut entity_textures: ResMut<EntityTextureCache>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    query: Query<&LocalPlayerSkinMaterial, With<LocalPlayerModel>>,
) {
    if !matches!(app_state.0, ApplicationState::Connected) {
        return;
    }
    let Ok(local_mat) = query.get_single() else {
        return;
    };
    let Some(material) = materials.get_mut(&local_mat.0) else {
        return;
    };

    // Prefer online skin; fall back to steve from the pack until it's available.
    let mut desired: Option<Handle<Image>> = None;
    if connect_ui.auth_mode == rs_utils::AuthMode::Authenticated
        && connect_ui.selected_auth_account < connect_ui.auth_accounts.len()
    {
        if let Ok(uuid) = connect_ui.auth_accounts[connect_ui.selected_auth_account]
            .uuid
            .parse::<rs_protocol::protocol::UUID>()
        {
            if let Some(url) = registry.player_skin_url_by_uuid.get(&uuid)
                && {
                    downloader.request(url.clone());
                    true
                }
                && let Some(tex) = downloader.skin_handle(url)
            {
                desired = Some(tex);
            }
        }
    }

    // Fall back to steve from the pack when available.
    const STEVE: &str = "entity/steve.png";
    entity_textures.request(STEVE);
    if desired.is_none() {
        desired = entity_textures.texture(STEVE);
    }

    let Some(desired) = desired else {
        return;
    };
    material.base_color_texture = Some(desired.clone());
    material.emissive_texture = Some(desired);
    material.alpha_mode = AlphaMode::Mask(0.5);
    material.unlit = false;
    material.base_color = Color::WHITE;
    material.emissive = player_shadow_emissive_strength(render_debug.player_shadow_opacity);
}

pub fn apply_player_shadow_opacity_material_system(
    render_debug: Res<RenderDebugSettings>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    local_player: Query<&LocalPlayerSkinMaterial, With<LocalPlayerModel>>,
    remote_players: Query<&RemotePlayerSkinMaterials, With<RemotePlayer>>,
) {
    if !render_debug.is_changed() {
        return;
    }
    let emissive = player_shadow_emissive_strength(render_debug.player_shadow_opacity);

    if let Ok(local_skin) = local_player.get_single()
        && let Some(material) = materials.get_mut(&local_skin.0)
    {
        material.emissive = emissive;
    }
    for mats in &remote_players {
        for mat in &mats.0 {
            if let Some(material) = materials.get_mut(mat) {
                material.emissive = emissive;
            }
        }
    }
}

pub fn sync_local_player_skin_model_system(
    app_state: Res<AppState>,
    connect_ui: Res<ConnectUiState>,
    registry: Res<RemoteEntityRegistry>,
    texture_debug: Res<PlayerTextureDebugSettings>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut query: Query<(
        &LocalPlayerModelParts,
        &LocalPlayerSkinMaterial,
        &mut LocalPlayerSkinModel,
    )>,
    children_query: Query<&Children>,
) {
    if !matches!(app_state.0, ApplicationState::Connected) {
        return;
    }

    let Ok((parts, skin_mat, mut skin_model)) = query.get_single_mut() else {
        return;
    };

    let mut desired = PlayerSkinModel::Classic;
    if connect_ui.auth_mode == rs_utils::AuthMode::Authenticated
        && connect_ui.selected_auth_account < connect_ui.auth_accounts.len()
        && let Ok(uuid) = connect_ui.auth_accounts[connect_ui.selected_auth_account]
            .uuid
            .parse::<rs_protocol::protocol::UUID>()
    {
        desired = registry
            .player_skin_model_by_uuid
            .get(&uuid)
            .copied()
            .unwrap_or(PlayerSkinModel::Classic);
    }

    if skin_model.0 == desired {
        return;
    }

    skin_model.0 = desired;

    // Only arms differ between classic and slim models.
    rebuild_part_children(
        &mut commands,
        &mut meshes,
        &children_query,
        parts.arm_left,
        &skin_mat.0,
        player_left_arm_meshes(desired, &texture_debug),
        Vec3::new(
            player_arm_child_offset_x(desired, false),
            limb_child_offset().y,
            0.0,
        ),
    );
    rebuild_part_children(
        &mut commands,
        &mut meshes,
        &children_query,
        parts.arm_right,
        &skin_mat.0,
        player_right_arm_meshes(desired, &texture_debug),
        Vec3::new(
            player_arm_child_offset_x(desired, true),
            limb_child_offset().y,
            0.0,
        ),
    );
}

pub fn animate_local_player_model_system(
    time: Res<Time>,
    input: Res<rs_sim::CurrentInput>,
    sim_state: Res<rs_sim::SimState>,
    swing_state: Res<rs_sim::LocalArmSwing>,
    render_debug: Res<RenderDebugSettings>,
    mut roots: Query<
        (
            &LocalPlayerModelParts,
            &LocalPlayerSkinModel,
            &mut LocalPlayerAnimation,
        ),
        With<LocalPlayerModel>,
    >,
    mut part_transforms: Query<&mut Transform, Without<LocalPlayerModel>>,
    player_query: Query<&LookAngles, With<Player>>,
) {
    if !render_debug.render_self_model {
        return;
    }

    let Ok(look) = player_query.get_single() else {
        return;
    };
    let Ok((parts, skin_model, mut anim)) = roots.get_single_mut() else {
        return;
    };

    let dt = time.delta_secs().max(1e-4);
    let vel = sim_state.current.vel;
    let speed = (Vec2::new(vel.x, vel.z).length() * 20.0).min(8.0);
    let stride = (speed / 4.0).clamp(0.0, 1.0);
    anim.walk_phase += speed * dt * 2.5;

    let swing = anim.walk_phase.sin() * 0.7 * stride;
    let sneak_amount = if input.0.sneak { 1.0 } else { 0.0 };
    let arm_x = player_arm_pivot_x(skin_model.0);
    let leg_x = player_leg_pivot_x();
    let leg_y = player_leg_pivot_y_sneak(sneak_amount);
    let leg_z = player_leg_pivot_z_sneak(sneak_amount);
    let arm_attack = if swing_state.progress < 1.0 {
        (swing_state.progress * std::f32::consts::PI).sin() * 1.2
    } else {
        0.0
    };

    if let Ok(mut t) = part_transforms.get_mut(parts.head) {
        t.translation = Vec3::new(0.0, player_head_pivot_y_sneak(sneak_amount), 0.0);
        t.rotation = Quat::from_rotation_x(-look.pitch - 0.2 * sneak_amount);
    }
    if let Ok(mut t) = part_transforms.get_mut(parts.body) {
        t.translation = Vec3::new(0.0, player_body_pivot_y(), 0.0);
        t.rotation = Quat::from_rotation_x(0.5 * sneak_amount);
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
