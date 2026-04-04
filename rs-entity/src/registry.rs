use super::*;

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
