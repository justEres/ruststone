use super::*;
use crate::components::{
    DROPPED_ITEM_COLLISION_HEIGHT_OFFSET, DROPPED_ITEM_COLLISION_RADIUS,
    DROPPED_ITEM_COLLECT_DURATION, DROPPED_ITEM_DRAG_AIR, DROPPED_ITEM_DRAG_GROUND,
    DROPPED_ITEM_EXTRAPOLATE_MAX, DROPPED_ITEM_FALLBACK_COLLECT_HEIGHT,
    DROPPED_ITEM_GRAVITY, DROPPED_ITEM_RESTITUTION,
};

pub(crate) fn update_motion_velocity(
    smoothing: &mut RemoteMotionSmoothing,
    previous: Vec3,
    next: Vec3,
    now_secs: f64,
) {
    let dt = (now_secs - smoothing.last_server_update_secs).max(1.0 / 120.0) as f32;
    smoothing.estimated_velocity = (next - previous) / dt;
    smoothing.target_translation = next;
    smoothing.last_server_update_secs = now_secs;
}

pub(crate) fn update_item_motion_velocity(
    motion: &mut RemoteDroppedItemMotion,
    previous: Vec3,
    next: Vec3,
    now_secs: f64,
) {
    let dt = (now_secs - motion.last_server_update_secs).max(1.0 / 120.0) as f32;
    motion.estimated_velocity = (next - previous) / dt;
    motion.authoritative_translation = next;
    motion.last_server_update_secs = now_secs;
    motion.ground_contact = false;
}

fn sample_solid_block(world: &WorldCollisionMap, point: Vec3) -> Option<IVec3> {
    let radius = DROPPED_ITEM_COLLISION_RADIUS;
    let probes = [
        point,
        point + Vec3::new(radius, 0.0, 0.0),
        point + Vec3::new(-radius, 0.0, 0.0),
        point + Vec3::new(0.0, 0.0, radius),
        point + Vec3::new(0.0, 0.0, -radius),
    ];
    probes.into_iter().find_map(|probe| {
        let cell = probe.floor().as_ivec3();
        is_solid(world.block_at(cell.x, cell.y, cell.z)).then_some(cell)
    })
}

fn clamp_item_translation(
    world: &WorldCollisionMap,
    current: Vec3,
    candidate: Vec3,
    velocity: &mut Vec3,
) -> (Vec3, bool) {
    let mut next = candidate;
    let mut grounded = false;

    if let Some(cell) = sample_solid_block(world, next) {
        let top = cell.y as f32 + 1.0 + DROPPED_ITEM_COLLISION_HEIGHT_OFFSET;
        if current.y >= top - 0.35 || velocity.y <= 0.0 {
            next.y = top;
            if velocity.y < 0.0 {
                velocity.y = -velocity.y * DROPPED_ITEM_RESTITUTION;
                if velocity.y.abs() < 0.02 {
                    velocity.y = 0.0;
                }
            }
            grounded = true;
        } else {
            velocity.x = 0.0;
            velocity.z = 0.0;
            next.x = current.x;
            next.z = current.z;
        }
    }

    let below = Vec3::new(
        next.x,
        next.y - DROPPED_ITEM_COLLISION_HEIGHT_OFFSET - 0.02,
        next.z,
    );
    if let Some(cell) = sample_solid_block(world, below) {
        let top = cell.y as f32 + 1.0 + DROPPED_ITEM_COLLISION_HEIGHT_OFFSET;
        if next.y <= top + 0.08 && velocity.y <= 0.0 {
            next.y = top;
            grounded = true;
            velocity.y = 0.0;
        }
    }

    (next, grounded)
}

