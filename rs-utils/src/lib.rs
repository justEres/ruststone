pub mod app;
pub mod assets;
pub mod chat;
pub mod entities;
pub mod inventory;
pub mod item_textures;
pub mod net_messages;
pub mod registry;
pub mod scoreboard;
pub mod sound;
pub mod world;

pub use app::{AppState, ApplicationState, UiState};
pub use assets::{
    RUSTSTONE_ASSETS_ROOT_ENV, ruststone_assets_root, sound_cache_minecraft_root,
    sound_cache_root, texturepack_minecraft_root, texturepack_textures_root,
};
pub use chat::{Chat, TitleMessage};
pub use entities::{
    MobKind, NetEntityAnimation, NetEntityKind, NetEntityMessage, ObjectKind, PlayerSkinModel,
};
pub use inventory::{
    InventoryEnchantment, InventoryItemMeta, InventoryItemStack, InventoryMessage, InventoryState,
    InventoryWindowInfo, item_max_durability,
};
pub use item_textures::item_texture_candidates;
pub use net_messages::{AuthMode, EntityUseAction, FromNet, FromNetMessage, ToNet, ToNetMessage};
pub use registry::{
    BlockFace, BlockModelKind, TEXTUREPACK_BLOCKS_BASE, TEXTUREPACK_ITEMS_BASE, block_model_kind,
    block_name, block_registry_key, block_state_id, block_state_meta, block_texture_name,
    item_name, item_registry_key,
};
pub use scoreboard::{ScoreboardMessage, ScoreboardObjectiveState, ScoreboardState, ScoreboardTeamState};
pub use sound::{SoundCategory, SoundEvent, SoundEventQueue, SoundSettings, SoundStopScope};
pub use world::{
    BlockUpdate, BreakIndicator, ChunkData, ChunkSection, PerfTimings, PlayerPosition,
    PlayerStatus, TabListHeaderFooter, TitleOverlayState, TitleTimes, WorldTime,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scoreboard_sidebar_applies_team_prefix_and_suffix() {
        let mut scoreboard = ScoreboardState::default();
        scoreboard.set_objective(
            "bedwars".to_string(),
            "Bedwars".to_string(),
            Some("integer".to_string()),
        );
        scoreboard.set_display_slot(1, "bedwars".to_string());
        scoreboard.apply_team(
            "red".to_string(),
            0,
            Some("Red".to_string()),
            Some("§c[R] ".to_string()),
            Some(" §7*".to_string()),
            Some(vec!["Alice".to_string()]),
        );
        scoreboard.set_score("Alice".to_string(), "bedwars".to_string(), 12);

        let lines = scoreboard.sidebar_lines();
        assert_eq!(lines, vec![("§c[R] Alice §7*".to_string(), 12)]);
    }

    #[test]
    fn scoreboard_remove_objective_clears_sidebar_slot_and_scores() {
        let mut scoreboard = ScoreboardState::default();
        scoreboard.set_objective("bw".to_string(), "Bedwars".to_string(), None);
        scoreboard.set_display_slot(1, "bw".to_string());
        scoreboard.set_score("Alice".to_string(), "bw".to_string(), 5);

        scoreboard.remove_objective("bw");

        assert!(scoreboard.sidebar_objective().is_none());
        assert!(scoreboard.sidebar_lines().is_empty());
    }

    #[test]
    fn scoreboard_sidebar_filters_hidden_entries_and_keeps_highest_fifteen() {
        let mut scoreboard = ScoreboardState::default();
        scoreboard.set_objective("bw".to_string(), "Bedwars".to_string(), None);
        scoreboard.set_display_slot(1, "bw".to_string());

        scoreboard.set_score("#hidden".to_string(), "bw".to_string(), 999);
        for idx in 0..20 {
            scoreboard.set_score(format!("Player{idx:02}"), "bw".to_string(), idx);
        }

        let lines = scoreboard.sidebar_lines();
        assert_eq!(lines.len(), 15);
        assert_eq!(lines.first(), Some(&("Player05".to_string(), 5)));
        assert_eq!(lines.last(), Some(&("Player19".to_string(), 19)));
        assert!(lines.iter().all(|(name, _)| !name.starts_with('#')));
    }

    #[test]
    fn sound_settings_final_gain_respects_master_and_category() {
        let settings = SoundSettings {
            master: 0.5,
            block: 0.4,
            ..Default::default()
        };
        assert!((settings.final_gain(SoundCategory::Block, 0.75) - 0.15).abs() < 1e-6);
    }
}
