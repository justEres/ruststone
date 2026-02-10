use std::collections::{HashMap, VecDeque};

use crate::sim::collision::{WorldCollisionMap, is_solid};
use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};
use rs_render::PlayerCamera;
use rs_utils::{AppState, ApplicationState, MobKind, NetEntityKind, NetEntityMessage, ObjectKind};

const PLAYER_SCALE: Vec3 = Vec3::new(0.55, 0.9, 0.55);
const PLAYER_Y_OFFSET: f32 = 0.9;
const PLAYER_NAME_Y_OFFSET: f32 = 1.35;

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
}

#[derive(Component, Debug, Clone)]
pub struct RemoteEntityUuid(pub rs_protocol::protocol::UUID);

#[derive(Component, Debug, Clone)]
pub struct RemoteEntityName(pub String);

#[derive(Component)]
pub struct RemotePlayer;

#[derive(Component, Debug, Clone, Copy)]
pub struct RemoteVisual {
    pub y_offset: f32,
    pub name_y_offset: f32,
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
}

pub fn apply_remote_entity_events(
    mut commands: Commands,
    mut queue: ResMut<RemoteEntityEventQueue>,
    mut registry: ResMut<RemoteEntityRegistry>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut transform_query: Query<(
        &mut Transform,
        &mut Mesh3d,
        &mut MeshMaterial3d<StandardMaterial>,
    )>,
    mut entity_query: Query<(&mut RemoteEntity, &mut RemoteEntityLook)>,
    mut name_query: Query<&mut RemoteEntityName>,
    visual_query: Query<&RemoteVisual>,
) {
    for event in queue.drain() {
        match event {
            NetEntityMessage::LocalPlayerId { entity_id } => {
                registry.local_entity_id = Some(entity_id);
                if let Some(entity) = registry.by_server_id.remove(&entity_id) {
                    commands.entity(entity).despawn_recursive();
                    registry
                        .player_entity_by_uuid
                        .retain(|_, id| *id != entity_id);
                }
            }
            NetEntityMessage::PlayerInfoAdd { uuid, name } => {
                registry
                    .player_name_by_uuid
                    .insert(uuid.clone(), name.clone());
                if let Some(server_id) = registry.player_entity_by_uuid.get(&uuid).copied()
                    && let Some(entity) = registry.by_server_id.get(&server_id).copied()
                    && let Ok(mut entity_name) = name_query.get_mut(entity)
                {
                    entity_name.0 = name;
                }
            }
            NetEntityMessage::PlayerInfoRemove { uuid } => {
                registry.player_name_by_uuid.remove(&uuid);
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

                let spec = visual_spec(kind);
                let visual = visual_for_kind(kind);
                let display_name = if kind == NetEntityKind::Player {
                    uuid.as_ref()
                        .and_then(|id| registry.player_name_by_uuid.get(id))
                        .cloned()
                        .unwrap_or_else(|| format!("Player {}", entity_id))
                } else {
                    kind_label(kind).to_string()
                };

                if let Some(existing) = registry.by_server_id.get(&entity_id).copied() {
                    if let Ok((mut transform, mut mesh3d, mut material3d)) =
                        transform_query.get_mut(existing)
                    {
                        transform.translation = pos + Vec3::Y * visual.y_offset;
                        transform.rotation = Quat::from_axis_angle(Vec3::Y, yaw);
                        transform.scale = spec.scale;
                        *mesh3d = Mesh3d(match spec.mesh {
                            VisualMesh::Capsule => meshes.add(Capsule3d::default()),
                            VisualMesh::Sphere => meshes.add(Sphere::default()),
                        });
                        *material3d = MeshMaterial3d(materials.add(StandardMaterial {
                            base_color: spec.color,
                            perceptual_roughness: 0.95,
                            metallic: 0.0,
                            ..Default::default()
                        }));
                    }
                    if let Ok((mut remote_entity, mut look)) = entity_query.get_mut(existing) {
                        remote_entity.kind = kind;
                        remote_entity.on_ground = on_ground.unwrap_or(remote_entity.on_ground);
                        look.yaw = yaw;
                        look.pitch = pitch;
                    }
                    commands.entity(existing).insert(visual);
                    if kind == NetEntityKind::Player {
                        commands.entity(existing).insert(RemotePlayer);
                    } else {
                        commands.entity(existing).remove::<RemotePlayer>();
                    }
                    if let Ok(mut name_comp) = name_query.get_mut(existing) {
                        name_comp.0 = display_name;
                    }
                    continue;
                }

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
                let mut spawn_cmd = commands.spawn((
                    Name::new(format!("RemoteEntity[{entity_id}]")),
                    Mesh3d(mesh),
                    MeshMaterial3d(material),
                    Transform {
                        translation: pos + Vec3::Y * visual.y_offset,
                        rotation: Quat::from_axis_angle(Vec3::Y, yaw),
                        scale: spec.scale,
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
                    RemoteEntityLook { yaw, pitch },
                    RemoteEntityName(display_name),
                    visual,
                ));
                if kind == NetEntityKind::Player {
                    spawn_cmd.insert(RemotePlayer);
                }
                if let Some(uuid) = uuid {
                    registry
                        .player_entity_by_uuid
                        .insert(uuid.clone(), entity_id);
                    spawn_cmd.insert(RemoteEntityUuid(uuid));
                }

                registry.by_server_id.insert(entity_id, spawn_cmd.id());
            }
            NetEntityMessage::MoveDelta {
                entity_id,
                delta,
                on_ground,
            } => {
                if let Some(entity) = registry.by_server_id.get(&entity_id).copied() {
                    if let Ok((mut transform, _, _)) = transform_query.get_mut(entity) {
                        transform.translation += delta;
                    }
                    if let Ok((mut remote_entity, _)) = entity_query.get_mut(entity)
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
                    if let Ok((mut transform, _, _)) = transform_query.get_mut(entity) {
                        transform.rotation = Quat::from_axis_angle(Vec3::Y, yaw);
                    }
                    if let Ok((mut remote_entity, mut look)) = entity_query.get_mut(entity) {
                        look.yaw = yaw;
                        look.pitch = pitch;
                        if let Some(on_ground) = on_ground {
                            remote_entity.on_ground = on_ground;
                        }
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
                    if let Ok((mut transform, _, _)) = transform_query.get_mut(entity) {
                        let y_offset = visual_query
                            .get(entity)
                            .map_or(PLAYER_Y_OFFSET, |v| v.y_offset);
                        transform.translation = pos + Vec3::Y * y_offset;
                        transform.rotation = Quat::from_axis_angle(Vec3::Y, yaw);
                    }
                    if let Ok((mut remote_entity, mut look)) = entity_query.get_mut(entity) {
                        look.yaw = yaw;
                        look.pitch = pitch;
                        remote_entity.on_ground = on_ground.unwrap_or(remote_entity.on_ground);
                    }
                }
            }
            NetEntityMessage::Velocity { .. } => {}
            NetEntityMessage::Destroy { entity_ids } => {
                for entity_id in entity_ids {
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
            scale: MOB_SCALE,
            y_offset: MOB_Y_OFFSET,
            name_y_offset: MOB_NAME_Y_OFFSET,
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
