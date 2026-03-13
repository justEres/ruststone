use std::collections::{HashMap, HashSet, VecDeque};
use std::thread;

mod entity_anim_spawn;
pub mod item_textures;
pub mod model;
mod player_mesh;
mod specs;

use rs_sim::collision::{WorldCollisionMap, is_solid};
use bevy::color::LinearRgba;
use bevy::ecs::system::SystemParam;
use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::mesh::Indices;
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::PrimitiveTopology;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::render::view::RenderLayers;
use bevy_egui::{EguiContexts, egui};
use crossbeam::channel::{Receiver, Sender, unbounded};
use rs_render::{
    CHUNK_CUTOUT_RENDER_LAYER, CHUNK_OPAQUE_RENDER_LAYER, CHUNK_TRANSPARENT_RENDER_LAYER,
    LOCAL_PLAYER_RENDER_LAYER, MAIN_RENDER_LAYER, PlayerCamera,
};
use rs_utils::{
    AppState, ApplicationState, InventoryItemStack, MobKind, NetEntityAnimation, NetEntityKind,
    NetEntityMessage, PlayerSkinModel, UiState,
};
use tracing::{debug, info, warn};

use crate::model::{
    BIPED_BODY, BIPED_HEAD, BIPED_LEFT_ARM, BIPED_LEFT_LEG, BIPED_MODEL_TEX32, BIPED_MODEL_TEX64,
    BIPED_RIGHT_ARM, BIPED_RIGHT_LEG, COW_MODEL_TEX32, CREEPER_MODEL_TEX64, EntityTextureCache,
    EntityTexturePath, PIG_MODEL_TEX32, QUADRUPED_BODY, QUADRUPED_HEAD, QUADRUPED_LEG_BACK_LEFT,
    QUADRUPED_LEG_BACK_RIGHT, QUADRUPED_LEG_FRONT_LEFT, QUADRUPED_LEG_FRONT_RIGHT,
    SHEEP_MODEL_TEX32, SHEEP_WOOL_MODEL_TEX32, part_mesh, spawn_model,
};

pub use crate::entity_anim_spawn::{
    animate_remote_biped_models, animate_remote_player_models, animate_remote_quadruped_models,
    rebuild_remote_player_meshes_on_texture_debug_change,
};
use crate::entity_anim_spawn::*;
use crate::item_textures::{ItemSpriteMesh, ItemTextureCache};
use crate::player_mesh::*;
use crate::specs::{
    BipedModelKind, QuadrupedModelKind, VisualMesh, kind_label, mob_biped_model_kind,
    mob_model_scale, mob_quadruped_anim_tuning, mob_quadruped_model_kind, mob_texture_path,
    mob_uses_biped_model, mob_uses_entity_model, mob_uses_quadruped_model, visual_spec,
};
use rs_sim::{CameraPerspectiveMode, CameraPerspectiveState, FreecamState, LocalArmSwing};
use rs_render::RenderDebugSettings;
use rs_render::{LookAngles, Player};
use rs_ui::ConnectUiState;

const SHEEP_WOOL_TEXTURE_PATH: &str = "entity/sheep/sheep_fur.png";

fn player_shadow_emissive_strength(player_shadow_opacity: f32) -> LinearRgba {
    // Separate curve from terrain shadows: this keeps skin colors readable without
    // requiring excessively low opacity values.
    let t = 1.0 - player_shadow_opacity.clamp(0.0, 1.0);
    let lift = t * 0.32;
    LinearRgba::rgb(lift, lift, lift)
}

#[derive(Component, Debug, Clone, Copy)]
pub struct RemoteItemSprite;

#[derive(Component, Debug, Clone)]
pub struct RemoteItemStackState(pub InventoryItemStack);

#[derive(Component, Debug, Clone)]
pub struct ItemSpriteStack(pub InventoryItemStack);

#[derive(Component, Debug, Clone, Copy, Default)]
pub struct ItemSpin(pub f32);

#[derive(Component, Debug, Clone, Copy)]
pub struct RemoteDroppedItemMotion {
    pub authoritative_translation: Vec3,
    pub render_translation: Vec3,
    pub estimated_velocity: Vec3,
    pub last_server_update_secs: f64,
    pub ground_contact: bool,
}

impl RemoteDroppedItemMotion {
    fn new(translation: Vec3, now_secs: f64) -> Self {
        Self {
            authoritative_translation: translation,
            render_translation: translation,
            estimated_velocity: Vec3::ZERO,
            last_server_update_secs: now_secs,
            ground_contact: false,
        }
    }
}

#[derive(Component, Debug, Clone, Copy, Default)]
pub struct RemoteDroppedItemCollect {
    pub collector_server_id: Option<i32>,
    pub progress_secs: f32,
}