fn advance_item_motion(
    world: &WorldCollisionMap,
    motion: &RemoteDroppedItemMotion,
    now_secs: f64,
) -> (Vec3, Vec3, bool) {
    let mut pos = motion.authoritative_translation;
    let mut vel = motion.estimated_velocity;
    let age = ((now_secs - motion.last_server_update_secs) as f32)
        .clamp(0.0, DROPPED_ITEM_EXTRAPOLATE_MAX);
    if age <= 0.0 {
        return (motion.render_translation, vel, motion.ground_contact);
    }

    let steps = (age / (1.0 / 60.0)).ceil().max(1.0) as usize;
    let dt = age / steps as f32;
    let mut grounded = motion.ground_contact;

    for _ in 0..steps {
        vel.y += DROPPED_ITEM_GRAVITY * dt * 20.0;
        let candidate = pos + vel * dt * 20.0;
        let (next_pos, hit_ground) = clamp_item_translation(world, pos, candidate, &mut vel);
        pos = next_pos;
        grounded = hit_ground;
        let drag = if grounded {
            DROPPED_ITEM_DRAG_GROUND
        } else {
            DROPPED_ITEM_DRAG_AIR
        };
        vel.x *= drag;
        vel.y *= DROPPED_ITEM_DRAG_AIR;
        vel.z *= drag;
    }

    (pos, vel, grounded)
}

pub fn smooth_remote_entity_motion(
    time: Res<Time>,
    mut query: Query<
        (
            &RemoteEntity,
            &RemoteEntityLook,
            &mut Transform,
            &RemoteMotionSmoothing,
        ),
        With<RemoteEntity>,
    >,
) {
    let dt = time.delta_secs().max(1e-4);
    let now_secs = time.elapsed_secs_f64();
    for (remote, look, mut transform, smoothing) in &mut query {
        // Extrapolate a little bit from the last known velocity to hide packet spacing.
        let extrapolate = ((now_secs - smoothing.last_server_update_secs) as f32).clamp(0.0, 0.1);
        let desired_pos = smoothing.target_translation + smoothing.estimated_velocity * extrapolate;
        let delta = desired_pos - transform.translation;
        if delta.length_squared() > 64.0 {
            transform.translation = desired_pos;
        } else {
            let pos_alpha = 1.0 - (-18.0 * dt).exp();
            transform.translation += delta * pos_alpha;
        }

        if remote.kind == NetEntityKind::Item {
            // Item sprites use a dedicated billboard/spin system; don't fight it here.
            continue;
        }

        let desired_rot = entity_root_rotation(remote.kind, look.yaw);
        let rot_alpha = 1.0 - (-22.0 * dt).exp();
        transform.rotation = transform.rotation.slerp(desired_rot, rot_alpha);
    }
}

pub fn smooth_remote_item_entities(
    mut commands: Commands,
    time: Res<Time>,
    mut registry: ResMut<RemoteEntityRegistry>,
    collision_map: Res<WorldCollisionMap>,
    transforms: Query<&GlobalTransform>,
    mut query: Query<
        (
            Entity,
            &RemoteEntity,
            &mut Transform,
            &mut RemoteDroppedItemMotion,
            Option<&mut RemoteDroppedItemCollect>,
            Option<&RemoteItemStackState>,
        ),
        With<RemoteItemSprite>,
    >,
) {
    let dt = time.delta_secs().max(1e-4);
    let now_secs = time.elapsed_secs_f64();
    for (entity, remote, mut transform, mut motion, collect, stack) in &mut query {
        let Some(stack) = stack else {
            motion.render_translation = transform.translation;
            continue;
        };

        if let Some(mut collect) = collect {
            collect.progress_secs += dt;
            let target = collect
                .collector_server_id
                .and_then(|collector_id| registry.by_server_id.get(&collector_id).copied())
                .and_then(|collector| transforms.get(collector).ok())
                .map(|gt| gt.translation() + Vec3::Y * 0.9)
                .unwrap_or(transform.translation + Vec3::Y * DROPPED_ITEM_FALLBACK_COLLECT_HEIGHT);
            let alpha = (collect.progress_secs / DROPPED_ITEM_COLLECT_DURATION).clamp(0.0, 1.0);
            transform.translation = transform.translation.lerp(target, alpha);
            transform.scale = Vec3::splat(DROPPED_ITEM_RENDER_SCALE * (1.0 - alpha * 0.75));
            motion.render_translation = transform.translation;
            if alpha >= 1.0 {
                debug!(
                    entity_id = remote.server_id,
                    item_id = stack.0.item_id,
                    damage = stack.0.damage,
                    "despawning collected dropped item after fly-to animation"
                );
                registry.by_server_id.remove(&remote.server_id);
                registry.pending_labels.remove(&remote.server_id);
                commands.entity(entity).despawn_recursive();
            }
            continue;
        }

        let (predicted_pos, predicted_vel, grounded) =
            advance_item_motion(&collision_map, &motion, now_secs);
        let delta = predicted_pos - motion.render_translation;
        if delta.length_squared() > 9.0 {
            motion.render_translation = predicted_pos;
        } else {
            let alpha = 1.0 - (-18.0 * dt).exp();
            motion.render_translation += delta * alpha;
        }
        motion.estimated_velocity = predicted_vel;
        motion.ground_contact = grounded;
        transform.translation = motion.render_translation;
        transform.scale = Vec3::splat(DROPPED_ITEM_RENDER_SCALE);
    }
}

