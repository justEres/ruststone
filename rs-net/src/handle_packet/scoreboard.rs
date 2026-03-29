use super::*;

pub(super) fn handle_packet(pkt: Packet, to_main: &crossbeam::channel::Sender<FromNetMessage>) {
    match pkt {
        Packet::ScoreboardDisplay(display) => {
            let _ = to_main.send(FromNetMessage::Scoreboard(ScoreboardMessage::Display {
                position: display.position,
                objective_name: display.name,
            }));
        }
        Packet::ScoreboardObjective(objective) => {
            let _ = to_main.send(FromNetMessage::Scoreboard(ScoreboardMessage::Objective {
                name: objective.name,
                mode: Some(objective.mode),
                display_name: objective.value,
                render_type: Some(objective.ty),
            }));
        }
        Packet::ScoreboardObjective_NoMode(objective) => {
            let _ = to_main.send(FromNetMessage::Scoreboard(ScoreboardMessage::Objective {
                name: objective.name,
                mode: None,
                display_name: objective.value,
                render_type: Some(objective.ty.to_string()),
            }));
        }
        Packet::UpdateScore(score) => {
            let _ = to_main.send(FromNetMessage::Scoreboard(ScoreboardMessage::UpdateScore {
                entry_name: score.name,
                action: score.action,
                objective_name: score.object_name,
                value: score.value.map(|value| value.0),
            }));
        }
        Packet::UpdateScore_i32(score) => {
            let _ = to_main.send(FromNetMessage::Scoreboard(ScoreboardMessage::UpdateScore {
                entry_name: score.name,
                action: score.action,
                objective_name: score.object_name,
                value: score.value,
            }));
        }
        Packet::Teams_u8(teams) => {
            send_team_packet(
                to_main,
                teams.name,
                teams.mode,
                teams.display_name,
                teams.prefix,
                teams.suffix,
                teams.players.map(|players| players.data),
            );
        }
        Packet::Teams_u8_NameTagVisibility(teams) => {
            send_team_packet(
                to_main,
                teams.name,
                teams.mode,
                teams.display_name,
                teams.prefix,
                teams.suffix,
                teams.players.map(|players| players.data),
            );
        }
        Packet::Teams_NoVisColor(teams) => {
            send_team_packet(
                to_main,
                teams.name,
                teams.mode,
                teams.display_name,
                teams.prefix,
                teams.suffix,
                teams.players.map(|players| players.data),
            );
        }
        Packet::Teams_VarInt(teams) => {
            send_team_packet(
                to_main,
                teams.name,
                teams.mode,
                teams.display_name,
                teams.prefix,
                teams.suffix,
                teams.players.map(|players| players.data),
            );
        }
        _ => {}
    }
}
