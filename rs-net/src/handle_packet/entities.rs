use super::*;

fn apply_attribute_modifiers(base: f64, modifiers: &[rs_protocol::protocol::packet::PropertyModifier]) -> f64 {
    let mut value = base;
    for modifier in modifiers.iter().filter(|modifier| modifier.operation == 0) {
        value += modifier.amount;
    }

    let mut scaled = value;
    for modifier in modifiers.iter().filter(|modifier| modifier.operation == 1) {
        scaled += value * modifier.amount;
    }

    for modifier in modifiers.iter().filter(|modifier| modifier.operation == 2) {
        scaled *= 1.0 + modifier.amount;
    }

    scaled
}

fn send_entity_attributes(
    to_main: &crossbeam::channel::Sender<FromNetMessage>,
    entity_id: i32,
    properties: &[rs_protocol::protocol::packet::EntityProperty],
) {
    let movement_speed = properties
        .iter()
        .find(|property| property.key == "generic.movementSpeed")
        .map(|property| apply_attribute_modifiers(property.value, &property.modifiers.data) as f32);
    let _ = to_main.send(FromNetMessage::EntityAttributes {
        entity_id,
        movement_speed,
    });
}

fn send_entity_attributes_i32(
    to_main: &crossbeam::channel::Sender<FromNetMessage>,
    entity_id: i32,
    properties: &[rs_protocol::protocol::packet::EntityProperty_i16],
) {
    let movement_speed = properties
        .iter()
        .find(|property| property.key == "generic.movementSpeed")
        .map(|property| apply_attribute_modifiers(property.value, &property.modifiers.data) as f32);
    let _ = to_main.send(FromNetMessage::EntityAttributes {
        entity_id,
        movement_speed,
    });
}

