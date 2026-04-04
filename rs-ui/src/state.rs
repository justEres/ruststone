use super::*;
use crate::options_ui::SettingsCategoryState;
use crate::options_persistence::default_prism_accounts_path;

#[derive(Resource)]
pub struct ConnectUiState {
    pub username: String,
    pub server_address: String,
    pub auth_mode: AuthMode,
    pub prism_accounts_path: String,
    pub auth_accounts: Vec<UiAuthAccount>,
    pub selected_auth_account: usize,
    pub auth_accounts_loaded: bool,
    pub connect_feedback: String,
    pub vsync_enabled: bool,
    pub options_loaded: bool,
    pub options_dirty: bool,
    pub options_path: String,
    pub options_status: String,
    pub options_search: String,
    pub settings_category_state: SettingsCategoryState,
    pub chat_background_opacity: f32,
    pub chat_font_size: f32,
    pub scoreboard_background_opacity: f32,
    pub scoreboard_font_size: f32,
    pub title_background_opacity: f32,
    pub title_font_size: f32,
    pub debug_items_open: bool,
    pub debug_items_filter: String,
    pub debug_items: Vec<InventoryItemStack>,
    pub inventory_drag: Option<InventoryDragUiState>,
}
impl Default for ConnectUiState {
    fn default() -> Self {
        Self {
            username: "RustyPlayer".to_string(),
            server_address: "localhost:25565".to_string(),
            auth_mode: AuthMode::Authenticated,
            prism_accounts_path: default_prism_accounts_path(),
            auth_accounts: Vec::new(),
            selected_auth_account: 0,
            auth_accounts_loaded: false,
            connect_feedback: String::new(),
            vsync_enabled: false,
            options_loaded: false,
            options_dirty: false,
            options_path: DEFAULT_OPTIONS_PATH.to_string(),
            options_status: String::new(),
            options_search: String::new(),
            settings_category_state: SettingsCategoryState::default(),
            chat_background_opacity: 96.0,
            chat_font_size: 15.0,
            scoreboard_background_opacity: 112.0,
            scoreboard_font_size: 15.5,
            title_background_opacity: 80.0,
            title_font_size: 34.0,
            debug_items_open: false,
            debug_items_filter: String::new(),
            debug_items: Vec::new(),
            inventory_drag: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct InventoryDragUiState {
    pub window_id: u8,
    pub window_unique_slots: usize,
    pub button: u8,
    pub visited_slots: Vec<i16>,
}

#[derive(Resource, Default)]
pub struct ChatAutocompleteState {
    pub suggestions: Vec<String>,
    pub selected: usize,
    pub pending_query: Option<String>,
    pub query_snapshot: String,
    pub suppress_next_clear: bool,
}

impl ChatAutocompleteState {
    pub fn clear(&mut self) {
        self.suggestions.clear();
        self.selected = 0;
        self.pending_query = None;
    }
}

#[derive(Debug, Clone)]
pub struct UiAuthAccount {
    pub username: String,
    pub uuid: String,
    pub active: bool,
}
