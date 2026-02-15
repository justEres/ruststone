use std::collections::{HashMap, HashSet, VecDeque};
use std::thread;

use crate::sim::collision::{WorldCollisionMap, is_solid};
use bevy::ecs::system::SystemParam;
use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::mesh::Indices;
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::PrimitiveTopology;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy_egui::{EguiContexts, egui};
use crossbeam::channel::{Receiver, Sender, unbounded};
use rs_render::PlayerCamera;
use rs_utils::{
    AppState, ApplicationState, InventoryItemStack, MobKind, NetEntityAnimation, NetEntityKind,
    NetEntityMessage, ObjectKind, PlayerSkinModel,
};
use tracing::{info, warn};

use crate::entity_model::{
    BIPED_BODY, BIPED_HEAD, BIPED_LEFT_ARM, BIPED_LEFT_LEG, BIPED_MODEL_TEX32, BIPED_MODEL_TEX64,
    BIPED_RIGHT_ARM, BIPED_RIGHT_LEG, EntityTextureCache, EntityTexturePath, spawn_model,
};

use crate::item_textures::{ItemSpriteMesh, ItemTextureCache};
use crate::sim::{CameraPerspectiveMode, CameraPerspectiveState};
use rs_render::RenderDebugSettings;
use rs_render::{LookAngles, Player};
use rs_ui::ConnectUiState;

const PLAYER_SCALE: Vec3 = Vec3::ONE;
const PLAYER_Y_OFFSET: f32 = 0.0;
const PLAYER_NAME_Y_OFFSET: f32 = 2.05;

const MOB_SCALE: Vec3 = Vec3::new(0.55, 0.9, 0.55);
const MOB_Y_OFFSET: f32 = 0.9;
const MOB_NAME_Y_OFFSET: f32 = 1.35;

const ITEM_SCALE: Vec3 = Vec3::splat(0.17);
const ITEM_Y_OFFSET: f32 = 0.17;
const ITEM_NAME_Y_OFFSET: f32 = 0.5;

const ORB_SCALE: Vec3 = Vec3::splat(0.14);
const ORB_Y_OFFSET: f32 = 0.14;
const ORB_NAME_Y_OFFSET: f32 = 0.45;

const OBJECT_SCALE: Vec3 = Vec3::splat(0.28);
const OBJECT_Y_OFFSET: f32 = 0.28;
const OBJECT_NAME_Y_OFFSET: f32 = 0.65;

#[derive(Component, Debug, Clone, Copy)]
pub struct RemoteItemSprite;

#[derive(Component, Debug, Clone, Copy)]
pub struct RemoteItemStackState(pub InventoryItemStack);

#[derive(Component, Debug, Clone, Copy)]
pub struct ItemSpriteStack(pub InventoryItemStack);

#[derive(Component, Debug, Clone, Copy, Default)]
pub struct ItemSpin(pub f32);

#[derive(SystemParam)]
pub(crate) struct RemoteEntityApplyParams<'w, 's> {
    transform_query: Query<'w, 's, &'static mut Transform>,
    smoothing_query: Query<'w, 's, &'static mut RemoteMotionSmoothing>,
    entity_query: Query<'w, 's, (&'static mut RemoteEntity, &'static mut RemoteEntityLook)>,
    player_anim_query: Query<'w, 's, &'static mut RemotePlayerAnimation>,
    biped_anim_query: Query<'w, 's, &'static mut RemoteBipedAnimation>,
    name_query: Query<'w, 's, &'static mut RemoteEntityName>,
    visual_query: Query<'w, 's, &'static RemoteVisual>,
    player_parts_query: Query<'w, 's, &'static RemotePlayerModelParts, With<RemotePlayer>>,
    held_item_query: Query<'w, 's, &'static RemoteHeldItem>,
}

#[derive(Debug)]
struct SkinDownloadResult {
    skin_url: String,
    rgba: Vec<u8>,
    width: u32,
    height: u32,
}

#[derive(Resource)]
pub struct RemoteSkinDownloader {
    request_tx: Sender<String>,
    result_rx: Receiver<SkinDownloadResult>,
    requested: HashSet<String>,
    loaded: HashMap<String, Handle<Image>>,
}

#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct PlayerTextureDebugSettings {
    pub flip_u: bool,
    pub flip_v: bool,
}

impl Default for RemoteSkinDownloader {
    fn default() -> Self {
        let (request_tx, request_rx) = unbounded::<String>();
        let (result_tx, result_rx) = unbounded::<SkinDownloadResult>();
        thread::spawn(move || skin_download_worker(request_rx, result_tx));
        Self {
            request_tx,
            result_rx,
            requested: HashSet::new(),
            loaded: HashMap::new(),
        }
    }
}

impl RemoteSkinDownloader {
    pub fn request(&mut self, skin_url: String) {
        if !self.requested.insert(skin_url.clone()) {
            return;
        }
        info!("queue skin fetch: {skin_url}");
        let _ = self.request_tx.send(skin_url);
    }

    pub fn skin_handle(&self, skin_url: &str) -> Option<Handle<Image>> {
        self.loaded.get(skin_url).cloned()
    }
}

#[derive(Default, Resource)]
pub struct RemoteEntityEventQueue {
    events: VecDeque<NetEntityMessage>,
}

impl RemoteEntityEventQueue {
    pub fn push(&mut self, event: NetEntityMessage) {
        self.events.push_back(event);
    }

    pub fn drain(&mut self) -> std::collections::vec_deque::Drain<'_, NetEntityMessage> {
        self.events.drain(..)
    }
}

