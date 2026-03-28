use super::*;

pub(super) fn handle_packet(
    pkt: Packet,
    to_main: &crossbeam::channel::Sender<FromNetMessage>,
    conn: &mut Conn,
    requested_view_distance: u8,
) {
    match pkt {
        Packet::JoinGame_i8(jg) => {
            log_join_game(
                jg.entity_id,
                jg.gamemode,
                Some(i32::from(jg.dimension)),
                Some(jg.difficulty),
                jg.max_players,
                Some(&jg.level_type),
                None,
                Some(jg.reduced_debug_info),
                None,
                requested_view_distance,
            );
            send_join_game(to_main, conn, jg.entity_id, jg.gamemode, requested_view_distance);
        }
        Packet::JoinGame_i8_NoDebug(jg) => {
            log_join_game(
                jg.entity_id,
                jg.gamemode,
                Some(i32::from(jg.dimension)),
                Some(jg.difficulty),
                jg.max_players,
                Some(&jg.level_type),
                None,
                None,
                None,
                requested_view_distance,
            );
            send_join_game(to_main, conn, jg.entity_id, jg.gamemode, requested_view_distance);
        }
        Packet::JoinGame_i32(jg) => {
            log_join_game(
                jg.entity_id,
                jg.gamemode,
                Some(jg.dimension),
                Some(jg.difficulty),
                jg.max_players,
                Some(&jg.level_type),
                None,
                Some(jg.reduced_debug_info),
                None,
                requested_view_distance,
            );
            send_join_game(to_main, conn, jg.entity_id, jg.gamemode, requested_view_distance);
        }
        Packet::JoinGame_i32_ViewDistance(jg) => {
            log_join_game(
                jg.entity_id,
                jg.gamemode,
                Some(jg.dimension),
                None,
                jg.max_players,
                Some(&jg.level_type),
                Some(jg.view_distance.0),
                Some(jg.reduced_debug_info),
                None,
                requested_view_distance,
            );
            send_join_game(to_main, conn, jg.entity_id, jg.gamemode, requested_view_distance);
        }
        Packet::KeepAliveClientbound_VarInt(ka) => {
            conn.write_packet(
                rs_protocol::protocol::packet::play::serverbound::KeepAliveServerbound_VarInt {
                    id: ka.id,
                },
            )
            .unwrap();
        }
        Packet::UpdateHealth(health) => {
            let _ = to_main.send(FromNetMessage::UpdateHealth {
                health: health.health,
                food: health.food.0,
                food_saturation: health.food_saturation,
            });
        }
        Packet::UpdateHealth_u16(health) => {
            let _ = to_main.send(FromNetMessage::UpdateHealth {
                health: health.health,
                food: health.food as i32,
                food_saturation: health.food_saturation,
            });
        }
        Packet::SetExperience(exp) => {
            let _ = to_main.send(FromNetMessage::UpdateExperience {
                experience_bar: exp.experience_bar,
                level: exp.level.0,
                total_experience: exp.total_experience.0,
            });
        }
        Packet::SetExperience_i16(exp) => {
            let _ = to_main.send(FromNetMessage::UpdateExperience {
                experience_bar: exp.experience_bar,
                level: exp.level as i32,
                total_experience: exp.total_experience as i32,
            });
        }
        Packet::ChangeGameState(gs) => {
            if gs.reason == 3 {
                let mode = gs.value as i32;
                if (0..=u8::MAX as i32).contains(&mode) {
                    let _ = to_main.send(FromNetMessage::GameMode {
                        gamemode: mode as u8,
                    });
                }
            }
        }
        Packet::TimeUpdate(time_update) => {
            let _ = to_main.send(FromNetMessage::TimeUpdate {
                world_age: time_update.world_age,
                time_of_day: time_update.time_of_day,
            });
        }
        Packet::Respawn_Gamemode(respawn) => {
            info!(gamemode = respawn.gamemode, "Respawn");
            let _ = to_main.send(FromNetMessage::Respawn);
            let _ = to_main.send(FromNetMessage::GameMode {
                gamemode: respawn.gamemode,
            });
        }
        Packet::Respawn_HashedSeed(respawn) => {
            info!(
                gamemode = respawn.gamemode,
                dimension = respawn.dimension,
                hashed_seed = respawn.hashed_seed,
                level_type = %respawn.level_type,
                "Respawn"
            );
            let _ = to_main.send(FromNetMessage::Respawn);
            let _ = to_main.send(FromNetMessage::GameMode {
                gamemode: respawn.gamemode,
            });
        }
        Packet::Respawn_NBT(respawn) => {
            info!(
                gamemode = respawn.gamemode,
                world_name = %respawn.world_name,
                previous_gamemode = respawn.previous_gamemode,
                "Respawn"
            );
            let _ = to_main.send(FromNetMessage::Respawn);
            let _ = to_main.send(FromNetMessage::GameMode {
                gamemode: respawn.gamemode,
            });
        }
        Packet::Respawn_WorldName(respawn) => {
            info!(
                gamemode = respawn.gamemode,
                dimension = respawn.dimension,
                world_name = %respawn.world_name,
                hashed_seed = respawn.hashed_seed,
                "Respawn"
            );
            let _ = to_main.send(FromNetMessage::Respawn);
            let _ = to_main.send(FromNetMessage::GameMode {
                gamemode: respawn.gamemode,
            });
        }
        Packet::UpdateViewDistance(update) => {
            info!(
                server_view_distance = update.view_distance.0,
                "UpdateViewDistance"
            );
        }
        Packet::PlayerAbilities(abilities) => {
            let _ = to_main.send(FromNetMessage::PlayerAbilities {
                flags: abilities.flags,
                flying_speed: abilities.flying_speed,
                walking_speed: abilities.walking_speed,
            });
        }
        _ => {}
    }
}
