use super::*;
use crate::skins::skin_download_worker;

pub(crate) fn player_shadow_emissive_strength(player_shadow_opacity: f32) -> LinearRgba {
    // Separate curve from terrain shadows: this keeps skin colors readable without
    // requiring excessively low opacity values.
    let t = 1.0 - player_shadow_opacity.clamp(0.0, 1.0);
    let lift = t * 0.32;
    LinearRgba::rgb(lift, lift, lift)
}

pub(crate) fn entity_root_translation(kind: NetEntityKind, pos: Vec3, visual_y_offset: f32) -> Vec3 {
    if kind == NetEntityKind::Item {
        pos
    } else {
        pos + Vec3::Y * visual_y_offset
    }
}

#[derive(Component, Debug, Clone, Copy)]
pub struct RemoteItemSprite;

#[derive(Component, Debug, Clone)]
pub struct RemoteItemStackState(pub InventoryItemStack);

#[derive(Component, Debug, Clone)]
pub struct ItemSpriteStack(pub InventoryItemStack);

#[derive(Component, Debug, Clone, Copy, Default)]
pub struct ItemSpin(pub f32);

#[derive(Component, Debug, Clone, Copy)]
pub struct RemoteDroppedItemMotion {
    pub authoritative_translation: Vec3,
    pub render_translation: Vec3,
    pub estimated_velocity: Vec3,
    pub last_server_update_secs: f64,
    pub ground_contact: bool,
}

impl RemoteDroppedItemMotion {
    pub(crate) fn new(translation: Vec3, now_secs: f64) -> Self {
        Self {
            authoritative_translation: translation,
            render_translation: translation,
            estimated_velocity: Vec3::ZERO,
            last_server_update_secs: now_secs,
            ground_contact: false,
        }
    }
}

#[derive(Component, Debug, Clone, Copy, Default)]
pub struct RemoteDroppedItemCollect {
    pub collector_server_id: Option<i32>,
    pub progress_secs: f32,
}

#[derive(SystemParam)]
pub struct RemoteEntityApplyParams<'w, 's> {
    pub(crate) transform_query: Query<'w, 's, &'static mut Transform>,
    pub(crate) smoothing_query: Query<'w, 's, &'static mut RemoteMotionSmoothing>,
    pub(crate) item_motion_query: Query<'w, 's, &'static mut RemoteDroppedItemMotion>,
    pub(crate) item_stack_query: Query<'w, 's, &'static RemoteItemStackState>,
    pub(crate) entity_query: Query<'w, 's, (&'static mut RemoteEntity, &'static mut RemoteEntityLook)>,
    pub(crate) player_anim_query: Query<'w, 's, &'static mut RemotePlayerAnimation>,
    pub(crate) biped_anim_query: Query<'w, 's, &'static mut RemoteBipedAnimation>,
    pub(crate) name_query: Query<'w, 's, &'static mut RemoteEntityName>,
    pub(crate) visual_query: Query<'w, 's, &'static RemoteVisual>,
    pub(crate) player_parts_query: Query<'w, 's, &'static RemotePlayerModelParts, With<RemotePlayer>>,
    pub(crate) held_item_query: Query<'w, 's, &'static RemoteHeldItem>,
    pub(crate) armor_state_query: Query<'w, 's, &'static mut HumanoidArmorState>,
}

#[derive(Debug)]
pub(crate) struct SkinDownloadResult {
    pub(crate) skin_url: String,
    pub(crate) rgba: Vec<u8>,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

#[derive(Resource)]
pub struct RemoteSkinDownloader {
    pub(crate) request_tx: Sender<String>,
    pub(crate) result_rx: Receiver<SkinDownloadResult>,
    pub(crate) requested: HashSet<String>,
    pub(crate) loaded: HashMap<String, Handle<Image>>,
}

#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct PlayerTextureDebugSettings;

impl Default for RemoteSkinDownloader {
    fn default() -> Self {
        let (request_tx, request_rx) = unbounded::<String>();
        let (result_tx, result_rx) = unbounded::<SkinDownloadResult>();
        thread::spawn(move || skin_download_worker(request_rx, result_tx));
        Self {
            request_tx,
            result_rx,
            requested: HashSet::new(),
            loaded: HashMap::new(),
        }
    }
}

impl RemoteSkinDownloader {
    pub fn request(&mut self, skin_url: String) {
        if !self.requested.insert(skin_url.clone()) {
            return;
        }
        info!("queue skin fetch: {skin_url}");
        let _ = self.request_tx.send(skin_url);
    }

    pub fn skin_handle(&self, skin_url: &str) -> Option<Handle<Image>> {
        self.loaded.get(skin_url).cloned()
    }
}

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
    pub player_skin_url_by_uuid: HashMap<rs_protocol::protocol::UUID, String>,
    pub player_skin_model_by_uuid: HashMap<rs_protocol::protocol::UUID, PlayerSkinModel>,
    pub pending_labels: HashMap<i32, String>,
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
    pub head_yaw: f32,
}