#[derive(Default, Resource)]
pub struct RemoteEntityRegistry {
    pub local_entity_id: Option<i32>,
    pub by_server_id: HashMap<i32, Entity>,
    pub player_entity_by_uuid: HashMap<rs_protocol::protocol::UUID, i32>,
    pub player_name_by_uuid: HashMap<rs_protocol::protocol::UUID, String>,
    pub player_skin_url_by_uuid: HashMap<rs_protocol::protocol::UUID, String>,
    pub player_skin_model_by_uuid: HashMap<rs_protocol::protocol::UUID, PlayerSkinModel>,
    pub pending_labels: HashMap<i32, String>,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct RemoteEntity {
    pub server_id: i32,
    pub kind: NetEntityKind,
    pub on_ground: bool,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct RemoteEntityLook {
    pub yaw: f32,
    pub pitch: f32,
    pub head_yaw: f32,
}

#[derive(Component, Debug, Clone)]
pub struct RemoteEntityUuid(pub rs_protocol::protocol::UUID);

#[derive(Component, Debug, Clone)]
pub struct RemoteEntityName(pub String);

#[derive(Component)]
pub struct RemotePlayer;

#[derive(Component, Debug, Clone)]
pub struct RemotePlayerModelParts {
    pub head: Entity,
    pub body: Entity,
    pub arm_left: Entity,
    pub arm_right: Entity,
    pub leg_left: Entity,
    pub leg_right: Entity,
}

#[derive(Component, Debug, Clone)]
pub struct RemotePlayerSkinMaterials(pub Vec<Handle<StandardMaterial>>);

#[derive(Component, Debug, Clone, Copy)]
pub struct RemotePlayerAnimation {
    pub previous_pos: Vec3,
    pub walk_phase: f32,
    pub swing_progress: f32,
    pub hurt_progress: f32,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct RemotePlayerSkinModel(pub PlayerSkinModel);

#[derive(Component, Debug, Clone, Copy)]
pub struct RemoteVisual {
    pub y_offset: f32,
    pub name_y_offset: f32,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct RemoteHeldItem(pub Entity);

#[derive(Component, Debug, Clone, Copy, Default)]
pub struct RemotePoseState {
    pub sneaking: bool,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct LocalPlayerModel;

#[derive(Component, Debug, Clone)]
pub struct LocalPlayerModelParts {
    pub head: Entity,
    pub body: Entity,
    pub arm_left: Entity,
    pub arm_right: Entity,
    pub leg_left: Entity,
    pub leg_right: Entity,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct LocalPlayerAnimation {
    pub walk_phase: f32,
    pub swing_progress: f32,
    pub hurt_progress: f32,
}

#[derive(Component, Debug, Clone)]
pub struct LocalPlayerSkinMaterial(pub Handle<StandardMaterial>);

#[derive(Component, Debug, Clone)]
pub struct RemoteBipedModelParts {
    pub model_root: Entity,
    pub head: Entity,
    pub body: Entity,
    pub arm_right: Entity,
    pub arm_left: Entity,
    pub leg_right: Entity,
    pub leg_left: Entity,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct RemoteBipedAnimation {
    pub previous_pos: Vec3,
    pub limb_swing: f32,
    pub limb_swing_amount: f32,
    pub swing_progress: f32,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct RemoteMotionSmoothing {
    pub target_translation: Vec3,
    pub estimated_velocity: Vec3,
    pub last_server_update_secs: f64,
}

impl RemoteMotionSmoothing {
    fn new(target_translation: Vec3, now_secs: f64) -> Self {
        Self {
            target_translation,
            estimated_velocity: Vec3::ZERO,
            last_server_update_secs: now_secs,
        }
    }
}

pub fn remote_entity_connection_sync(
    app_state: Res<AppState>,
    mut queue: ResMut<RemoteEntityEventQueue>,
    mut registry: ResMut<RemoteEntityRegistry>,
    mut was_connected: Local<bool>,
) {
    let connected = matches!(app_state.0, ApplicationState::Connected);
    if connected == *was_connected {
        return;
    }
    *was_connected = connected;

    if !registry.by_server_id.is_empty() {
        queue.push(NetEntityMessage::Destroy {
            entity_ids: registry.by_server_id.keys().copied().collect(),
        });
    }
    registry.local_entity_id = None;
    registry.player_entity_by_uuid.clear();
    registry.player_name_by_uuid.clear();
    registry.player_skin_url_by_uuid.clear();
    registry.player_skin_model_by_uuid.clear();
    registry.pending_labels.clear();
}

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

                let spawn_cmd = commands.spawn((
                    Name::new(format!("RemoteEntity[{entity_id}]")),
                    Transform {
                        translation: pos + Vec3::Y * visual.y_offset,
                        rotation: entity_root_rotation(kind, yaw),
                        scale: if biped_mob.is_some() {
                            Vec3::ONE
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
                    RemoteMotionSmoothing::new(pos + Vec3::Y * visual.y_offset, now_secs),
                    RemotePoseState::default(),
                ));
                let root = spawn_cmd.id();

                if kind == NetEntityKind::Player {
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
                        commands.entity(root).insert((
                            Mesh3d(item_sprite_mesh.0.clone()),
                            MeshMaterial3d(material),
                            RemoteItemSprite,
                            ItemSpin::default(),
                        ));
                    } else if let Some(mob) = biped_mob {
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
                            RemoteBipedAnimation {
                                previous_pos: pos,
                                limb_swing: 0.0,
                                limb_swing_amount: 0.0,
                                swing_progress: 1.0,
                            },
                        ));
                    } else {
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
                        item_textures.request_stack(stack);
                        if let Ok(mut commands_entity) = commands.get_entity(entity) {
                            commands_entity
                                .insert((RemoteItemStackState(stack), ItemSpriteStack(stack)));
                        }
                    }
                    None => {
                        if let Ok(mut commands_entity) = commands.get_entity(entity) {
                            commands_entity.remove::<RemoteItemStackState>();
                            commands_entity.remove::<ItemSpriteStack>();
                        }
                    }
                }
            }
            NetEntityMessage::MoveDelta {
                entity_id,
                delta,
                on_ground,
            } => {
                if let Some(entity) = registry.by_server_id.get(&entity_id).copied() {
                    if let Ok(mut smoothing) = params.smoothing_query.get_mut(entity) {
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
                        let target = pos
                            + Vec3::Y
                                * params
                                    .visual_query
                                    .get(entity)
                                    .map_or(PLAYER_Y_OFFSET, |v| v.y_offset);
                        if let Ok(mut smoothing) = params.smoothing_query.get_mut(entity) {
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
            NetEntityMessage::Velocity { .. } => {}
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
                // For now, only visualize the held item (slot 0) on remote players.
                if slot != 0 {
                    continue;
                }
                let Some(root) = registry.by_server_id.get(&entity_id).copied() else {
                    continue;
                };
                if registry.local_entity_id == Some(entity_id) {
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

                item_textures.request_stack(stack);
                let material = item_textures.material_for_stack(stack).unwrap_or_else(|| {
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
                ..
            } => {
                // For now, despawn immediately to avoid "ghost" items lingering.
                if let Some(entity) = registry.by_server_id.remove(&collected_entity_id) {
                    commands.entity(entity).despawn_recursive();
                }
                registry.pending_labels.remove(&collected_entity_id);
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

fn update_motion_velocity(
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

pub fn draw_remote_entity_names(
    mut contexts: EguiContexts,
    camera_query: Query<(&Camera, &GlobalTransform), With<PlayerCamera>>,
    names_query: Query<
        (
            &GlobalTransform,
            &RemoteEntityName,
            &RemoteVisual,
            &RemoteEntity,
        ),
        With<RemoteEntity>,
    >,
    collision_map: Res<WorldCollisionMap>,
) {
    let Ok((camera, camera_transform)) = camera_query.get_single() else {
        return;
    };
    let ctx = contexts.ctx_mut().unwrap();
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("remote_player_names"),
    ));

    let cam_pos = camera_transform.translation();
    for (transform, name, visual, remote) in &names_query {
        let world_pos = transform.translation() + Vec3::Y * visual.name_y_offset;
        let through_walls = remote.kind == NetEntityKind::Player;
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
            egui::Color32::WHITE,
        );
    }
}

pub fn apply_item_sprite_textures_system(
    mut cache: ResMut<ItemTextureCache>,
    mut query: Query<(&ItemSpriteStack, &mut MeshMaterial3d<StandardMaterial>)>,
) {
    for (stack, mut material) in &mut query {
        cache.request_stack(stack.0);
        if let Some(handle) = cache.material_for_stack(stack.0) {
            if material.0 != handle {
                material.0 = handle;
            }
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

fn skin_download_worker(request_rx: Receiver<String>, result_tx: Sender<SkinDownloadResult>) {
    while let Ok(skin_url) = request_rx.recv() {
        info!("fetching skin: {skin_url}");
        let Ok(response) = reqwest::blocking::get(&skin_url) else {
            warn!("skin fetch failed (request): {skin_url}");
            continue;
        };
        let Ok(bytes) = response.bytes() else {
            warn!("skin fetch failed (bytes): {skin_url}");
            continue;
        };
        let Ok(decoded) = image::load_from_memory(&bytes) else {
            warn!("skin fetch failed (decode): {skin_url}");
            continue;
        };
        let rgba = decoded.to_rgba8();
        let (width, height) = rgba.dimensions();
        info!("skin fetched: {skin_url} ({width}x{height})");
        let _ = result_tx.send(SkinDownloadResult {
            skin_url,
            rgba: rgba.into_raw(),
            width,
            height,
        });
    }
}

pub fn remote_skin_download_tick(
    mut downloader: ResMut<RemoteSkinDownloader>,
    mut images: ResMut<Assets<Image>>,
) {
    while let Ok(downloaded) = downloader.result_rx.try_recv() {
        let mut image = Image::new_fill(
            Extent3d {
                width: downloaded.width,
                height: downloaded.height,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            &[0, 0, 0, 0],
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        );
        image.data = Some(downloaded.rgba);
        let mut sampler = ImageSamplerDescriptor::nearest();
        sampler.address_mode_u = ImageAddressMode::ClampToEdge;
        sampler.address_mode_v = ImageAddressMode::ClampToEdge;
        image.sampler = ImageSampler::Descriptor(sampler);
        let handle = images.add(image);
        downloader.loaded.insert(downloaded.skin_url, handle);
    }
}

pub fn apply_remote_player_skins(
    registry: Res<RemoteEntityRegistry>,
    downloader: Res<RemoteSkinDownloader>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    player_query: Query<(&RemoteEntityUuid, &RemotePlayerSkinMaterials), With<RemotePlayer>>,
) {
    for (uuid, player_mats) in &player_query {
        let Some(skin_url) = registry.player_skin_url_by_uuid.get(&uuid.0) else {
            continue;
        };
        let Some(texture_handle) = downloader.skin_handle(skin_url) else {
            continue;
        };
        for mat_handle in &player_mats.0 {
            let Some(material) = materials.get_mut(mat_handle) else {
                continue;
            };
            if material.base_color_texture.as_ref() != Some(&texture_handle) {
                material.base_color_texture = Some(texture_handle.clone());
                material.base_color = Color::WHITE;
                material.alpha_mode = AlphaMode::Mask(0.5);
                material.unlit = false;
            }
        }
    }
}

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
    if !connected || !render_debug.render_self_model {
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
        base_color_texture: skin_handle,
        alpha_mode: AlphaMode::Mask(0.5),
        perceptual_roughness: 0.95,
        metallic: 0.0,
        ..Default::default()
    });

    let model_root = commands
        .spawn((
            Name::new("LocalPlayerModel"),
            LocalPlayerModel,
            // Match the remote player model facing (player root doesn't include the +PI).
            Transform::from_rotation(Quat::from_rotation_y(std::f32::consts::PI)),
            GlobalTransform::default(),
            Visibility::Inherited,
            InheritedVisibility::default(),
            ViewVisibility::default(),
            LocalPlayerSkinMaterial(base_mat.clone()),
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
    );
    commands.entity(model_root).add_child(parts.head);
    commands.entity(model_root).add_child(parts.body);
    commands.entity(model_root).add_child(parts.arm_left);
    commands.entity(model_root).add_child(parts.arm_right);
    commands.entity(model_root).add_child(parts.leg_left);
    commands.entity(model_root).add_child(parts.leg_right);

    commands.entity(model_root).insert((
        parts,
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
    mut query: Query<&mut Visibility, With<LocalPlayerModel>>,
) {
    let Ok(mut vis) = query.get_single_mut() else {
        return;
    };

    let should_show = render_debug.render_self_model
        && !matches!(perspective.mode, CameraPerspectiveMode::FirstPerson);
    *vis = if should_show {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
}

pub fn update_local_player_skin_system(
    app_state: Res<AppState>,
    connect_ui: Res<ConnectUiState>,
    registry: Res<RemoteEntityRegistry>,
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
    if material.base_color_texture.as_ref() != Some(&desired) {
        material.base_color_texture = Some(desired);
        material.base_color = Color::WHITE;
    }
}

pub fn animate_local_player_model_system(
    time: Res<Time>,
    input: Res<crate::sim::CurrentInput>,
    sim_state: Res<crate::sim::SimState>,
    render_debug: Res<RenderDebugSettings>,
    mut roots: Query<(&LocalPlayerModelParts, &mut LocalPlayerAnimation), With<LocalPlayerModel>>,
    mut part_transforms: Query<&mut Transform, Without<LocalPlayerModel>>,
    player_query: Query<&LookAngles, With<Player>>,
) {
    if !render_debug.render_self_model {
        return;
    }

    let Ok(look) = player_query.get_single() else {
        return;
    };
    let Ok((parts, mut anim)) = roots.get_single_mut() else {
        return;
    };

    let dt = time.delta_secs().max(1e-4);
    let vel = sim_state.current.vel;
    let speed = (Vec2::new(vel.x, vel.z).length() * 20.0).min(8.0);
    let stride = (speed / 4.0).clamp(0.0, 1.0);
    anim.walk_phase += speed * dt * 2.5;

    let swing = anim.walk_phase.sin() * 0.7 * stride;
    let sneak_amount = if input.0.sneak { 1.0 } else { 0.0 };

    // Head follows camera pitch; yaw comes from the player root rotation.
    if let Ok(mut t) = part_transforms.get_mut(parts.head) {
        t.translation = Vec3::new(0.0, 1.75 - 0.1 * sneak_amount, 0.0);
        t.rotation = Quat::from_rotation_x(-look.pitch - 0.2 * sneak_amount);
    }
    if let Ok(mut t) = part_transforms.get_mut(parts.body) {
        t.translation = Vec3::new(0.0, 1.125 - 0.1 * sneak_amount, 0.0);
        t.rotation = Quat::from_rotation_x(0.5 * sneak_amount);
    }
    if let Ok(mut t) = part_transforms.get_mut(parts.arm_left) {
        t.translation = Vec3::new(-0.375, 1.125, 0.0);
        t.rotation = Quat::from_rotation_x(swing + 0.4 * sneak_amount);
    }
    if let Ok(mut t) = part_transforms.get_mut(parts.arm_right) {
        t.translation = Vec3::new(0.375, 1.125, 0.0);
        t.rotation = Quat::from_rotation_x(-swing + 0.4 * sneak_amount);
    }
    if let Ok(mut t) = part_transforms.get_mut(parts.leg_left) {
        t.translation = Vec3::new(-0.125, 0.375 - 0.2 * sneak_amount, 0.0);
        t.rotation = Quat::from_rotation_x(-swing * (1.0 - 0.6 * sneak_amount));
    }
    if let Ok(mut t) = part_transforms.get_mut(parts.leg_right) {
        t.translation = Vec3::new(0.125, 0.375 - 0.2 * sneak_amount, 0.0);
        t.rotation = Quat::from_rotation_x(swing * (1.0 - 0.6 * sneak_amount));
    }
}

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
        );
        rebuild_part_children(
            &mut commands,
            &mut meshes,
            &children_query,
            parts.body,
            base_material,
            player_body_meshes(&settings),
        );
        rebuild_part_children(
            &mut commands,
            &mut meshes,
            &children_query,
            parts.arm_left,
            base_material,
            player_left_arm_meshes(skin_model.0, &settings),
        );
        rebuild_part_children(
            &mut commands,
            &mut meshes,
            &children_query,
            parts.arm_right,
            base_material,
            player_right_arm_meshes(skin_model.0, &settings),
        );
        rebuild_part_children(
            &mut commands,
            &mut meshes,
            &children_query,
            parts.leg_left,
            base_material,
            player_left_leg_meshes(&settings),
        );
        rebuild_part_children(
            &mut commands,
            &mut meshes,
            &children_query,
            parts.leg_right,
            base_material,
            player_right_leg_meshes(&settings),
        );
    }
}

fn rebuild_part_children(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    children_query: &Query<&Children>,
    pivot: Entity,
    base_material: &Handle<StandardMaterial>,
    part_meshes: Vec<Mesh>,
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
                Transform::IDENTITY,
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
            &mut RemotePlayerAnimation,
        ),
        With<RemotePlayer>,
    >,
    mut part_transforms: Query<&mut Transform, Without<RemotePlayer>>,
) {
    let dt = time.delta_secs().max(1e-4);
    for (root_transform, look, pose, parts, mut anim) in &mut roots {
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

        if let Ok(mut t) = part_transforms.get_mut(parts.head) {
            t.translation = Vec3::new(0.0, 1.75 - 0.1 * sneak_amount, 0.0);
            t.rotation = Quat::from_rotation_y(head_yaw_delta)
                * Quat::from_rotation_x(-head_pitch - 0.2 * sneak_amount);
        }
        if let Ok(mut t) = part_transforms.get_mut(parts.body) {
            t.translation = Vec3::new(0.0, 1.125 - 0.1 * sneak_amount, 0.0);
            t.rotation =
                Quat::from_rotation_x(0.5 * sneak_amount) * Quat::from_rotation_z(hurt_tilt);
        }
        if let Ok(mut t) = part_transforms.get_mut(parts.arm_left) {
            t.translation = Vec3::new(-0.375, 1.125, 0.0);
            t.rotation = Quat::from_rotation_x(swing + 0.4 * sneak_amount);
        }
        if let Ok(mut t) = part_transforms.get_mut(parts.arm_right) {
            t.translation = Vec3::new(0.375, 1.125, 0.0);
            t.rotation = Quat::from_rotation_x(-swing - arm_attack + 0.4 * sneak_amount);
        }
        if let Ok(mut t) = part_transforms.get_mut(parts.leg_left) {
            t.translation = Vec3::new(-0.125, 0.375 - 0.2 * sneak_amount, 0.0);
            t.rotation = Quat::from_rotation_x(-swing * (1.0 - 0.6 * sneak_amount));
        }
        if let Ok(mut t) = part_transforms.get_mut(parts.leg_right) {
            t.translation = Vec3::new(0.125, 0.375 - 0.2 * sneak_amount, 0.0);
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

fn spawn_remote_player_model(
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
        base_color_texture: player_skin,
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
        Vec3::new(0.0, 1.75, 0.0),
    );
    let body = spawn_player_part(
        commands,
        meshes,
        materials,
        &base_mat,
        player_body_meshes(texture_debug),
        Vec3::new(0.0, 1.125, 0.0),
    );
    let arm_left = spawn_player_part(
        commands,
        meshes,
        materials,
        &base_mat,
        player_left_arm_meshes(skin_model, texture_debug),
        Vec3::new(-0.375, 1.125, 0.0),
    );
    let arm_right = spawn_player_part(
        commands,
        meshes,
        materials,
        &base_mat,
        player_right_arm_meshes(skin_model, texture_debug),
        Vec3::new(0.375, 1.125, 0.0),
    );
    let leg_left = spawn_player_part(
        commands,
        meshes,
        materials,
        &base_mat,
        player_left_leg_meshes(texture_debug),
        Vec3::new(-0.125, 0.375, 0.0),
    );
    let leg_right = spawn_player_part(
        commands,
        meshes,
        materials,
        &base_mat,
        player_right_leg_meshes(texture_debug),
        Vec3::new(0.125, 0.375, 0.0),
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

fn spawn_player_model_with_material(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    base_material: &Handle<StandardMaterial>,
    skin_model: PlayerSkinModel,
    texture_debug: &PlayerTextureDebugSettings,
) -> LocalPlayerModelParts {
    let head = spawn_player_part(
        commands,
        meshes,
        materials,
        base_material,
        player_head_meshes(texture_debug),
        Vec3::new(0.0, 1.75, 0.0),
    );
    let body = spawn_player_part(
        commands,
        meshes,
        materials,
        base_material,
        player_body_meshes(texture_debug),
        Vec3::new(0.0, 1.125, 0.0),
    );
    let arm_left = spawn_player_part(
        commands,
        meshes,
        materials,
        base_material,
        player_left_arm_meshes(skin_model, texture_debug),
        Vec3::new(-0.375, 1.125, 0.0),
    );
    let arm_right = spawn_player_part(
        commands,
        meshes,
        materials,
        base_material,
        player_right_arm_meshes(skin_model, texture_debug),
        Vec3::new(0.375, 1.125, 0.0),
    );
    let leg_left = spawn_player_part(
        commands,
        meshes,
        materials,
        base_material,
        player_left_leg_meshes(texture_debug),
        Vec3::new(-0.125, 0.375, 0.0),
    );
    let leg_right = spawn_player_part(
        commands,
        meshes,
        materials,
        base_material,
        player_right_leg_meshes(texture_debug),
        Vec3::new(0.125, 0.375, 0.0),
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

fn spawn_player_part(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
    base_material: &Handle<StandardMaterial>,
    part_meshes: Vec<Mesh>,
    translation: Vec3,
) -> Entity {
    let mut children = Vec::new();
    for mesh in part_meshes {
        let child = commands
            .spawn((
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(base_material.clone()),
                Transform::IDENTITY,
                GlobalTransform::default(),
                Visibility::Visible,
                InheritedVisibility::default(),
                ViewVisibility::default(),
            ))
            .id();
        children.push(child);
    }

    let pivot = commands
        .spawn((
            Transform::from_translation(translation),
            GlobalTransform::default(),
            Visibility::Visible,
            InheritedVisibility::default(),
            ViewVisibility::default(),
        ))
        .id();

    for child in children {
        commands.entity(pivot).add_child(child);
    }
    pivot
}

fn player_head_meshes(texture_debug: &PlayerTextureDebugSettings) -> Vec<Mesh> {
    vec![
        make_skin_box_with_faces(8.0, 8.0, 8.0, 0.0, head_base_face_rects(), texture_debug),
        make_skin_box_with_faces(8.0, 8.0, 8.0, 0.5, head_outer_face_rects(), texture_debug),
    ]
}

fn player_body_meshes(texture_debug: &PlayerTextureDebugSettings) -> Vec<Mesh> {
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

fn player_right_arm_meshes(
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

fn player_left_arm_meshes(
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

fn player_right_leg_meshes(texture_debug: &PlayerTextureDebugSettings) -> Vec<Mesh> {
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

fn player_left_leg_meshes(texture_debug: &PlayerTextureDebugSettings) -> Vec<Mesh> {
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

fn make_skin_box_with_faces(
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
struct SkinUvRect {
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
struct SkinFaceMap {
    down: SkinUvRect,
    up: SkinUvRect,
    north: SkinUvRect,
    south: SkinUvRect,
    west: SkinUvRect,
    east: SkinUvRect,
}

fn rect(x1: f32, y1: f32, x2: f32, y2: f32) -> SkinUvRect {
    SkinUvRect::new(x1, y1, x2 - x1, y2 - y1)
}

fn map_from_named_faces(
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

fn head_base_face_rects() -> SkinFaceMap {
    map_from_named_faces(
        rect(8.0, 0.0, 16.0, 8.0),
        rect(16.0, 0.0, 24.0, 8.0),
        rect(0.0, 8.0, 8.0, 16.0),
        rect(8.0, 8.0, 16.0, 16.0),
        rect(16.0, 8.0, 24.0, 16.0),
        rect(24.0, 8.0, 32.0, 16.0),
    )
}

fn head_outer_face_rects() -> SkinFaceMap {
    map_from_named_faces(
        rect(40.0, 0.0, 48.0, 8.0),
        rect(48.0, 0.0, 56.0, 8.0),
        rect(32.0, 8.0, 40.0, 16.0),
        rect(40.0, 8.0, 48.0, 16.0),
        rect(48.0, 8.0, 56.0, 16.0),
        rect(56.0, 8.0, 64.0, 16.0),
    )
}

fn torso_base_face_rects() -> SkinFaceMap {
    map_from_named_faces(
        rect(20.0, 16.0, 28.0, 20.0),
        rect(28.0, 16.0, 36.0, 20.0),
        rect(16.0, 20.0, 20.0, 32.0),
        rect(20.0, 20.0, 28.0, 32.0),
        rect(28.0, 20.0, 32.0, 32.0),
        rect(32.0, 20.0, 40.0, 32.0),
    )
}

fn torso_outer_face_rects() -> SkinFaceMap {
    map_from_named_faces(
        rect(20.0, 32.0, 28.0, 36.0),
        rect(28.0, 32.0, 36.0, 36.0),
        rect(16.0, 36.0, 20.0, 48.0),
        rect(20.0, 36.0, 28.0, 48.0),
        rect(28.0, 36.0, 32.0, 48.0),
        rect(32.0, 36.0, 40.0, 48.0),
    )
}

fn right_arm_base_face_rects(model: PlayerSkinModel) -> SkinFaceMap {
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

fn right_arm_outer_face_rects(model: PlayerSkinModel) -> SkinFaceMap {
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

fn left_arm_base_face_rects(model: PlayerSkinModel) -> SkinFaceMap {
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

fn left_arm_outer_face_rects(model: PlayerSkinModel) -> SkinFaceMap {
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

fn right_leg_base_face_rects() -> SkinFaceMap {
    map_from_named_faces(
        rect(4.0, 16.0, 8.0, 20.0),
        rect(8.0, 16.0, 12.0, 20.0),
        rect(0.0, 20.0, 4.0, 32.0),
        rect(4.0, 20.0, 8.0, 32.0),
        rect(8.0, 20.0, 12.0, 32.0),
        rect(12.0, 20.0, 16.0, 32.0),
    )
}

fn right_leg_outer_face_rects() -> SkinFaceMap {
    map_from_named_faces(
        rect(4.0, 32.0, 8.0, 36.0),
        rect(8.0, 32.0, 12.0, 36.0),
        rect(0.0, 36.0, 4.0, 48.0),
        rect(4.0, 36.0, 8.0, 48.0),
        rect(8.0, 36.0, 12.0, 48.0),
        rect(12.0, 36.0, 16.0, 48.0),
    )
}

fn left_leg_base_face_rects() -> SkinFaceMap {
    map_from_named_faces(
        rect(20.0, 48.0, 24.0, 52.0),
        rect(24.0, 48.0, 28.0, 52.0),
        rect(16.0, 52.0, 20.0, 64.0),
        rect(20.0, 52.0, 24.0, 64.0),
        rect(24.0, 52.0, 28.0, 64.0),
        rect(28.0, 52.0, 32.0, 64.0),
    )
}

fn left_leg_outer_face_rects() -> SkinFaceMap {
    map_from_named_faces(
        rect(4.0, 48.0, 8.0, 52.0),
        rect(8.0, 48.0, 12.0, 52.0),
        rect(0.0, 52.0, 4.0, 64.0),
        rect(4.0, 52.0, 8.0, 64.0),
        rect(8.0, 52.0, 12.0, 64.0),
        rect(12.0, 52.0, 16.0, 64.0),
    )
}

fn add_face(
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    uvs: &mut Vec<[f32; 2]>,
    indices: &mut Vec<u32>,
    verts: [[f32; 3]; 4],
    normal: [f32; 3],
    rect: SkinUvRect,
    texture_debug: &PlayerTextureDebugSettings,
) {
    let mut verts = verts;
    let mut uv = uv_rect(rect, texture_debug.flip_u, texture_debug.flip_v);

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

fn uv_rect(rect: SkinUvRect, flip_u: bool, flip_v: bool) -> [[f32; 2]; 4] {
    let u0 = rect.u / 64.0;
    let u1 = (rect.u + rect.w) / 64.0;
    let v0 = rect.v / 64.0;
    let v1 = (rect.v + rect.h) / 64.0;
    let mut out = [[u0, v1], [u1, v1], [u1, v0], [u0, v0]];
    if flip_u {
        for uv in &mut out {
            uv[0] = 1.0 - uv[0];
        }
    }
    if flip_v {
        for uv in &mut out {
            uv[1] = 1.0 - uv[1];
        }
    }
    out
}

fn player_root_rotation(yaw: f32) -> Quat {
    // Align model forward/back with Minecraft protocol-facing direction.
    Quat::from_axis_angle(Vec3::Y, yaw + std::f32::consts::PI)
}

fn entity_root_rotation(kind: NetEntityKind, yaw: f32) -> Quat {
    match kind {
        // Player skin model pipeline has its own historic 180deg alignment.
        NetEntityKind::Player => player_root_rotation(yaw),
        // Vanilla biped mob models should use yaw directly (no extra PI).
        NetEntityKind::Mob(m) if mob_uses_biped_model(m) => Quat::from_axis_angle(Vec3::Y, yaw),
        _ => Quat::from_axis_angle(Vec3::Y, yaw),
    }
}

#[derive(Clone, Copy)]
enum VisualMesh {
    Capsule,
    Sphere,
}

#[derive(Clone, Copy)]
struct VisualSpec {
    mesh: VisualMesh,
    scale: Vec3,
    y_offset: f32,
    name_y_offset: f32,
    color: Color,
}

fn visual_for_kind(kind: NetEntityKind) -> RemoteVisual {
    RemoteVisual {
        y_offset: visual_spec(kind).y_offset,
        name_y_offset: visual_spec(kind).name_y_offset,
    }
}

fn visual_spec(kind: NetEntityKind) -> VisualSpec {
    match kind {
        NetEntityKind::Player => VisualSpec {
            mesh: VisualMesh::Capsule,
            scale: PLAYER_SCALE,
            y_offset: PLAYER_Y_OFFSET,
            name_y_offset: PLAYER_NAME_Y_OFFSET,
            color: Color::srgb(0.85, 0.78, 0.72),
        },
        NetEntityKind::Mob(mob) => VisualSpec {
            mesh: VisualMesh::Capsule,
            scale: if mob_uses_biped_model(mob) {
                Vec3::ONE
            } else {
                MOB_SCALE
            },
            y_offset: if mob_uses_biped_model(mob) {
                0.0
            } else {
                MOB_Y_OFFSET
            },
            name_y_offset: if mob_uses_biped_model(mob) {
                2.05
            } else {
                MOB_NAME_Y_OFFSET
            },
            color: mob_color(mob),
        },
        NetEntityKind::Item => VisualSpec {
            mesh: VisualMesh::Sphere,
            scale: ITEM_SCALE,
            y_offset: ITEM_Y_OFFSET,
            name_y_offset: ITEM_NAME_Y_OFFSET,
            color: Color::srgb(0.95, 0.85, 0.20),
        },
        NetEntityKind::ExperienceOrb => VisualSpec {
            mesh: VisualMesh::Sphere,
            scale: ORB_SCALE,
            y_offset: ORB_Y_OFFSET,
            name_y_offset: ORB_NAME_Y_OFFSET,
            color: Color::srgb(0.15, 0.95, 0.20),
        },
        NetEntityKind::Object(kind) => VisualSpec {
            mesh: VisualMesh::Sphere,
            scale: OBJECT_SCALE,
            y_offset: OBJECT_Y_OFFSET,
            name_y_offset: OBJECT_NAME_Y_OFFSET,
            color: object_color(kind),
        },
    }
}

fn mob_color(kind: MobKind) -> Color {
    match kind {
        MobKind::Zombie | MobKind::PigZombie => Color::srgb(0.25, 0.73, 0.25),
        MobKind::Skeleton | MobKind::Wither => Color::srgb(0.86, 0.86, 0.86),
        MobKind::Creeper => Color::srgb(0.10, 0.78, 0.12),
        MobKind::Spider | MobKind::CaveSpider | MobKind::Endermite => Color::srgb(0.22, 0.22, 0.22),
        MobKind::Enderman => Color::srgb(0.20, 0.10, 0.28),
        MobKind::Blaze | MobKind::MagmaCube | MobKind::Ghast => Color::srgb(0.92, 0.45, 0.12),
        MobKind::Pig
        | MobKind::Sheep
        | MobKind::Cow
        | MobKind::Chicken
        | MobKind::Squid
        | MobKind::Wolf
        | MobKind::Mooshroom
        | MobKind::SnowGolem
        | MobKind::Ocelot
        | MobKind::Horse
        | MobKind::Rabbit
        | MobKind::Villager
        | MobKind::IronGolem => Color::srgb(0.30, 0.55, 0.88),
        MobKind::Unknown(_)
        | MobKind::Slime
        | MobKind::Giant
        | MobKind::Silverfish
        | MobKind::EnderDragon
        | MobKind::Bat
        | MobKind::Witch
        | MobKind::Guardian => Color::srgb(0.72, 0.35, 0.85),
    }
}

fn object_color(kind: ObjectKind) -> Color {
    match kind {
        ObjectKind::Arrow
        | ObjectKind::Snowball
        | ObjectKind::Egg
        | ObjectKind::EnderPearl
        | ObjectKind::EnderEye => Color::srgb(0.72, 0.72, 0.72),
        ObjectKind::PrimedTnt
        | ObjectKind::LargeFireball
        | ObjectKind::SmallFireball
        | ObjectKind::WitherSkull => Color::srgb(0.90, 0.25, 0.18),
        ObjectKind::Minecart
        | ObjectKind::Boat
        | ObjectKind::ArmorStand
        | ObjectKind::ItemFrame
        | ObjectKind::LeashKnot
        | ObjectKind::FishingHook => Color::srgb(0.72, 0.56, 0.35),
        ObjectKind::FallingBlock
        | ObjectKind::Firework
        | ObjectKind::ExpBottle
        | ObjectKind::SplashPotion
        | ObjectKind::EndCrystal
        | ObjectKind::Unknown(_) => Color::srgb(0.45, 0.74, 0.88),
    }
}

fn kind_label(kind: NetEntityKind) -> &'static str {
    match kind {
        NetEntityKind::Player => "Player",
        NetEntityKind::Item => "Dropped Item",
        NetEntityKind::ExperienceOrb => "XP Orb",
        NetEntityKind::Mob(mob) => mob_label(mob),
        NetEntityKind::Object(object) => object_label(object),
    }
}

fn mob_label(kind: MobKind) -> &'static str {
    match kind {
        MobKind::Creeper => "Creeper",
        MobKind::Skeleton => "Skeleton",
        MobKind::Spider => "Spider",
        MobKind::Giant => "Giant",
        MobKind::Zombie => "Zombie",
        MobKind::Slime => "Slime",
        MobKind::Ghast => "Ghast",
        MobKind::PigZombie => "Zombie Pigman",
        MobKind::Enderman => "Enderman",
        MobKind::CaveSpider => "Cave Spider",
        MobKind::Silverfish => "Silverfish",
        MobKind::Blaze => "Blaze",
        MobKind::MagmaCube => "Magma Cube",
        MobKind::EnderDragon => "Ender Dragon",
        MobKind::Wither => "Wither",
        MobKind::Bat => "Bat",
        MobKind::Witch => "Witch",
        MobKind::Endermite => "Endermite",
        MobKind::Guardian => "Guardian",
        MobKind::Pig => "Pig",
        MobKind::Sheep => "Sheep",
        MobKind::Cow => "Cow",
        MobKind::Chicken => "Chicken",
        MobKind::Squid => "Squid",
        MobKind::Wolf => "Wolf",
        MobKind::Mooshroom => "Mooshroom",
        MobKind::SnowGolem => "Snow Golem",
        MobKind::Ocelot => "Ocelot",
        MobKind::IronGolem => "Iron Golem",
        MobKind::Horse => "Horse",
        MobKind::Rabbit => "Rabbit",
        MobKind::Villager => "Villager",
        MobKind::Unknown(_) => "Mob",
    }
}

fn object_label(kind: ObjectKind) -> &'static str {
    match kind {
        ObjectKind::Boat => "Boat",
        ObjectKind::Minecart => "Minecart",
        ObjectKind::Arrow => "Arrow",
        ObjectKind::Snowball => "Snowball",
        ObjectKind::ItemFrame => "Item Frame",
        ObjectKind::LeashKnot => "Leash Knot",
        ObjectKind::EnderPearl => "Ender Pearl",
        ObjectKind::EnderEye => "Ender Eye",
        ObjectKind::Firework => "Firework",
        ObjectKind::LargeFireball => "Fireball",
        ObjectKind::SmallFireball => "Small Fireball",
        ObjectKind::WitherSkull => "Wither Skull",
        ObjectKind::Egg => "Egg",
        ObjectKind::SplashPotion => "Splash Potion",
        ObjectKind::ExpBottle => "XP Bottle",
        ObjectKind::FishingHook => "Fishing Hook",
        ObjectKind::PrimedTnt => "Primed TNT",
        ObjectKind::ArmorStand => "Armor Stand",
        ObjectKind::EndCrystal => "End Crystal",
        ObjectKind::FallingBlock => "Falling Block",
        ObjectKind::Unknown(_) => "Object",
    }
}

fn mob_uses_biped_model(mob: MobKind) -> bool {
    matches!(
        mob,
        MobKind::Zombie | MobKind::Skeleton | MobKind::PigZombie
    )
}

fn mob_texture_path(mob: MobKind) -> Option<&'static str> {
    Some(match mob {
        MobKind::Zombie => "entity/zombie/zombie.png",
        MobKind::Skeleton => "entity/skeleton/skeleton.png",
        MobKind::PigZombie => "entity/zombie_pigman.png",
        _ => return None,
    })
}

fn mob_biped_model(mob: MobKind) -> &'static crate::entity_model::ModelDef {
    // Vanilla uses mixed 64x32 and 64x64 biped textures in 1.8.9.
    // If we normalize using the wrong height, only the top portion (often the head) will sample correctly.
    match mob {
        MobKind::Skeleton => &BIPED_MODEL_TEX32,
        MobKind::Zombie | MobKind::PigZombie => &BIPED_MODEL_TEX64,
        _ => &BIPED_MODEL_TEX32,
    }
}