pub(super) fn handle_packet(pkt: Packet, to_main: &crossbeam::channel::Sender<FromNetMessage>) {
    match pkt {
        Packet::TeleportPlayer_NoConfirm(tp) => send_player_position(
            to_main,
            Some((tp.x, tp.y, tp.z)),
            Some(tp.yaw),
            Some(tp.pitch),
            Some(tp.flags),
            None,
        ),
        Packet::TeleportPlayer_OnGround(tp) => send_player_position(
            to_main,
            Some((tp.x, tp.eyes_y, tp.z)),
            Some(tp.yaw),
            Some(tp.pitch),
            None,
            Some(tp.on_ground),
        ),
        Packet::PlayerPosition(position) => send_player_position(
            to_main,
            Some((position.x, position.y, position.z)),
            None,
            None,
            None,
            Some(position.on_ground),
        ),
        Packet::PlayerPosition_HeadY(position) => send_player_position(
            to_main,
            Some((position.x, position.feet_y, position.z)),
            None,
            None,
            None,
            Some(position.on_ground),
        ),
        Packet::PlayerPositionLook(position) => send_player_position(
            to_main,
            Some((position.x, position.y, position.z)),
            Some(position.yaw),
            Some(position.pitch),
            None,
            Some(position.on_ground),
        ),
        Packet::PlayerPositionLook_HeadY(position) => send_player_position(
            to_main,
            Some((position.x, position.feet_y, position.z)),
            Some(position.yaw),
            Some(position.pitch),
            None,
            Some(position.on_ground),
        ),
        Packet::PlayerLook(position) => send_player_position(
            to_main,
            None,
            Some(position.yaw),
            Some(position.pitch),
            None,
            Some(position.on_ground),
        ),
        Packet::SpawnPlayer_i32_HeldItem(sp) => send_spawn_player(
            to_main,
            sp.entity_id.0,
            Some(sp.uuid),
            bevy::prelude::Vec3::new(
                f64::from(sp.x) as f32,
                f64::from(sp.y) as f32,
                f64::from(sp.z) as f32,
            ),
            sp.yaw,
            sp.pitch,
        ),
        Packet::SpawnPlayer_i32(sp) => send_spawn_player(
            to_main,
            sp.entity_id.0,
            Some(sp.uuid),
            bevy::prelude::Vec3::new(
                f64::from(sp.x) as f32,
                f64::from(sp.y) as f32,
                f64::from(sp.z) as f32,
            ),
            sp.yaw,
            sp.pitch,
        ),
        Packet::SpawnPlayer_f64(sp) => send_spawn_player(
            to_main,
            sp.entity_id.0,
            Some(sp.uuid),
            bevy::prelude::Vec3::new(sp.x as f32, sp.y as f32, sp.z as f32),
            sp.yaw,
            sp.pitch,
        ),
        Packet::SpawnPlayer_f64_NoMeta(sp) => send_spawn_player(
            to_main,
            sp.entity_id.0,
            Some(sp.uuid),
            bevy::prelude::Vec3::new(sp.x as f32, sp.y as f32, sp.z as f32),
            sp.yaw,
            sp.pitch,
        ),
        Packet::SpawnPlayer_i32_HeldItem_String(sp) => {
            let parsed_uuid = sp.uuid.parse::<rs_protocol::protocol::UUID>().ok();
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::Spawn {
                entity_id: sp.entity_id.0,
                uuid: parsed_uuid,
                kind: NetEntityKind::Player,
                pos: bevy::prelude::Vec3::new(
                    f64::from(sp.x) as f32,
                    f64::from(sp.y) as f32,
                    f64::from(sp.z) as f32,
                ),
                yaw: server_yaw_to_client_yaw(angle_i8_to_degrees(sp.yaw)),
                pitch: server_pitch_to_client_pitch(angle_i8_to_degrees(sp.pitch)),
                on_ground: None,
            }));
            if let Ok(uuid) = sp.uuid.parse::<rs_protocol::protocol::UUID>() {
                let (skin_url, skin_model) =
                    extract_skin_info_from_spawn_properties(&sp.properties.data);
                debug!(
                    "NET SpawnPlayer_i32_HeldItem_String name={} uuid={:?} props={} skin_url={:?} skin_model={:?}",
                    sp.name,
                    uuid,
                    sp.properties.data.len(),
                    skin_url,
                    skin_model
                );
                let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::PlayerInfoAdd {
                    uuid,
                    name: sp.name,
                    skin_url,
                    skin_model,
                }));
            }
        }
        Packet::EntityMetadata(em) => {
            handle_entity_metadata(em.entity_id.0, &em.metadata, to_main);
        }
        Packet::EntityMetadata_i32(em) => {
            handle_entity_metadata(em.entity_id, &em.metadata, to_main);
        }
        Packet::Animation(anim) => {
            let animation = match anim.animation_id {
                0 => NetEntityAnimation::SwingMainArm,
                1 => NetEntityAnimation::TakeDamage,
                2 => NetEntityAnimation::LeaveBed,
                other => NetEntityAnimation::Unknown(other),
            };
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::Animation {
                entity_id: anim.entity_id.0,
                animation,
            }));
        }
        Packet::EntityProperties(ep) => {
            send_entity_attributes(to_main, ep.entity_id.0, &ep.properties.data);
        }
        Packet::EntityProperties_i32(ep) => {
            send_entity_attributes_i32(to_main, ep.entity_id, &ep.properties.data);
        }
        Packet::SpawnObject_i32_NoUUID(so) => {
            if object_type_to_kind(so.ty) == NetEntityKind::Item {
                debug!(entity_id = so.entity_id.0, data = so.data, pos = ?(so.x, so.y, so.z), "spawned dropped item object before metadata");
            }
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::Spawn {
                entity_id: so.entity_id.0,
                uuid: None,
                kind: object_type_to_kind(so.ty),
                pos: bevy::prelude::Vec3::new(
                    f64::from(so.x) as f32,
                    f64::from(so.y) as f32,
                    f64::from(so.z) as f32,
                ),
                yaw: server_yaw_to_client_yaw(angle_i8_to_degrees(so.yaw)),
                pitch: server_pitch_to_client_pitch(angle_i8_to_degrees(so.pitch)),
                on_ground: None,
            }));
        }
        Packet::SpawnObject_i32(so) => {
            if object_type_to_kind(so.ty) == NetEntityKind::Item {
                debug!(entity_id = so.entity_id.0, data = so.data, pos = ?(so.x, so.y, so.z), "spawned dropped item object before metadata");
            }
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::Spawn {
                entity_id: so.entity_id.0,
                uuid: Some(so.uuid),
                kind: object_type_to_kind(so.ty),
                pos: bevy::prelude::Vec3::new(
                    f64::from(so.x) as f32,
                    f64::from(so.y) as f32,
                    f64::from(so.z) as f32,
                ),
                yaw: server_yaw_to_client_yaw(angle_i8_to_degrees(so.yaw)),
                pitch: server_pitch_to_client_pitch(angle_i8_to_degrees(so.pitch)),
                on_ground: None,
            }));
        }
        Packet::SpawnExperienceOrb_i32(xp) => {
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::Spawn {
                entity_id: xp.entity_id.0,
                uuid: None,
                kind: NetEntityKind::ExperienceOrb,
                pos: bevy::prelude::Vec3::new(
                    f64::from(xp.x) as f32,
                    f64::from(xp.y) as f32,
                    f64::from(xp.z) as f32,
                ),
                yaw: 0.0,
                pitch: 0.0,
                on_ground: None,
            }));
        }
        Packet::SpawnMob_u8_i32_NoUUID(sm) => {
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::Spawn {
                entity_id: sm.entity_id.0,
                uuid: None,
                kind: NetEntityKind::Mob(mob_type_to_kind(sm.ty)),
                pos: bevy::prelude::Vec3::new(
                    f64::from(sm.x) as f32,
                    f64::from(sm.y) as f32,
                    f64::from(sm.z) as f32,
                ),
                yaw: server_yaw_to_client_yaw(angle_i8_to_degrees(sm.yaw)),
                pitch: server_pitch_to_client_pitch(angle_i8_to_degrees(sm.pitch)),
                on_ground: None,
            }));
        }
        Packet::SpawnMob_u8_i32(sm) => {
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::Spawn {
                entity_id: sm.entity_id.0,
                uuid: Some(sm.uuid),
                kind: NetEntityKind::Mob(mob_type_to_kind(sm.ty)),
                pos: bevy::prelude::Vec3::new(
                    f64::from(sm.x) as f32,
                    f64::from(sm.y) as f32,
                    f64::from(sm.z) as f32,
                ),
                yaw: server_yaw_to_client_yaw(angle_i8_to_degrees(sm.yaw)),
                pitch: server_pitch_to_client_pitch(angle_i8_to_degrees(sm.pitch)),
                on_ground: None,
            }));
        }
        Packet::SpawnMob_u8(sm) => {
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::Spawn {
                entity_id: sm.entity_id.0,
                uuid: Some(sm.uuid),
                kind: NetEntityKind::Mob(mob_type_to_kind(sm.ty)),
                pos: bevy::prelude::Vec3::new(sm.x as f32, sm.y as f32, sm.z as f32),
                yaw: server_yaw_to_client_yaw(angle_i8_to_degrees(sm.yaw)),
                pitch: server_pitch_to_client_pitch(angle_i8_to_degrees(sm.pitch)),
                on_ground: None,
            }));
        }
        Packet::EntityHeadLook(ehl) => {
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::HeadLook {
                entity_id: ehl.entity_id.0,
                head_yaw: server_yaw_to_client_yaw(angle_i8_to_degrees(ehl.head_yaw)),
            }));
        }
        Packet::EntityHeadLook_i32(ehl) => {
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::HeadLook {
                entity_id: ehl.entity_id,
                head_yaw: server_yaw_to_client_yaw(angle_i8_to_degrees(ehl.head_yaw)),
            }));
        }
        Packet::EntityMove_i8(em) => {
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::MoveDelta {
                entity_id: em.entity_id.0,
                delta: bevy::prelude::Vec3::new(
                    f64::from(em.delta_x) as f32,
                    f64::from(em.delta_y) as f32,
                    f64::from(em.delta_z) as f32,
                ),
                on_ground: Some(em.on_ground),
            }));
        }
        Packet::EntityMove_i8_i32_NoGround(em) => {
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::MoveDelta {
                entity_id: em.entity_id,
                delta: bevy::prelude::Vec3::new(
                    f64::from(em.delta_x) as f32,
                    f64::from(em.delta_y) as f32,
                    f64::from(em.delta_z) as f32,
                ),
                on_ground: None,
            }));
        }
        Packet::EntityVelocity(ev) => {
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::Velocity {
                entity_id: ev.entity_id.0,
                velocity: bevy::prelude::Vec3::new(
                    ev.velocity_x as f32 / 8000.0,
                    ev.velocity_y as f32 / 8000.0,
                    ev.velocity_z as f32 / 8000.0,
                ),
            }));
        }
        Packet::EntityVelocity_i32(ev) => {
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::Velocity {
                entity_id: ev.entity_id,
                velocity: bevy::prelude::Vec3::new(
                    ev.velocity_x as f32 / 8000.0,
                    ev.velocity_y as f32 / 8000.0,
                    ev.velocity_z as f32 / 8000.0,
                ),
            }));
        }
        Packet::EntityTeleport_i32(et) => {
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::Teleport {
                entity_id: et.entity_id.0,
                pos: bevy::prelude::Vec3::new(
                    f64::from(et.x) as f32,
                    f64::from(et.y) as f32,
                    f64::from(et.z) as f32,
                ),
                yaw: server_yaw_to_client_yaw(angle_i8_to_degrees(et.yaw)),
                pitch: server_pitch_to_client_pitch(angle_i8_to_degrees(et.pitch)),
                on_ground: Some(et.on_ground),
            }));
        }
        Packet::EntityTeleport_i32_i32_NoGround(et) => {
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::Teleport {
                entity_id: et.entity_id,
                pos: bevy::prelude::Vec3::new(
                    f64::from(et.x) as f32,
                    f64::from(et.y) as f32,
                    f64::from(et.z) as f32,
                ),
                yaw: server_yaw_to_client_yaw(angle_i8_to_degrees(et.yaw)),
                pitch: server_pitch_to_client_pitch(angle_i8_to_degrees(et.pitch)),
                on_ground: None,
            }));
        }
        Packet::EntityEquipment_u16(ee) => {
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::Equipment {
                entity_id: ee.entity_id.0,
                slot: ee.slot,
                item: protocol_stack_to_inventory_item(ee.item),
            }));
        }
        Packet::EntityEquipment_u16_i32(ee) => {
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::Equipment {
                entity_id: ee.entity_id,
                slot: ee.slot,
                item: protocol_stack_to_inventory_item(ee.item),
            }));
        }
        Packet::EntityLookAndMove_i8(elm) => {
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::MoveDelta {
                entity_id: elm.entity_id.0,
                delta: bevy::prelude::Vec3::new(
                    f64::from(elm.delta_x) as f32,
                    f64::from(elm.delta_y) as f32,
                    f64::from(elm.delta_z) as f32,
                ),
                on_ground: Some(elm.on_ground),
            }));
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::Look {
                entity_id: elm.entity_id.0,
                yaw: server_yaw_to_client_yaw(angle_i8_to_degrees(elm.yaw)),
                pitch: server_pitch_to_client_pitch(angle_i8_to_degrees(elm.pitch)),
                on_ground: Some(elm.on_ground),
            }));
        }
        Packet::EntityLookAndMove_i8_i32_NoGround(elm) => {
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::MoveDelta {
                entity_id: elm.entity_id,
                delta: bevy::prelude::Vec3::new(
                    f64::from(elm.delta_x) as f32,
                    f64::from(elm.delta_y) as f32,
                    f64::from(elm.delta_z) as f32,
                ),
                on_ground: None,
            }));
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::Look {
                entity_id: elm.entity_id,
                yaw: server_yaw_to_client_yaw(angle_i8_to_degrees(elm.yaw)),
                pitch: server_pitch_to_client_pitch(angle_i8_to_degrees(elm.pitch)),
                on_ground: None,
            }));
        }
        Packet::EntityLook_VarInt(el) => {
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::Look {
                entity_id: el.entity_id.0,
                yaw: server_yaw_to_client_yaw(angle_i8_to_degrees(el.yaw)),
                pitch: server_pitch_to_client_pitch(angle_i8_to_degrees(el.pitch)),
                on_ground: Some(el.on_ground),
            }));
        }
        Packet::EntityLook_i32_NoGround(el) => {
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::Look {
                entity_id: el.entity_id,
                yaw: server_yaw_to_client_yaw(angle_i8_to_degrees(el.yaw)),
                pitch: server_pitch_to_client_pitch(angle_i8_to_degrees(el.pitch)),
                on_ground: None,
            }));
        }
        Packet::EntityDestroy(ed) => {
            let ids = ed.entity_ids.data.into_iter().map(|id| id.0).collect();
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::Destroy {
                entity_ids: ids,
            }));
        }
        Packet::EntityDestroy_u8(ed) => {
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::Destroy {
                entity_ids: ed.entity_ids.data,
            }));
        }
        Packet::EntityStatus(es) => {
            if es.entity_status == 2 {
                let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::Animation {
                    entity_id: es.entity_id,
                    animation: NetEntityAnimation::TakeDamage,
                }));
            }
        }
        Packet::CollectItem_nocount(ci) => {
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::CollectItem {
                collected_entity_id: ci.collected_entity_id.0,
                collector_entity_id: ci.collector_entity_id.0,
            }));
        }
        Packet::CollectItem_nocount_i32(ci) => {
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::CollectItem {
                collected_entity_id: ci.collected_entity_id,
                collector_entity_id: ci.collector_entity_id,
            }));
        }
        Packet::PlayerInfo(info) => {
            for detail in info.inner.players {
                match detail {
                    rs_protocol::protocol::packet::PlayerDetail::Add {
                        uuid,
                        name,
                        properties,
                        ..
                    } => {
                        let (skin_url, skin_model) =
                            extract_skin_info_from_player_properties(&properties);
                        debug!(
                            "NET PlayerInfo::Add name={} uuid={:?} props={} skin_url={:?} skin_model={:?}",
                            name,
                            uuid,
                            properties.len(),
                            skin_url,
                            skin_model
                        );
                        info!(
                            name,
                            uuid = ?uuid,
                            properties = properties.len(),
                            skin_url,
                            skin_model = ?skin_model,
                            "PlayerInfo::Add"
                        );
                        let _ = to_main.send(FromNetMessage::NetEntity(
                            NetEntityMessage::PlayerInfoAdd {
                                uuid,
                                name,
                                skin_url,
                                skin_model,
                            },
                        ));
                    }
                    rs_protocol::protocol::packet::PlayerDetail::Remove { uuid } => {
                        info!(uuid = ?uuid, "PlayerInfo::Remove");
                        let _ = to_main.send(FromNetMessage::NetEntity(
                            NetEntityMessage::PlayerInfoRemove { uuid },
                        ));
                    }
                    _ => {}
                }
            }
        }
        Packet::EntityEffect(effect) => {
            let _ = to_main.send(FromNetMessage::PotionEffect {
                entity_id: effect.entity_id.0,
                effect_id: effect.effect_id,
                amplifier: effect.amplifier,
                duration_ticks: effect.duration.0,
            });
        }
        Packet::EntityEffect_i32(effect) => {
            let _ = to_main.send(FromNetMessage::PotionEffect {
                entity_id: effect.entity_id,
                effect_id: effect.effect_id,
                amplifier: effect.amplifier,
                duration_ticks: i32::from(effect.duration),
            });
        }
        Packet::EntityRemoveEffect(remove) => {
            let _ = to_main.send(FromNetMessage::PotionEffectRemove {
                entity_id: remove.entity_id.0,
                effect_id: remove.effect_id,
            });
        }
        Packet::EntityRemoveEffect_i32(remove) => {
            let _ = to_main.send(FromNetMessage::PotionEffectRemove {
                entity_id: remove.entity_id,
                effect_id: remove.effect_id,
            });
        }
        _ => {}
    }
}
