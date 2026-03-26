use std::collections::HashMap;

use bevy::ecs::resource::Resource;

#[derive(Debug, Clone, Default)]
pub struct ScoreboardObjectiveState {
    pub display_name: String,
    pub render_type: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ScoreboardTeamState {
    pub display_name: String,
    pub prefix: String,
    pub suffix: String,
    pub players: Vec<String>,
}

#[derive(Resource, Debug, Clone, Default)]
pub struct ScoreboardState {
    pub objectives: HashMap<String, ScoreboardObjectiveState>,
    pub display_slots: HashMap<u8, String>,
    pub scores: HashMap<(String, String), i32>,
    pub teams: HashMap<String, ScoreboardTeamState>,
    pub player_teams: HashMap<String, String>,
}

impl ScoreboardState {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn set_display_slot(&mut self, position: u8, objective_name: String) {
        if objective_name.is_empty() {
            self.display_slots.remove(&position);
        } else {
            self.display_slots.insert(position, objective_name);
        }
    }

    pub fn remove_objective(&mut self, objective_name: &str) {
        self.objectives.remove(objective_name);
        self.display_slots
            .retain(|_, name| name.as_str() != objective_name);
        self.scores
            .retain(|(_, objective), _| objective.as_str() != objective_name);
    }

    pub fn set_objective(
        &mut self,
        objective_name: String,
        display_name: String,
        render_type: Option<String>,
    ) {
        self.objectives.insert(
            objective_name,
            ScoreboardObjectiveState {
                display_name,
                render_type,
            },
        );
    }

    pub fn set_score(&mut self, entry_name: String, objective_name: String, value: i32) {
        self.scores.insert((entry_name, objective_name), value);
    }

    pub fn remove_score(&mut self, entry_name: &str, objective_name: &str) {
        self.scores
            .remove(&(entry_name.to_string(), objective_name.to_string()));
    }

    pub fn apply_team(
        &mut self,
        team_name: String,
        mode: u8,
        display_name: Option<String>,
        prefix: Option<String>,
        suffix: Option<String>,
        players: Option<Vec<String>>,
    ) {
        match mode {
            0 => {
                let players = players.unwrap_or_default();
                self.detach_players(&players);
                for player in &players {
                    self.player_teams.insert(player.clone(), team_name.clone());
                }
                self.teams.insert(
                    team_name,
                    ScoreboardTeamState {
                        display_name: display_name.unwrap_or_default(),
                        prefix: prefix.unwrap_or_default(),
                        suffix: suffix.unwrap_or_default(),
                        players,
                    },
                );
            }
            1 => {
                if let Some(team) = self.teams.remove(&team_name) {
                    for player in team.players {
                        self.player_teams.remove(&player);
                    }
                }
            }
            2 => {
                let team = self.teams.entry(team_name).or_default();
                if let Some(display_name) = display_name {
                    team.display_name = display_name;
                }
                if let Some(prefix) = prefix {
                    team.prefix = prefix;
                }
                if let Some(suffix) = suffix {
                    team.suffix = suffix;
                }
            }
            3 => {
                let players = players.unwrap_or_default();
                self.detach_players(&players);
                let team = self.teams.entry(team_name.clone()).or_default();
                for player in players {
                    if !team.players.iter().any(|existing| existing == &player) {
                        team.players.push(player.clone());
                    }
                    self.player_teams.insert(player, team_name.clone());
                }
            }
            4 => {
                if let Some(team) = self.teams.get_mut(&team_name) {
                    for player in players.unwrap_or_default() {
                        team.players.retain(|existing| existing != &player);
                        if self
                            .player_teams
                            .get(&player)
                            .is_some_and(|mapped_team| mapped_team == &team_name)
                        {
                            self.player_teams.remove(&player);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    pub fn sidebar_objective(&self) -> Option<(&str, &ScoreboardObjectiveState)> {
        let name = self.display_slots.get(&1)?;
        let objective = self.objectives.get(name)?;
        Some((name.as_str(), objective))
    }

    pub fn sidebar_lines(&self) -> Vec<(String, i32)> {
        let Some((objective_name, _)) = self.sidebar_objective() else {
            return Vec::new();
        };

        let mut lines: Vec<(String, i32)> = self
            .scores
            .iter()
            .filter(|((entry, objective), _)| objective == objective_name && !entry.starts_with('#'))
            .map(|((entry, _), value)| (self.format_entry(entry), *value))
            .collect();
        lines.sort_by(|(name_a, value_a), (name_b, value_b)| {
            value_a.cmp(value_b).then_with(|| name_b.cmp(name_a))
        });
        if lines.len() > 15 {
            lines = lines.split_off(lines.len() - 15);
        }
        lines
    }

    fn format_entry(&self, entry_name: &str) -> String {
        let Some(team_name) = self.player_teams.get(entry_name) else {
            return entry_name.to_string();
        };
        let Some(team) = self.teams.get(team_name) else {
            return entry_name.to_string();
        };
        format!("{}{}{}", team.prefix, entry_name, team.suffix)
    }

    fn detach_players(&mut self, players: &[String]) {
        for player in players {
            if let Some(previous_team) = self.player_teams.remove(player)
                && let Some(team) = self.teams.get_mut(&previous_team)
            {
                team.players.retain(|existing| existing != player);
            }
        }
    }
}

pub enum ScoreboardMessage {
    Display {
        position: u8,
        objective_name: String,
    },
    Objective {
        name: String,
        mode: Option<u8>,
        display_name: String,
        render_type: Option<String>,
    },
    UpdateScore {
        entry_name: String,
        action: u8,
        objective_name: String,
        value: Option<i32>,
    },
    Team {
        name: String,
        mode: u8,
        display_name: Option<String>,
        prefix: Option<String>,
        suffix: Option<String>,
        players: Option<Vec<String>>,
    },
}