#[derive(Component, Debug, Clone)]
pub struct RemoteEntityUuid(pub rs_protocol::protocol::UUID);

#[derive(Component, Debug, Clone)]
pub struct RemoteEntityName(pub String);

#[derive(Component)]
pub struct RemotePlayer;

#[derive(Component, Debug, Clone, Copy)]
pub struct RemotePlayerModelParts {
    pub head: Entity,
    pub body: Entity,
    pub arm_left: Entity,
    pub arm_right: Entity,
    pub leg_left: Entity,
    pub leg_right: Entity,
}

#[derive(Component, Debug, Clone)]
pub struct RemotePlayerSkinMaterials(pub Vec<Handle<StandardMaterial>>);

#[derive(Component, Debug, Clone, Copy)]
pub struct RemotePlayerAnimation {
    pub previous_pos: Vec3,
    pub walk_phase: f32,
    pub swing_progress: f32,
    pub hurt_progress: f32,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct RemotePlayerSkinModel(pub PlayerSkinModel);

#[derive(Component, Debug, Clone, Copy)]
pub struct RemoteVisual {
    pub y_offset: f32,
    pub name_y_offset: f32,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct RemoteHeldItem(pub Entity);

#[derive(Component, Debug, Clone, Copy, Default)]
pub struct RemotePoseState {
    pub sneaking: bool,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct LocalPlayerModel;

#[derive(Component, Debug, Clone, Copy)]
pub struct LocalPlayerModelParts {
    pub head: Entity,
    pub body: Entity,
    pub arm_left: Entity,
    pub arm_right: Entity,
    pub leg_left: Entity,
    pub leg_right: Entity,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct LocalPlayerAnimation {
    pub walk_phase: f32,
    pub swing_progress: f32,
    pub hurt_progress: f32,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct LocalPlayerSkinModel(pub PlayerSkinModel);

#[derive(Component, Debug, Clone)]
pub struct LocalPlayerSkinMaterial(pub Handle<StandardMaterial>);

#[derive(Component)]
pub struct FirstPersonViewModel;

#[derive(Component, Debug, Clone)]
pub struct FirstPersonViewModelParts {
    pub arm_right: Entity,
    pub skin_model: PlayerSkinModel,
    pub held_stack: Option<InventoryItemStack>,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct RemoteBipedModelParts {
    pub model_root: Entity,
    pub head: Entity,
    pub body: Entity,
    pub arm_right: Entity,
    pub arm_left: Entity,
    pub leg_right: Entity,
    pub leg_left: Entity,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct RemoteBipedAnimation {
    pub previous_pos: Vec3,
    pub limb_swing: f32,
    pub limb_swing_amount: f32,
    pub swing_progress: f32,
}

#[derive(Component, Debug, Clone)]
pub struct RemoteQuadrupedModelParts {
    pub model_root: Entity,
    pub head: Entity,
    pub body: Entity,
    pub leg_front_right: Entity,
    pub leg_front_left: Entity,
    pub leg_back_right: Entity,
    pub leg_back_left: Entity,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct RemoteQuadrupedAnimation {
    pub previous_pos: Vec3,
    pub limb_swing: f32,
    pub limb_swing_amount: f32,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct RemoteQuadrupedAnimTuning {
    pub body_pitch: f32,
    pub leg_swing_scale: f32,
}

#[derive(Component, Debug, Clone)]
pub struct RemoteSheepWoolLayer {
    pub mesh_entities: [Entity; 6],
    pub material: Handle<StandardMaterial>,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct RemoteSheepAppearance {
    pub fleece_color: u8,
    pub sheared: bool,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct RemoteMotionSmoothing {
    pub target_translation: Vec3,
    pub estimated_velocity: Vec3,
    pub last_server_update_secs: f64,
}

pub(crate) const DROPPED_ITEM_GRAVITY: f32 = -0.04;
pub(crate) const DROPPED_ITEM_DRAG_AIR: f32 = 0.98;
pub(crate) const DROPPED_ITEM_DRAG_GROUND: f32 = 0.58;
pub(crate) const DROPPED_ITEM_RESTITUTION: f32 = 0.12;
pub(crate) const DROPPED_ITEM_EXTRAPOLATE_MAX: f32 = 0.12;
pub(crate) const DROPPED_ITEM_COLLISION_RADIUS: f32 = 0.125;
pub(crate) const DROPPED_ITEM_COLLISION_HEIGHT_OFFSET: f32 = DROPPED_ITEM_RENDER_Y_OFFSET;
pub(crate) const DROPPED_ITEM_COLLECT_DURATION: f32 = 0.14;
pub(crate) const DROPPED_ITEM_FALLBACK_COLLECT_HEIGHT: f32 = 0.6;

impl RemoteMotionSmoothing {
    pub(crate) fn new(target_translation: Vec3, now_secs: f64) -> Self {
        Self {
            target_translation,
            estimated_velocity: Vec3::ZERO,
            last_server_update_secs: now_secs,
        }
    }
}
