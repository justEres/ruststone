use super::super::*;
use rs_sound::{block_dig_sound, block_step_sound, emit_world_sound};
use rs_utils::{SoundCategory, SoundEventQueue};

pub fn world_interaction_system(
    time: Res<Time>,
    mouse: Res<ButtonInput<MouseButton>>,
    app_state: Res<AppState>,
    ui_state: Res<UiState>,
    player_status: Res<rs_utils::PlayerStatus>,
    to_net: Res<ToNet>,
    mut inventory_state: ResMut<InventoryState>,
    mut sound_queue: ResMut<SoundEventQueue>,
    sim_state: Res<SimState>,
    mut swing: ResMut<LocalArmSwing>,
    mut break_indicator: ResMut<BreakIndicator>,
    collision_map: Res<WorldCollisionMap>,
    camera_query: Query<&GlobalTransform, With<PlayerCamera>>,
    remote_entities: Query<(&GlobalTransform, &RemoteEntity, &RemoteVisual)>,
    mut mining: Local<MiningState>,
) {
    if !matches!(app_state.0, ApplicationState::Connected)
        || ui_state.chat_open
        || ui_state.paused
        || ui_state.inventory_open
        || player_status.dead
    {
        if mining.active {
            let _ = to_net.0.send(ToNetMessage::DigCancel {
                x: mining.target_block.x,
                y: mining.target_block.y,
                z: mining.target_block.z,
                face: mining.face,
            });
            *mining = MiningState::default();
        }
        *break_indicator = BreakIndicator::default();
        return;
    }

    let left_just_pressed = mouse.just_pressed(MouseButton::Left);
    let left_held = mouse.pressed(MouseButton::Left);
    let right_click = mouse.just_pressed(MouseButton::Right);
    if !left_held && !right_click {
        if mining.active {
            let _ = to_net.0.send(ToNetMessage::DigCancel {
                x: mining.target_block.x,
                y: mining.target_block.y,
                z: mining.target_block.z,
                face: mining.face,
            });
            *mining = MiningState::default();
        }
        *break_indicator = BreakIndicator::default();
        return;
    }

    let Ok(camera_transform) = camera_query.get_single() else {
        *break_indicator = BreakIndicator::default();
        return;
    };
    let origin = camera_transform.translation();
    let dir = *camera_transform.forward();
    let is_creative = player_status.gamemode == 1;
    let block_reach = if is_creative {
        CREATIVE_BLOCK_REACH
    } else {
        SURVIVAL_BLOCK_REACH
    };
    let entity_reach = if is_creative {
        CREATIVE_ENTITY_REACH
    } else {
        SURVIVAL_ENTITY_REACH
    };

    let block_hit = raycast_block(&collision_map, origin, dir, block_reach);
    let entity_hit = raycast_remote_entity(&remote_entities, origin, dir, entity_reach);

    if left_just_pressed {
        let _ = to_net.0.send(ToNetMessage::SwingArm);
        swing.progress = 0.0;
    }

    if left_held {
        if let Some(entity) = nearest_entity_hit(entity_hit, block_hit) {
            if left_just_pressed {
                let _ = to_net.0.send(ToNetMessage::UseEntity {
                    target_id: entity.entity_id,
                    action: EntityUseAction::Attack,
                });
            }
            if mining.active {
                let _ = to_net.0.send(ToNetMessage::DigCancel {
                    x: mining.target_block.x,
                    y: mining.target_block.y,
                    z: mining.target_block.z,
                    face: mining.face,
                });
                *mining = MiningState::default();
            }
            *break_indicator = BreakIndicator::default();
        } else if let Some(hit) = block_hit {
            let face = normal_to_face_index(hit.normal);
            let block_id = collision_map.block_at(hit.block.x, hit.block.y, hit.block.z);
            let held_item = inventory_state.hotbar_item(inventory_state.selected_hotbar_slot);
            let total_secs = estimate_break_time_secs(block_id, held_item);

            if !mining.active || mining.target_block != hit.block || mining.face != face {
                if mining.active {
                    let _ = to_net.0.send(ToNetMessage::DigCancel {
                        x: mining.target_block.x,
                        y: mining.target_block.y,
                        z: mining.target_block.z,
                        face: mining.face,
                    });
                }
                *mining = MiningState {
                    active: true,
                    target_block: hit.block,
                    face,
                    elapsed_secs: 0.0,
                    total_secs,
                    finish_sent: false,
                };
                let _ = to_net.0.send(ToNetMessage::DigStart {
                    x: hit.block.x,
                    y: hit.block.y,
                    z: hit.block.z,
                    face,
                });
                emit_world_sound(
                    &mut sound_queue,
                    block_dig_sound(block_state_id(block_id)),
                    Vec3::new(
                        hit.block.x as f32 + 0.5,
                        hit.block.y as f32 + 0.5,
                        hit.block.z as f32 + 0.5,
                    ),
                    0.7,
                    1.0,
                    Some(SoundCategory::Block),
                );
            } else {
                mining.elapsed_secs += time.delta_secs();
            }

            let progress = if mining.total_secs > 0.0 {
                (mining.elapsed_secs / mining.total_secs).clamp(0.0, 1.0)
            } else {
                1.0
            };
            *break_indicator = BreakIndicator {
                active: true,
                progress,
                elapsed_secs: mining.elapsed_secs,
                total_secs: mining.total_secs,
            };

            if !mining.finish_sent && progress >= 1.0 {
                let _ = to_net.0.send(ToNetMessage::DigFinish {
                    x: mining.target_block.x,
                    y: mining.target_block.y,
                    z: mining.target_block.z,
                    face: mining.face,
                });
                mining.finish_sent = true;
            }
        } else {
            if mining.active {
                let _ = to_net.0.send(ToNetMessage::DigCancel {
                    x: mining.target_block.x,
                    y: mining.target_block.y,
                    z: mining.target_block.z,
                    face: mining.face,
                });
            }
            *mining = MiningState::default();
            *break_indicator = BreakIndicator::default();
        }
    } else if mining.active {
        let _ = to_net.0.send(ToNetMessage::DigCancel {
            x: mining.target_block.x,
            y: mining.target_block.y,
            z: mining.target_block.z,
            face: mining.face,
        });
        *mining = MiningState::default();
        *break_indicator = BreakIndicator::default();
    }

    if right_click {
        if let Some(entity) = nearest_entity_hit(entity_hit, block_hit) {
            let _ = to_net.0.send(ToNetMessage::UseEntity {
                target_id: entity.entity_id,
                action: EntityUseAction::Interact,
            });
        } else if let Some(hit) = block_hit {
            let face = normal_to_face_index(hit.normal);
            let target_state = collision_map.block_at(hit.block.x, hit.block.y, hit.block.z);
            let target_id = block_state_id(target_state);

            if is_interactable_block(target_id) {
                let _ = to_net.0.send(ToNetMessage::PlaceBlock {
                    x: hit.block.x,
                    y: hit.block.y,
                    z: hit.block.z,
                    face: face as i8,
                    cursor_x: 8,
                    cursor_y: 8,
                    cursor_z: 8,
                });
                emit_world_sound(
                    &mut sound_queue,
                    "minecraft:random.click",
                    Vec3::new(
                        hit.block.x as f32 + 0.5,
                        hit.block.y as f32 + 0.5,
                        hit.block.z as f32 + 0.5,
                    ),
                    0.4,
                    1.0,
                    Some(SoundCategory::Block),
                );
                return;
            }

            let place_pos = hit.block + hit.normal;
            if placement_intersects_player(place_pos, sim_state.current.pos) {
                return;
            }
            let _ = to_net.0.send(ToNetMessage::PlaceBlock {
                x: hit.block.x,
                y: hit.block.y,
                z: hit.block.z,
                face: face as i8,
                cursor_x: 8,
                cursor_y: 8,
                cursor_z: 8,
            });
            emit_world_sound(
                &mut sound_queue,
                block_step_sound(target_id),
                Vec3::new(
                    place_pos.x as f32 + 0.5,
                    place_pos.y as f32 + 0.5,
                    place_pos.z as f32 + 0.5,
                ),
                0.8,
                0.9,
                Some(SoundCategory::Block),
            );
            if player_status.gamemode != 1 {
                let _ = inventory_state.predict_place_selected_hotbar();
            }
        } else {
            let held_item = inventory_state.selected_hotbar_item();
            let _ = to_net.0.send(ToNetMessage::UseItem { held_item });
        }
    }
}