#[derive(SystemParam)]
pub struct RemoteEntityApplyParams<'w, 's> {
    transform_query: Query<'w, 's, &'static mut Transform>,
    smoothing_query: Query<'w, 's, &'static mut RemoteMotionSmoothing>,
    item_motion_query: Query<'w, 's, &'static mut RemoteDroppedItemMotion>,
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
pub struct PlayerTextureDebugSettings;

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

#[derive(Component, Debug, Clone, Copy)]
pub struct LocalPlayerSkinModel(pub PlayerSkinModel);

#[derive(Component, Debug, Clone)]
pub struct LocalPlayerSkinMaterial(pub Handle<StandardMaterial>);

#[derive(Component)]
pub struct FirstPersonViewModel;

#[derive(Component, Debug, Clone)]
pub struct FirstPersonViewModelParts {
    pub arm_right: Entity,
    pub item: Entity,
    pub skin_model: PlayerSkinModel,
}

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

#[derive(Component, Debug, Clone)]
pub struct RemoteQuadrupedModelParts {
    pub model_root: Entity,
    pub head: Entity,
    pub body: Entity,
    pub leg_front_right: Entity,
    pub leg_front_left: Entity,
    pub leg_back_right: Entity,
    pub leg_back_left: Entity,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct RemoteQuadrupedAnimation {
    pub previous_pos: Vec3,
    pub limb_swing: f32,
    pub limb_swing_amount: f32,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct RemoteQuadrupedAnimTuning {
    pub body_pitch: f32,
    pub leg_swing_scale: f32,
}

#[derive(Component, Debug, Clone)]
pub struct RemoteSheepWoolLayer {
    pub mesh_entities: [Entity; 6],
    pub material: Handle<StandardMaterial>,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct RemoteSheepAppearance {
    pub fleece_color: u8,
    pub sheared: bool,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct RemoteMotionSmoothing {
    pub target_translation: Vec3,
    pub estimated_velocity: Vec3,
    pub last_server_update_secs: f64,
}

const DROPPED_ITEM_GRAVITY: f32 = -0.04;
const DROPPED_ITEM_DRAG_AIR: f32 = 0.98;
const DROPPED_ITEM_DRAG_GROUND: f32 = 0.58;
const DROPPED_ITEM_RESTITUTION: f32 = 0.12;
const DROPPED_ITEM_EXTRAPOLATE_MAX: f32 = 0.12;
const DROPPED_ITEM_COLLISION_RADIUS: f32 = 0.125;
const DROPPED_ITEM_COLLISION_HEIGHT_OFFSET: f32 = 0.17;
const DROPPED_ITEM_COLLECT_DURATION: f32 = 0.14;
const DROPPED_ITEM_FALLBACK_COLLECT_HEIGHT: f32 = 0.6;

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
                let quadruped_mob = match kind {
                    NetEntityKind::Mob(m) if mob_uses_quadruped_model(m) => Some(m),
                    _ => None,
                };
                let uses_model_mesh = biped_mob.is_some() || quadruped_mob.is_some();

                let spawn_cmd = commands.spawn((
                    Name::new(format!("RemoteEntity[{entity_id}]")),
                    Transform {
                        translation: pos + Vec3::Y * visual.y_offset,
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
                        .insert(RemoteMotionSmoothing::new(pos + Vec3::Y * visual.y_offset, now_secs));
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
                        debug!(entity_id, pos = ?pos, "spawned dropped item placeholder awaiting metadata");
                        commands.entity(root).insert((
                            Mesh3d(item_sprite_mesh.0.clone()),
                            MeshMaterial3d(material),
                            RemoteItemSprite,
                            ItemSpin::default(),
                            RemoteDroppedItemMotion::new(pos + Vec3::Y * visual.y_offset, now_secs),
                            Visibility::Hidden,
                        ));
                    } else if let Some(mob) = biped_mob {
                        commands
                            .entity(root)
                            .insert(RemoteMotionSmoothing::new(pos + Vec3::Y * visual.y_offset, now_secs));
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
                    } else if let Some(mob) = quadruped_mob {
                        commands
                            .entity(root)
                            .insert(RemoteMotionSmoothing::new(pos + Vec3::Y * visual.y_offset, now_secs));
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
                            .insert(RemoteMotionSmoothing::new(pos + Vec3::Y * visual.y_offset, now_secs));
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
                            commands_entity.remove::<RemoteItemStackState>();
                            commands_entity.remove::<ItemSpriteStack>();
                            commands_entity.insert(Visibility::Hidden);
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
                        let target = pos
                            + Vec3::Y
                                * params
                                    .visual_query
                                    .get(entity)
                                    .map_or(0.0, |v| v.y_offset);
                        if let Ok(mut item_motion) = params.item_motion_query.get_mut(entity) {
                            let previous = item_motion.authoritative_translation;
                            update_item_motion_velocity(&mut item_motion, previous, target, now_secs);
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

fn update_item_motion_velocity(
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

    let below = Vec3::new(next.x, next.y - DROPPED_ITEM_COLLISION_HEIGHT_OFFSET - 0.02, next.z);
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
    let age = ((now_secs - motion.last_server_update_secs) as f32).clamp(0.0, DROPPED_ITEM_EXTRAPOLATE_MAX);
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
            transform.scale = Vec3::splat(0.17 * (1.0 - alpha * 0.75));
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
        transform.scale = Vec3::splat(0.17);
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
    render_debug: Res<RenderDebugSettings>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    player_query: Query<(&RemoteEntityUuid, &RemotePlayerSkinMaterials), With<RemotePlayer>>,
) {
    let emissive = player_shadow_emissive_strength(render_debug.player_shadow_opacity);
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
                material.emissive_texture = Some(texture_handle.clone());
                material.alpha_mode = AlphaMode::Mask(0.5);
                material.unlit = false;
            }
            material.base_color = Color::WHITE;
            material.emissive = emissive;
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
    mut commands: Commands,
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

    let should_show = render_debug.render_self_model;
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
        commands
            .entity(e)
            .insert(RenderLayers::layer(LOCAL_PLAYER_RENDER_LAYER));
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
    if material.base_color_texture.as_ref() != Some(&desired) {
        material.base_color_texture = Some(desired.clone());
        material.emissive_texture = Some(desired);
        material.base_color = Color::WHITE;
    }
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

    // Head follows camera pitch; yaw comes from the player root rotation.
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
