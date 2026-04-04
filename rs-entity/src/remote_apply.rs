use super::*;
use crate::motion::{update_item_motion_velocity, update_motion_velocity};

pub fn apply_remote_entity_events(
    mut commands: Commands,
    time: Res<Time>,
    mut queue: ResMut<RemoteEntityEventQueue>,
    mut registry: ResMut<RemoteEntityRegistry>,
    mut skin_downloader: ResMut<RemoteSkinDownloader>,
    mut item_textures: ResMut<ItemTextureCache>,
    mut entity_textures: ResMut<EntityTextureCache>,
    item_sprite_mesh: Res<ItemSpriteMesh>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut params: RemoteEntityApplyParams,
    texture_debug: Res<PlayerTextureDebugSettings>,
) {
    let now_secs = time.elapsed_secs_f64();
    for event in queue.drain() {
        match event {
            NetEntityMessage::LocalPlayerId { entity_id } => {
                registry.local_entity_id = Some(entity_id);
                registry.pending_labels.remove(&entity_id);
                if let Some(entity) = registry.by_server_id.remove(&entity_id) {
                    commands.entity(entity).despawn_recursive();
                    registry
                        .player_entity_by_uuid
                        .retain(|_, id| *id != entity_id);
                }
            }
            NetEntityMessage::PlayerInfoAdd {
                uuid,
                name,
                skin_url,
                skin_model,
            } => {
                info!(
                    "ENTITY PlayerInfoAdd name={} uuid={:?} skin_url={:?} skin_model={:?}",
                    name, uuid, skin_url, skin_model
                );
                registry
                    .player_name_by_uuid
                    .insert(uuid.clone(), name.clone());
                registry
                    .player_skin_model_by_uuid
                    .insert(uuid.clone(), skin_model);
                if let Some(url) = skin_url {
                    skin_downloader.request(url.clone());
                    registry.player_skin_url_by_uuid.insert(uuid.clone(), url);
                } else {
                    warn!("ENTITY no skin url in PlayerInfoAdd for uuid={:?}", uuid);
                }
                if let Some(server_id) = registry.player_entity_by_uuid.get(&uuid).copied()
                    && let Some(entity) = registry.by_server_id.get(&server_id).copied()
                    && let Ok(mut entity_name) = params.name_query.get_mut(entity)
                {
                    entity_name.0 = name;
                }
            }
            NetEntityMessage::PlayerInfoRemove { uuid } => {
                registry.player_name_by_uuid.remove(&uuid);
                registry.player_skin_model_by_uuid.remove(&uuid);
            }
            NetEntityMessage::Spawn {
                entity_id,
                uuid,
                kind,
                pos,
                yaw,
                pitch,
                on_ground,
            } => {
                if registry.local_entity_id == Some(entity_id) {
                    continue;
                }

                if let Some(existing) = registry.by_server_id.remove(&entity_id) {
                    commands.entity(existing).despawn_recursive();
                    registry
                        .player_entity_by_uuid
                        .retain(|_, id| *id != entity_id);
                }

                let spec = visual_spec(kind);
                let visual = visual_for_kind(kind);
                let player_skin = if kind == NetEntityKind::Player {
                    let url = uuid
                        .as_ref()
                        .and_then(|id| registry.player_skin_url_by_uuid.get(id));
                    if let Some(url) = url {
                        skin_downloader.request(url.clone());
                        skin_downloader.skin_handle(url)
                    } else {
                        if let Some(id) = uuid.as_ref() {
                            warn!("ENTITY player spawn without known skin url uuid={:?}", id);
                        }
                        None
                    }
                } else {
                    None
                };
                let player_skin_model = uuid
                    .as_ref()
                    .and_then(|id| registry.player_skin_model_by_uuid.get(id))
                    .copied()
                    .unwrap_or(PlayerSkinModel::Classic);
                let display_name = if kind == NetEntityKind::Player {
                    uuid.as_ref()
                        .and_then(|id| registry.player_name_by_uuid.get(id))
                        .cloned()
                        .unwrap_or_else(|| format!("Player {}", entity_id))
                } else {
                    registry
                        .pending_labels
                        .remove(&entity_id)
                        .unwrap_or_else(|| kind_label(kind).to_string())
                };

                let biped_mob = match kind {
                    NetEntityKind::Mob(m) if mob_uses_biped_model(m) => Some(m),
                    _ => None,
                };
                let quadruped_mob = match kind {
                    NetEntityKind::Mob(m) if mob_uses_quadruped_model(m) => Some(m),
                    _ => None,
                };
                let uses_model_mesh = biped_mob.is_some() || quadruped_mob.is_some();
                let root_translation = entity_root_translation(kind, pos, visual.y_offset);

                let spawn_cmd = commands.spawn((
                    Name::new(format!("RemoteEntity[{entity_id}]")),
                    Transform {
                        translation: root_translation,
                        rotation: entity_root_rotation(kind, yaw),
                        scale: if uses_model_mesh {
                            match kind {
                                NetEntityKind::Mob(mob) => mob_model_scale(mob),
                                _ => Vec3::ONE,
                            }
                        } else {
                            spec.scale
                        },
                    },
                    GlobalTransform::default(),
                    Visibility::Visible,
                    InheritedVisibility::default(),
                    ViewVisibility::default(),
                    RemoteEntity {
                        server_id: entity_id,
                        kind,
                        on_ground: on_ground.unwrap_or(false),
                    },
                    RemoteEntityLook {
                        yaw,
                        pitch,
                        head_yaw: yaw,
                    },
                    RemoteEntityName(display_name),
                    visual,
                    RemotePoseState::default(),
                ));
                let root = spawn_cmd.id();

                if kind == NetEntityKind::Player {
                    commands
                        .entity(root)
                        .insert(RemoteMotionSmoothing::new(root_translation, now_secs));
                    let (parts, material_handles) = spawn_remote_player_model(
                        &mut commands,
                        &mut meshes,
                        &mut materials,
                        player_skin,
                        player_skin_model,
                        &texture_debug,
                    );
                    commands.entity(root).add_child(parts.head);
                    commands.entity(root).add_child(parts.body);
                    commands.entity(root).add_child(parts.arm_left);
                    commands.entity(root).add_child(parts.arm_right);
                    commands.entity(root).add_child(parts.leg_left);
                    commands.entity(root).add_child(parts.leg_right);
                    commands.entity(root).insert((
                        RemotePlayer,
                        parts,
                        HumanoidRigParts {
                            kind: HumanoidRigKind::Player,
                            model_root: root,
                            head: parts.head,
                            body: parts.body,
                            arm_right: parts.arm_right,
                            arm_left: parts.arm_left,
                            leg_right: parts.leg_right,
                            leg_left: parts.leg_left,
                            render_layer: None,
                        },
                        HumanoidArmorState::default(),
                        HumanoidArmorLayerEntities::default(),
                        RemotePlayerSkinMaterials(material_handles),
                        RemotePlayerAnimation {
                            previous_pos: pos,
                            walk_phase: 0.0,
                            swing_progress: 1.0,
                            hurt_progress: 1.0,
                        },
                        RemotePlayerSkinModel(player_skin_model),
                    ));
                } else {
                    if kind == NetEntityKind::Item {
                        // Dropped item sprite (texture applied once metadata arrives).
                        let material = materials.add(StandardMaterial {
                            base_color: Color::WHITE,
                            alpha_mode: AlphaMode::Mask(0.5),
                            cull_mode: None,
                            unlit: true,
                            perceptual_roughness: 1.0,
                            metallic: 0.0,
                            ..Default::default()
                        });
                        debug!(entity_id, pos = ?pos, "spawned dropped item placeholder awaiting metadata");
                        commands.entity(root).insert((
                            Mesh3d(item_sprite_mesh.0.clone()),
                            MeshMaterial3d(material),
                            RemoteItemSprite,
                            ItemSpin::default(),
                            RemoteDroppedItemMotion::new(root_translation, now_secs),
                            Visibility::Hidden,
                        ));
                    } else if let Some(mob) = biped_mob {
                        commands
                            .entity(root)
                            .insert(RemoteMotionSmoothing::new(root_translation, now_secs));
                        let Some(texture_path) = mob_texture_path(mob) else {
                            // Shouldn't happen since `biped_mob` is gated above.
                            continue;
                        };
                        entity_textures.request(texture_path);
                        let material =
                            entity_textures.material(texture_path).unwrap_or_else(|| {
                                materials.add(StandardMaterial {
                                    base_color: Color::srgb(1.0, 0.0, 1.0),
                                    alpha_mode: AlphaMode::Mask(0.5),
                                    unlit: true,
                                    perceptual_roughness: 1.0,
                                    metallic: 0.0,
                                    ..Default::default()
                                })
                            });

                        let spawned = spawn_model(
                            &mut commands,
                            &mut meshes,
                            material,
                            mob_biped_model(mob),
                            texture_path,
                        );
                        commands.entity(root).add_child(spawned.root);
                        commands.entity(root).insert((
                            RemoteBipedModelParts {
                                model_root: spawned.root,
                                head: spawned.parts[BIPED_HEAD],
                                body: spawned.parts[BIPED_BODY],
                                arm_right: spawned.parts[BIPED_RIGHT_ARM],
                                arm_left: spawned.parts[BIPED_LEFT_ARM],
                                leg_right: spawned.parts[BIPED_RIGHT_LEG],
                                leg_left: spawned.parts[BIPED_LEFT_LEG],
                            },
                            HumanoidRigParts {
                                kind: HumanoidRigKind::BipedMob,
                                model_root: spawned.root,
                                head: spawned.parts[BIPED_HEAD],
                                body: spawned.parts[BIPED_BODY],
                                arm_right: spawned.parts[BIPED_RIGHT_ARM],
                                arm_left: spawned.parts[BIPED_LEFT_ARM],
                                leg_right: spawned.parts[BIPED_RIGHT_LEG],
                                leg_left: spawned.parts[BIPED_LEFT_LEG],
                                render_layer: None,
                            },
                            HumanoidArmorState::default(),
                            HumanoidArmorLayerEntities::default(),
                            RemoteBipedAnimation {
                                previous_pos: pos,
                                limb_swing: 0.0,
                                limb_swing_amount: 0.0,
                                swing_progress: 1.0,
                            },
                        ));
                    } else if let Some(mob) = quadruped_mob {
                        commands
                            .entity(root)
                            .insert(RemoteMotionSmoothing::new(root_translation, now_secs));
                        let Some(texture_path) = mob_texture_path(mob) else {
                            // Shouldn't happen since `quadruped_mob` is gated above.
                            continue;
                        };
                        entity_textures.request(texture_path);
                        let material =
                            entity_textures.material(texture_path).unwrap_or_else(|| {
                                materials.add(StandardMaterial {
                                    base_color: Color::srgb(1.0, 0.0, 1.0),
                                    alpha_mode: AlphaMode::Mask(0.5),
                                    unlit: true,
                                    perceptual_roughness: 1.0,
                                    metallic: 0.0,
                                    ..Default::default()
                                })
                            });

                        let spawned = spawn_model(
                            &mut commands,
                            &mut meshes,
                            material,
                            mob_quadruped_model(mob),
                            texture_path,
                        );
                        commands.entity(root).add_child(spawned.root);
                        commands.entity(root).insert((
                            RemoteQuadrupedModelParts {
                                model_root: spawned.root,
                                head: spawned.parts[QUADRUPED_HEAD],
                                body: spawned.parts[QUADRUPED_BODY],
                                leg_front_right: spawned.parts[QUADRUPED_LEG_FRONT_RIGHT],
                                leg_front_left: spawned.parts[QUADRUPED_LEG_FRONT_LEFT],
                                leg_back_right: spawned.parts[QUADRUPED_LEG_BACK_RIGHT],
                                leg_back_left: spawned.parts[QUADRUPED_LEG_BACK_LEFT],
                            },
                            RemoteQuadrupedAnimation {
                                previous_pos: pos,
                                limb_swing: 0.0,
                                limb_swing_amount: 0.0,
                            },
                            mob_quadruped_anim_tuning(mob),
                        ));
                        if mob == MobKind::Sheep {
                            let wool_material = materials.add(StandardMaterial {
                                base_color: Color::WHITE,
                                alpha_mode: AlphaMode::Mask(0.5),
                                unlit: true,
                                perceptual_roughness: 1.0,
                                metallic: 0.0,
                                ..Default::default()
                            });
                            entity_textures.request(SHEEP_WOOL_TEXTURE_PATH);
                            let wool_mesh_entities = spawn_sheep_wool_layer(
                                &mut commands,
                                &mut meshes,
                                spawned.parts,
                                wool_material.clone(),
                            );
                            commands.entity(root).insert((
                                RemoteSheepWoolLayer {
                                    mesh_entities: wool_mesh_entities,
                                    material: wool_material,
                                },
                                // Vanilla initializes sheep metadata byte (index 16) to 0:
                                // white fleece and not sheared.
                                RemoteSheepAppearance {
                                    fleece_color: 0,
                                    sheared: false,
                                },
                            ));
                        }
                    } else {
                        commands
                            .entity(root)
                            .insert(RemoteMotionSmoothing::new(root_translation, now_secs));
                        let mesh = meshes.add(match spec.mesh {
                            VisualMesh::Capsule => Mesh::from(Capsule3d::default()),
                            VisualMesh::Sphere => Mesh::from(Sphere::default()),
                        });
                        let material = materials.add(StandardMaterial {
                            base_color: spec.color,
                            perceptual_roughness: 0.95,
                            metallic: 0.0,
                            ..Default::default()
                        });
                        commands
                            .entity(root)
                            .insert((Mesh3d(mesh), MeshMaterial3d(material)));
                    }
                }

                if let Some(uuid) = uuid {
                    registry
                        .player_entity_by_uuid
                        .insert(uuid.clone(), entity_id);
                    commands.entity(root).insert(RemoteEntityUuid(uuid));
                }

                registry.by_server_id.insert(entity_id, root);
            }
            NetEntityMessage::SetLabel { entity_id, label } => {
                if let Some(entity) = registry.by_server_id.get(&entity_id).copied() {
                    if let Ok(mut name_comp) = params.name_query.get_mut(entity) {
                        name_comp.0 = label;
                    }
                } else {
                    registry.pending_labels.insert(entity_id, label);
                }
            }
            NetEntityMessage::SetItemStack { entity_id, stack } => {
                let Some(entity) = registry.by_server_id.get(&entity_id).copied() else {
                    continue;
                };
                let Ok((remote, _look)) = params.entity_query.get_mut(entity) else {
                    continue;
                };
                if remote.kind != NetEntityKind::Item {
                    continue;
                }
                match stack {
                    Some(stack) => {
                        debug!(
                            entity_id,
                            item_id = stack.item_id,
                            damage = stack.damage,
                            count = stack.count,
                            "dropped item metadata resolved stack"
                        );
                        item_textures.request_stack(&stack);
                        if let Ok(mut commands_entity) = commands.get_entity(entity) {
                            commands_entity.insert((
                                RemoteItemStackState(stack.clone()),
                                ItemSpriteStack(stack),
                                Visibility::Visible,
                            ));
                            commands_entity.remove::<RemoteDroppedItemCollect>();
                        }
                    }
                    None => {
                        debug!(entity_id, "dropped item metadata cleared stack");
                        if let Ok(mut commands_entity) = commands.get_entity(entity) {
                            if params.item_stack_query.get(entity).is_ok() {
                                debug!(
                                    entity_id,
                                    "keeping last resolved dropped item stack until destroy"
                                );
                            } else {
                                commands_entity.remove::<RemoteItemStackState>();
                                commands_entity.remove::<ItemSpriteStack>();
                                commands_entity.insert(Visibility::Hidden);
                            }
                        }
                    }
                }
            }
            NetEntityMessage::SheepAppearance {
                entity_id,
                fleece_color,
                sheared,
            } => {
                let Some(entity) = registry.by_server_id.get(&entity_id).copied() else {
                    continue;
                };
                if let Ok((remote, _look)) = params.entity_query.get_mut(entity)
                    && remote.kind == NetEntityKind::Mob(MobKind::Sheep)
                    && let Ok(mut commands_entity) = commands.get_entity(entity)
                {
                    commands_entity.insert(RemoteSheepAppearance {
                        fleece_color,
                        sheared,
                    });
                }
            }
            NetEntityMessage::MoveDelta {
                entity_id,
                delta,
                on_ground,
            } => {
                if let Some(entity) = registry.by_server_id.get(&entity_id).copied() {
                    if let Ok(mut item_motion) = params.item_motion_query.get_mut(entity) {
                        let previous = item_motion.authoritative_translation;
                        let next = previous + delta;
                        update_item_motion_velocity(&mut item_motion, previous, next, now_secs);
                    } else if let Ok(mut smoothing) = params.smoothing_query.get_mut(entity) {
                        let previous = smoothing.target_translation;
                        let next = previous + delta;
                        update_motion_velocity(&mut smoothing, previous, next, now_secs);
                    } else if let Ok(mut transform) = params.transform_query.get_mut(entity) {
                        transform.translation += delta;
                    }
                    if let Ok((mut remote_entity, _)) = params.entity_query.get_mut(entity)
                        && let Some(on_ground) = on_ground
                    {
                        remote_entity.on_ground = on_ground;
                    }
                }
            }
            NetEntityMessage::Look {
                entity_id,
                yaw,
                pitch,
                on_ground,
            } => {
                if let Some(entity) = registry.by_server_id.get(&entity_id).copied() {
                    if let Ok((mut remote_entity, mut look)) = params.entity_query.get_mut(entity) {
                        let old_yaw = look.yaw;
                        look.yaw = yaw;
                        look.pitch = pitch;
                        if (look.head_yaw - old_yaw).abs() < 0.001 {
                            look.head_yaw = yaw;
                        }
                        if let Some(on_ground) = on_ground {
                            remote_entity.on_ground = on_ground;
                        }
                    }
                }
            }
            NetEntityMessage::HeadLook {
                entity_id,
                head_yaw,
            } => {
                if let Some(entity) = registry.by_server_id.get(&entity_id).copied() {
                    if let Ok((_remote_entity, mut look)) = params.entity_query.get_mut(entity) {
                        look.head_yaw = head_yaw;
                    }
                }
            }
            NetEntityMessage::Teleport {
                entity_id,
                pos,
                yaw,
                pitch,
                on_ground,
            } => {
                if let Some(entity) = registry.by_server_id.get(&entity_id).copied() {
                    if let Ok((mut remote_entity, mut look)) = params.entity_query.get_mut(entity) {
                        let target = entity_root_translation(
                            remote_entity.kind,
                            pos,
                            params.visual_query.get(entity).map_or(0.0, |v| v.y_offset),
                        );
                        if let Ok(mut item_motion) = params.item_motion_query.get_mut(entity) {
                            let previous = item_motion.authoritative_translation;
                            update_item_motion_velocity(
                                &mut item_motion,
                                previous,
                                target,
                                now_secs,
                            );
                            if let Ok(mut transform) = params.transform_query.get_mut(entity)
                                && transform.translation.distance_squared(target) > 64.0
                            {
                                transform.translation = target;
                                item_motion.render_translation = target;
                            }
                        } else if let Ok(mut smoothing) = params.smoothing_query.get_mut(entity) {
                            let previous = smoothing.target_translation;
                            update_motion_velocity(&mut smoothing, previous, target, now_secs);
                            // Big teleports should still snap to avoid long catch-up.
                            if let Ok(mut transform) = params.transform_query.get_mut(entity)
                                && transform.translation.distance_squared(target) > 64.0
                            {
                                transform.translation = target;
                            }
                        } else if let Ok(mut transform) = params.transform_query.get_mut(entity) {
                            transform.translation = target;
                        }
                        if let Ok(mut transform) = params.transform_query.get_mut(entity) {
                            transform.rotation = entity_root_rotation(remote_entity.kind, yaw);
                        }
                        let old_yaw = look.yaw;
                        look.yaw = yaw;
                        look.pitch = pitch;
                        if (look.head_yaw - old_yaw).abs() < 0.001 {
                            look.head_yaw = yaw;
                        }
                        remote_entity.on_ground = on_ground.unwrap_or(remote_entity.on_ground);
                    }
                }
            }
            NetEntityMessage::Velocity {
                entity_id,
                velocity,
            } => {
                if let Some(entity) = registry.by_server_id.get(&entity_id).copied()
                    && let Ok(mut item_motion) = params.item_motion_query.get_mut(entity)
                {
                    debug!(entity_id, velocity = ?velocity, "received dropped item velocity");
                    item_motion.estimated_velocity = velocity;
                    item_motion.ground_contact = false;
                    item_motion.last_server_update_secs = now_secs;
                }
            }
            NetEntityMessage::Pose {
                entity_id,
                sneaking,
            } => {
                if let Some(entity) = registry.by_server_id.get(&entity_id).copied()
                    && let Ok(mut commands_entity) = commands.get_entity(entity)
                {
                    commands_entity.insert(RemotePoseState { sneaking });
                }
            }
            NetEntityMessage::Equipment {
                entity_id,
                slot,
                item,
            } => {
                let Some(root) = registry.by_server_id.get(&entity_id).copied() else {
                    continue;
                };
                if (1..=4).contains(&slot) {
                    if registry.local_entity_id == Some(entity_id) {
                        continue;
                    }
                    if let Ok(mut slot_ref) = params.armor_state_query.get_mut(root) {
                        match slot {
                            1 => slot_ref.boots = item.clone(),
                            2 => slot_ref.leggings = item.clone(),
                            3 => slot_ref.chestplate = item.clone(),
                            4 => slot_ref.helmet = item.clone(),
                            _ => {}
                        }
                    }
                    continue;
                }
                if slot != 0 || registry.local_entity_id == Some(entity_id) {
                    continue;
                }
                let Ok((remote, _look)) = params.entity_query.get_mut(root) else {
                    continue;
                };
                if remote.kind != NetEntityKind::Player {
                    continue;
                }
                let Ok(parts) = params.player_parts_query.get(root) else {
                    continue;
                };

                if let Ok(existing) = params.held_item_query.get(root) {
                    commands.entity(existing.0).despawn_recursive();
                    commands.entity(root).remove::<RemoteHeldItem>();
                }

                let Some(stack) = item else {
                    continue;
                };

                item_textures.request_stack(&stack);
                let material = item_textures.material_for_stack(&stack).unwrap_or_else(|| {
                    materials.add(StandardMaterial {
                        base_color: Color::WHITE,
                        alpha_mode: AlphaMode::Mask(0.5),
                        cull_mode: None,
                        unlit: true,
                        perceptual_roughness: 1.0,
                        metallic: 0.0,
                        ..Default::default()
                    })
                });
                let item_entity = commands
                    .spawn((
                        Name::new("RemoteHeldItem"),
                        Mesh3d(item_sprite_mesh.0.clone()),
                        MeshMaterial3d(material),
                        Transform {
                            translation: Vec3::new(0.02, -0.86, -0.18),
                            rotation: Quat::from_rotation_x(-0.30) * Quat::from_rotation_y(0.35),
                            scale: Vec3::splat(0.55),
                            ..Default::default()
                        },
                        GlobalTransform::default(),
                        Visibility::Visible,
                        InheritedVisibility::default(),
                        ViewVisibility::default(),
                        ItemSpriteStack(stack),
                    ))
                    .id();
                commands.entity(parts.arm_right).add_child(item_entity);
                commands.entity(root).insert(RemoteHeldItem(item_entity));
            }
            NetEntityMessage::Animation {
                entity_id,
                animation,
            } => {
                if let Some(entity) = registry.by_server_id.get(&entity_id).copied() {
                    if let Ok(mut anim) = params.player_anim_query.get_mut(entity) {
                        match animation {
                            NetEntityAnimation::SwingMainArm => anim.swing_progress = 0.0,
                            NetEntityAnimation::TakeDamage => anim.hurt_progress = 0.0,
                            NetEntityAnimation::LeaveBed | NetEntityAnimation::Unknown(_) => {}
                        }
                    }
                    if let Ok(mut anim) = params.biped_anim_query.get_mut(entity) {
                        if matches!(animation, NetEntityAnimation::SwingMainArm) {
                            anim.swing_progress = 0.0;
                        }
                    }
                }
            }
            NetEntityMessage::CollectItem {
                collected_entity_id,
                collector_entity_id,
            } => {
                let Some(entity) = registry.by_server_id.get(&collected_entity_id).copied() else {
                    registry.pending_labels.remove(&collected_entity_id);
                    continue;
                };
                let Ok((remote, _look)) = params.entity_query.get_mut(entity) else {
                    continue;
                };
                if remote.kind != NetEntityKind::Item {
                    continue;
                }
                if let Ok(mut commands_entity) = commands.get_entity(entity) {
                    commands_entity.insert(RemoteDroppedItemCollect {
                        collector_server_id: Some(collector_entity_id),
                        progress_secs: 0.0,
                    });
                }
            }
            NetEntityMessage::Destroy { entity_ids } => {
                for entity_id in entity_ids {
                    registry.pending_labels.remove(&entity_id);
                    if let Some(entity) = registry.by_server_id.remove(&entity_id) {
                        commands.entity(entity).despawn_recursive();
                    }
                    registry
                        .player_entity_by_uuid
                        .retain(|_, id| *id != entity_id);
                }
            }
        }
    }
}