fn is_interactable_block(block_id: u16) -> bool {
    matches!(
        block_id,
        // Containers
        23 | 54 | 61 | 62 | 84 | 130 | 146 | 154 | 158
            // Utility blocks
            | 58 | 116 | 117 | 118 | 120 | 145
            // Doors, trapdoors, fence gates
            | 64 | 71 | 96 | 107 | 167 | 183 | 184 | 185 | 186 | 187 | 193 | 194 | 195 | 196 | 197
            // Buttons/levers
            | 69 | 77 | 143
            // Note block
            | 25
    )
}

fn placement_intersects_player(block_pos: IVec3, player_feet: Vec3) -> bool {
    const PLAYER_HALF_WIDTH: f32 = 0.3;
    const PLAYER_HEIGHT: f32 = 1.8;
    let player_min = Vec3::new(
        player_feet.x - PLAYER_HALF_WIDTH,
        player_feet.y,
        player_feet.z - PLAYER_HALF_WIDTH,
    );
    let player_max = Vec3::new(
        player_feet.x + PLAYER_HALF_WIDTH,
        player_feet.y + PLAYER_HEIGHT,
        player_feet.z + PLAYER_HALF_WIDTH,
    );

    let block_min = Vec3::new(block_pos.x as f32, block_pos.y as f32, block_pos.z as f32);
    let block_max = block_min + Vec3::ONE;

    player_min.x < block_max.x
        && player_max.x > block_min.x
        && player_min.y < block_max.y
        && player_max.y > block_min.y
        && player_min.z < block_max.z
        && player_max.z > block_min.z
}