pub fn draw_remote_entity_names(
    mut contexts: EguiContexts,
    camera_query: Query<(&Camera, &GlobalTransform), With<PlayerCamera>>,
    names_query: Query<
        (
            &GlobalTransform,
            &RemoteEntityName,
            &RemoteVisual,
            &RemoteEntity,
            &ViewVisibility,
        ),
        With<RemoteEntity>,
    >,
    collision_map: Res<WorldCollisionMap>,
    ui_state: Res<UiState>,
) {
    const NON_PLAYER_NAME_MAX_DISTANCE: f32 = 5.0;
    const PLAYER_NAME_ALPHA: u8 = 210;
    const NON_PLAYER_NAME_ALPHA: u8 = 120;

    if ui_state.ui_hidden {
        return;
    }

    let Ok((camera, camera_transform)) = camera_query.get_single() else {
        return;
    };
    let ctx = contexts.ctx_mut().unwrap();
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("remote_player_names"),
    ));

    let cam_pos = camera_transform.translation();
    for (transform, name, visual, remote, view_visibility) in &names_query {
        // Modeled entities already carry strong visual identity and their labels
        // add unnecessary clutter.
        if entity_kind_uses_model(remote.kind) && remote.kind != NetEntityKind::Player {
            continue;
        }

        let world_pos = transform.translation() + Vec3::Y * visual.name_y_offset;
        let through_walls = remote.kind == NetEntityKind::Player;
        if !through_walls && !view_visibility.get() {
            continue;
        }
        if !through_walls
            && transform.translation().distance(cam_pos) > NON_PLAYER_NAME_MAX_DISTANCE
        {
            continue;
        }
        if !through_walls && line_of_sight_blocked(&collision_map, cam_pos, world_pos) {
            continue;
        }
        let Ok(screen_pos) = camera.world_to_viewport(camera_transform, world_pos) else {
            continue;
        };
        let pos = egui::pos2(screen_pos.x, screen_pos.y);
        painter.text(
            pos,
            egui::Align2::CENTER_BOTTOM,
            &name.0,
            egui::TextStyle::Body.resolve(&ctx.style()),
            if through_walls {
                egui::Color32::from_white_alpha(PLAYER_NAME_ALPHA)
            } else {
                egui::Color32::from_white_alpha(NON_PLAYER_NAME_ALPHA)
            },
        );
    }
}

fn entity_kind_uses_model(kind: NetEntityKind) -> bool {
    match kind {
        NetEntityKind::Player => true,
        NetEntityKind::Mob(mob) => mob_uses_entity_model(mob),
        _ => false,
    }
}

