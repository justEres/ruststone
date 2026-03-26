use bevy::ecs::resource::Resource;

#[derive(Resource)]
pub struct AppState(pub ApplicationState);

#[derive(Debug, Clone, Copy)]
pub enum ApplicationState {
    Disconnected,
    Connecting,
    Connected,
}

#[derive(Resource, Default)]
pub struct UiState {
    pub chat_open: bool,
    pub paused: bool,
    pub inventory_open: bool,
    pub ui_hidden: bool,
}