#[derive(Clone, Copy)]
pub(super) struct RayHit {
    pub(super) block: IVec3,
    pub(super) normal: IVec3,
    pub(super) distance: f32,
}

#[derive(Clone, Copy)]
struct EntityHit {
    entity_id: i32,
    distance: f32,
}

fn estimate_break_time_secs(block_id: u16, held_item: Option<rs_utils::InventoryItemStack>) -> f32 {
    let hardness = block_hardness(block_id);
    if hardness < 0.0 {
        return 9999.0;
    }
    if hardness == 0.0 {
        return 0.05;
    }
    let item_id = held_item.map(|stack| stack.item_id);
    let can_harvest = can_harvest_block(item_id, block_id);
    let speed = destroy_speed(item_id, block_id).max(0.1);
    let damage_per_tick = speed / hardness / if can_harvest { 30.0 } else { 100.0 };
    let ticks = (1.0 / damage_per_tick).ceil().max(1.0);
    (ticks / 20.0).clamp(0.05, 9999.0)
}

fn block_hardness(block_id: u16) -> f32 {
    match block_id {
        0 => 0.0,                                      // air
        1 | 4 => 2.0,                                  // stone, cobble
        2 => 0.6,                                      // grass
        3 => 0.5,                                      // dirt
        5 | 17 | 162 => 2.0,                           // planks/log
        12 => 0.5,                                     // sand
        13 => 0.6,                                     // gravel
        14 | 15 | 16 | 21 | 56 | 73 | 74 | 129 => 3.0, // ores
        18 | 161 => 0.2,                               // leaves
        20 => 0.3,                                     // glass
        24 | 45 => 0.8,                                // sandstone, brick
        49 => 50.0,                                    // obsidian
        50 => 0.0,                                     // torch
        54 => 2.5,                                     // chest
        58 => 2.5,                                     // crafting
        61 | 62 => 3.5,                                // furnace
        79 => 0.5,                                     // ice
        80 => 0.2,                                     // snow block
        81 => 0.4,                                     // cactus
        82 => 0.6,                                     // clay
        87 => 0.4,                                     // netherrack
        88 => 0.5,                                     // soulsand
        89 => 0.3,                                     // glowstone
        95 => 0.3,                                     // stained glass
        98 => 1.5,                                     // stone bricks
        155 => 0.8,                                    // quartz block
        159 => 0.8,                                    // stained hardened clay
        171 => 0.8,                                    // carpet/wool-ish break feel
        172 => 1.25,                                   // hardened clay
        173 => 5.0,                                    // coal block
        174 => 0.5,                                    // packed ice
        _ => 1.0,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ToolKind {
    Pickaxe,
    Shovel,
    Axe,
    Sword,
    Shears,
}

fn tool_kind(item_id: i32) -> Option<ToolKind> {
    match item_id {
        256 | 269 | 273 | 277 | 284 => Some(ToolKind::Shovel),
        257 | 270 | 274 | 278 | 285 => Some(ToolKind::Pickaxe),
        258 | 271 | 275 | 279 | 286 => Some(ToolKind::Axe),
        267 | 268 | 272 | 276 | 283 => Some(ToolKind::Sword),
        359 => Some(ToolKind::Shears),
        _ => None,
    }
}

fn tool_harvest_level(item_id: i32) -> i32 {
    match item_id {
        269 | 270 | 271 | 268 => 0, // wood / gold handled by speed
        273 | 274 | 275 | 272 => 1, // stone
        256..=258 | 267 => 2,       // iron
        277..=279 | 276 => 3,       // diamond
        284..=286 | 283 => 0,       // gold
        _ => -1,
    }
}

fn tool_speed(item_id: i32) -> f32 {
    match item_id {
        269..=271 | 268 => 2.0,
        273..=275 | 272 => 4.0,
        256..=258 | 267 => 6.0,
        277..=279 | 276 => 8.0,
        284..=286 | 283 => 12.0,
        359 => 5.0, // shears
        _ => 1.0,
    }
}

fn block_required_tool(block_id: u16) -> Option<(ToolKind, i32)> {
    match block_id {
        // Pickaxe, mostly stone/ore-like blocks.
        1 | 4 | 14 | 15 | 16 | 21 | 22 | 24 | 41 | 42 | 45 | 48 | 57 | 61 | 62 | 73 | 74 | 79
        | 80 | 98 | 101 | 109 | 112 | 121 | 133 | 152 | 155 | 172 | 173 => {
            Some((ToolKind::Pickaxe, 0))
        }
        // Higher harvest tiers.
        56 | 129 | 130 => Some((ToolKind::Pickaxe, 2)), // diamond/emerald/ender chest
        49 | 116 => Some((ToolKind::Pickaxe, 3)),       // obsidian/enchanting table
        // Shovel blocks.
        2 | 3 | 12 | 13 | 78 | 82 | 88 | 110 => Some((ToolKind::Shovel, 0)),
        // Axe blocks.
        5 | 17 | 47 | 53 | 54 | 58 | 64 | 96 | 107 | 134..=136 | 156 | 162 => {
            Some((ToolKind::Axe, 0))
        }
        _ => None,
    }
}

fn block_tool_bonus_kind(block_id: u16) -> Option<ToolKind> {
    match block_id {
        18 | 31 | 32 | 37 | 38 | 39 | 40 | 59 | 81 | 83 | 106 | 111 | 141 | 142 | 161 | 175 => {
            Some(ToolKind::Shears)
        }
        30 => Some(ToolKind::Sword), // cobweb
        _ => block_required_tool(block_id).map(|(kind, _)| kind),
    }
}

fn can_harvest_block(item_id: Option<i32>, block_id: u16) -> bool {
    let Some((required_kind, required_level)) = block_required_tool(block_id) else {
        return true;
    };
    let Some(item_id) = item_id else {
        return false;
    };
    if tool_kind(item_id) != Some(required_kind) {
        return false;
    }
    tool_harvest_level(item_id) >= required_level
}

fn destroy_speed(item_id: Option<i32>, block_id: u16) -> f32 {
    let Some(item_id) = item_id else {
        return 1.0;
    };
    let Some(kind) = tool_kind(item_id) else {
        return 1.0;
    };
    if block_tool_bonus_kind(block_id) == Some(kind) {
        tool_speed(item_id)
    } else {
        1.0
    }
}

fn nearest_entity_hit(
    entity_hit: Option<EntityHit>,
    block_hit: Option<RayHit>,
) -> Option<EntityHit> {
    match (entity_hit, block_hit) {
        (Some(entity), Some(block)) if entity.distance <= block.distance + 0.12 => Some(entity),
        (Some(_), Some(_)) => None,
        (Some(entity), None) => Some(entity),
        _ => None,
    }
}

pub(super) fn raycast_block(
    world: &WorldCollisionMap,
    origin: Vec3,
    direction: Vec3,
    max_distance: f32,
) -> Option<RayHit> {
    let dir = direction.normalize_or_zero();
    if dir.length_squared() == 0.0 {
        return None;
    }

    let mut prev_cell = origin.floor().as_ivec3();
    let step = 0.05f32;
    let mut t = step;
    while t <= max_distance {
        let point = origin + dir * t;
        let cell = point.floor().as_ivec3();
        if cell != prev_cell {
            let block_state = world.block_at(cell.x, cell.y, cell.z);
            let block_id = block_state_id(block_state);
            if block_id != 0 && !matches!(block_id, 8 | 9 | 10 | 11) {
                let normal = prev_cell - cell;
                let normal = IVec3::new(normal.x.signum(), normal.y.signum(), normal.z.signum());
                return Some(RayHit {
                    block: cell,
                    normal,
                    distance: t,
                });
            }
            prev_cell = cell;
        }
        t += step;
    }
    None
}

fn raycast_remote_entity(
    remote_entities: &Query<(&GlobalTransform, &RemoteEntity, &RemoteVisual)>,
    origin: Vec3,
    direction: Vec3,
    max_distance: f32,
) -> Option<EntityHit> {
    let dir = direction.normalize_or_zero();
    if dir.length_squared() == 0.0 {
        return None;
    }

    let mut nearest: Option<EntityHit> = None;
    for (transform, remote, visual) in remote_entities.iter() {
        let (half_w, height) = match remote.kind {
            rs_utils::NetEntityKind::Player | rs_utils::NetEntityKind::Mob(_) => (0.34, 1.8),
            rs_utils::NetEntityKind::Item => (0.22, 0.35),
            rs_utils::NetEntityKind::ExperienceOrb => (0.18, 0.28),
            rs_utils::NetEntityKind::Object(_) => (0.28, 0.56),
        };
        // Remote entity transforms are rendered with a y-offset; derive collider base from that.
        let feet = transform.translation() - Vec3::Y * visual.y_offset;
        let min = Vec3::new(feet.x - half_w, feet.y, feet.z - half_w);
        let max = Vec3::new(feet.x + half_w, feet.y + height, feet.z + half_w);
        let Some(distance) = ray_aabb_distance(origin, dir, min, max, max_distance) else {
            continue;
        };

        match nearest {
            Some(current) if current.distance <= distance => {}
            _ => {
                nearest = Some(EntityHit {
                    entity_id: remote.server_id,
                    distance,
                });
            }
        }
    }

    nearest
}

fn ray_aabb_distance(origin: Vec3, dir: Vec3, min: Vec3, max: Vec3, max_dist: f32) -> Option<f32> {
    let mut t_min = 0.0f32;
    let mut t_max = max_dist;

    for axis in 0..3 {
        let origin_axis = origin[axis];
        let dir_axis = dir[axis];
        let min_axis = min[axis];
        let max_axis = max[axis];

        if dir_axis.abs() < 1e-6 {
            if origin_axis < min_axis || origin_axis > max_axis {
                return None;
            }
            continue;
        }

        let inv_dir = 1.0 / dir_axis;
        let mut t1 = (min_axis - origin_axis) * inv_dir;
        let mut t2 = (max_axis - origin_axis) * inv_dir;
        if t1 > t2 {
            std::mem::swap(&mut t1, &mut t2);
        }
        t_min = t_min.max(t1);
        t_max = t_max.min(t2);
        if t_max < t_min {
            return None;
        }
    }

    if t_min >= 0.0 && t_min <= max_dist {
        Some(t_min)
    } else if t_max >= 0.0 && t_max <= max_dist {
        Some(t_max)
    } else {
        None
    }
}

fn normal_to_face_index(normal: IVec3) -> u8 {
    match normal {
        IVec3 { x: -1, y: 0, z: 0 } => 4, // east face clicked (normal points west)
        IVec3 { x: 1, y: 0, z: 0 } => 5,  // west face clicked
        IVec3 { x: 0, y: -1, z: 0 } => 0, // top face clicked
        IVec3 { x: 0, y: 1, z: 0 } => 1,  // bottom face clicked
        IVec3 { x: 0, y: 0, z: -1 } => 2, // south face clicked
        IVec3 { x: 0, y: 0, z: 1 } => 3,  // north face clicked
        _ => 1,
    }
}