pub fn apply_item_sprite_textures_system(
    mut cache: ResMut<ItemTextureCache>,
    mut query: Query<(&ItemSpriteStack, &mut MeshMaterial3d<StandardMaterial>)>,
) {
    for (stack, mut material) in &mut query {
        cache.request_stack(&stack.0);
        if let Some(handle) = cache.material_for_stack(&stack.0) {
            if material.0 != handle {
                material.0 = handle;
            }
            crate::item_textures::log_item_texture_resolution(&mut cache, &stack.0);
        }
    }
}

pub fn apply_entity_model_textures_system(
    mut cache: ResMut<EntityTextureCache>,
    mut query: Query<(&EntityTexturePath, &mut MeshMaterial3d<StandardMaterial>)>,
) {
    for (path, mut material) in &mut query {
        cache.request(path.0);
        if let Some(handle) = cache.material(path.0) {
            if material.0 != handle {
                material.0 = handle;
            }
        }
    }
}

pub fn update_remote_sheep_wool_system(
    mut cache: ResMut<EntityTextureCache>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    query: Query<(&RemoteSheepWoolLayer, &RemoteSheepAppearance)>,
    mut visibility_query: Query<&mut Visibility>,
) {
    cache.request(SHEEP_WOOL_TEXTURE_PATH);
    let wool_texture = cache.texture(SHEEP_WOOL_TEXTURE_PATH);

    for (wool, appearance) in &query {
        if let Some(material) = materials.get_mut(&wool.material) {
            if let Some(texture) = wool_texture.clone() {
                if material.base_color_texture.as_ref() != Some(&texture) {
                    material.base_color_texture = Some(texture);
                }
            }
            let [r, g, b] = sheep_fleece_rgb(appearance.fleece_color);
            material.base_color = Color::srgb(r, g, b);
            material.alpha_mode = AlphaMode::Mask(0.5);
            material.unlit = true;
        }

        let target_visibility = if appearance.sheared {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        for mesh_entity in wool.mesh_entities {
            if let Ok(mut visibility) = visibility_query.get_mut(mesh_entity) {
                *visibility = target_visibility;
            }
        }
    }
}

pub fn apply_held_item_visibility_system(
    settings: Res<RenderDebugSettings>,
    held: Query<&RemoteHeldItem>,
    mut vis: Query<&mut Visibility>,
) {
    if !settings.is_changed() {
        return;
    }
    let target = if settings.render_held_items {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for held in &held {
        if let Ok(mut v) = vis.get_mut(held.0) {
            *v = target;
        }
    }
}

pub fn billboard_item_sprites(
    time: Res<Time>,
    camera: Query<&GlobalTransform, With<PlayerCamera>>,
    mut items: Query<(&GlobalTransform, &mut Transform, &mut ItemSpin), With<RemoteItemSprite>>,
) {
    let Ok(cam) = camera.get_single() else {
        return;
    };
    let cam_pos = cam.translation();
    let dt = time.delta_secs().clamp(0.0, 0.05);

    for (global, mut local, mut spin) in &mut items {
        // Use the global translation so culling offsets don't matter.
        let pos = global.translation();
        let to_cam = cam_pos - pos;
        let yaw = to_cam.x.atan2(to_cam.z);
        spin.0 = (spin.0 + dt * 1.2).rem_euclid(std::f32::consts::TAU);
        local.rotation = Quat::from_rotation_y(yaw + spin.0);
    }
}

fn line_of_sight_blocked(world: &WorldCollisionMap, from: Vec3, to: Vec3) -> bool {
    let delta = to - from;
    let len = delta.length();
    if len <= 0.05 {
        return false;
    }
    let dir = delta / len;
    let step = 0.1f32;
    let mut t = 0.05f32;
    while t < len - 0.05 {
        let p = from + dir * t;
        let cell = p.floor().as_ivec3();
        if is_solid(world.block_at(cell.x, cell.y, cell.z)) {
            return true;
        }
        t += step;
    }
    false
}
