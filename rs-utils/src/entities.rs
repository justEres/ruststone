use bevy::prelude::Vec3;
use rs_protocol::protocol::UUID;

use crate::inventory::InventoryItemStack;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NetEntityKind {
    Player,
    Item,
    ExperienceOrb,
    Mob(MobKind),
    Object(ObjectKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PlayerSkinModel {
    #[default]
    Classic,
    Slim,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MobKind {
    Creeper,
    Skeleton,
    Spider,
    Giant,
    Zombie,
    Slime,
    Ghast,
    PigZombie,
    Enderman,
    CaveSpider,
    Silverfish,
    Blaze,
    MagmaCube,
    EnderDragon,
    Wither,
    Bat,
    Witch,
    Endermite,
    Guardian,
    Pig,
    Sheep,
    Cow,
    Chicken,
    Squid,
    Wolf,
    Mooshroom,
    SnowGolem,
    Ocelot,
    IronGolem,
    Horse,
    Rabbit,
    Villager,
    Unknown(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ObjectKind {
    Boat,
    Minecart,
    Arrow,
    Snowball,
    ItemFrame,
    LeashKnot,
    EnderPearl,
    EnderEye,
    Firework,
    LargeFireball,
    SmallFireball,
    WitherSkull,
    Egg,
    SplashPotion,
    ExpBottle,
    FishingHook,
    PrimedTnt,
    ArmorStand,
    EndCrystal,
    FallingBlock,
    Unknown(u8),
}

#[derive(Debug, Clone)]
pub enum NetEntityMessage {
    LocalPlayerId {
        entity_id: i32,
    },
    PlayerInfoAdd {
        uuid: UUID,
        name: String,
        skin_url: Option<String>,
        skin_model: PlayerSkinModel,
    },
    PlayerInfoRemove {
        uuid: UUID,
    },
    Spawn {
        entity_id: i32,
        uuid: Option<UUID>,
        kind: NetEntityKind,
        pos: Vec3,
        yaw: f32,
        pitch: f32,
        on_ground: Option<bool>,
    },
    MoveDelta {
        entity_id: i32,
        delta: Vec3,
        on_ground: Option<bool>,
    },
    Look {
        entity_id: i32,
        yaw: f32,
        pitch: f32,
        on_ground: Option<bool>,
    },
    Teleport {
        entity_id: i32,
        pos: Vec3,
        yaw: f32,
        pitch: f32,
        on_ground: Option<bool>,
    },
    Velocity {
        entity_id: i32,
        velocity: Vec3,
    },
    Pose {
        entity_id: i32,
        sneaking: bool,
    },
    HeadLook {
        entity_id: i32,
        head_yaw: f32,
    },
    Equipment {
        entity_id: i32,
        slot: u16,
        item: Option<InventoryItemStack>,
    },
    SetItemStack {
        entity_id: i32,
        stack: Option<InventoryItemStack>,
    },
    SheepAppearance {
        entity_id: i32,
        fleece_color: u8,
        sheared: bool,
    },
    Animation {
        entity_id: i32,
        animation: NetEntityAnimation,
    },
    SetLabel {
        entity_id: i32,
        label: String,
    },
    CollectItem {
        collected_entity_id: i32,
        collector_entity_id: i32,
    },
    Destroy {
        entity_ids: Vec<i32>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetEntityAnimation {
    SwingMainArm,
    TakeDamage,
    LeaveBed,
    Unknown(u8),
}
