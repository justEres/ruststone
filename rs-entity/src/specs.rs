use bevy::prelude::{Color, Vec3};
use rs_utils::{MobKind, NetEntityKind, ObjectKind};

use crate::RemoteQuadrupedAnimTuning;

const PLAYER_SCALE: Vec3 = Vec3::ONE;
const PLAYER_Y_OFFSET: f32 = 0.0;
const PLAYER_NAME_Y_OFFSET: f32 = 2.05;

const MOB_SCALE: Vec3 = Vec3::new(0.55, 0.9, 0.55);
const MOB_Y_OFFSET: f32 = 0.9;
const MOB_NAME_Y_OFFSET: f32 = 1.35;

const ITEM_SCALE: Vec3 = Vec3::splat(0.17);
const ITEM_Y_OFFSET: f32 = 0.17;
const ITEM_NAME_Y_OFFSET: f32 = 0.5;

const ORB_SCALE: Vec3 = Vec3::splat(0.14);
const ORB_Y_OFFSET: f32 = 0.14;
const ORB_NAME_Y_OFFSET: f32 = 0.45;

const OBJECT_SCALE: Vec3 = Vec3::splat(0.28);
const OBJECT_Y_OFFSET: f32 = 0.28;
const OBJECT_NAME_Y_OFFSET: f32 = 0.65;

#[derive(Clone, Copy)]
pub(crate) enum VisualMesh {
    Capsule,
    Sphere,
}

#[derive(Clone, Copy)]
pub(crate) struct VisualSpec {
    pub mesh: VisualMesh,
    pub scale: Vec3,
    pub y_offset: f32,
    pub name_y_offset: f32,
    pub color: Color,
}

#[derive(Clone, Copy)]
struct MobSpec {
    kind: MobKind,
    label: &'static str,
    color: [f32; 3],
    uses_biped_model: bool,
    uses_quadruped_model: bool,
    scale: Vec3,
    name_y_offset: f32,
    texture_path: Option<&'static str>,
    quadruped_tuning: RemoteQuadrupedAnimTuning,
    biped_model: BipedModelKind,
    quadruped_model: QuadrupedModelKind,
}

#[derive(Clone, Copy)]
struct ObjectSpec {
    kind: ObjectKind,
    label: &'static str,
    color: [f32; 3],
}

#[derive(Clone, Copy)]
pub(crate) enum BipedModelKind {
    Tex32,
    Tex64,
}

#[derive(Clone, Copy)]
pub(crate) enum QuadrupedModelKind {
    PigTex32,
    SheepTex32,
    CowTex32,
    CreeperTex64,
}

const DEFAULT_QUAD_TUNING: RemoteQuadrupedAnimTuning = RemoteQuadrupedAnimTuning {
    body_pitch: -std::f32::consts::FRAC_PI_2,
    leg_swing_scale: 1.0,
};

const MOB_SPECS: &[MobSpec] = &[
    MobSpec {
        kind: MobKind::Creeper,
        label: "Creeper",
        color: [0.10, 0.78, 0.12],
        uses_biped_model: false,
        uses_quadruped_model: true,
        scale: Vec3::ONE,
        name_y_offset: 1.8,
        texture_path: Some("entity/creeper/creeper.png"),
        quadruped_tuning: RemoteQuadrupedAnimTuning { body_pitch: 0.0, leg_swing_scale: 0.95 },
        biped_model: BipedModelKind::Tex32,
        quadruped_model: QuadrupedModelKind::CreeperTex64,
    },
    MobSpec {
        kind: MobKind::Skeleton,
        label: "Skeleton",
        color: [0.86, 0.86, 0.86],
        uses_biped_model: true,
        uses_quadruped_model: false,
        scale: Vec3::ONE,
        name_y_offset: 2.05,
        texture_path: Some("entity/skeleton/skeleton.png"),
        quadruped_tuning: DEFAULT_QUAD_TUNING,
        biped_model: BipedModelKind::Tex32,
        quadruped_model: QuadrupedModelKind::PigTex32,
    },
    MobSpec {
        kind: MobKind::Spider,
        label: "Spider",
        color: [0.22, 0.22, 0.22],
        uses_biped_model: false,
        uses_quadruped_model: false,
        scale: Vec3::ONE,
        name_y_offset: 1.6,
        texture_path: None,
        quadruped_tuning: DEFAULT_QUAD_TUNING,
        biped_model: BipedModelKind::Tex32,
        quadruped_model: QuadrupedModelKind::PigTex32,
    },
    MobSpec {
        kind: MobKind::Giant,
        label: "Giant",
        color: [0.72, 0.35, 0.85],
        uses_biped_model: false,
        uses_quadruped_model: false,
        scale: Vec3::ONE,
        name_y_offset: 1.6,
        texture_path: None,
        quadruped_tuning: DEFAULT_QUAD_TUNING,
        biped_model: BipedModelKind::Tex32,
        quadruped_model: QuadrupedModelKind::PigTex32,
    },
    MobSpec {
        kind: MobKind::Zombie,
        label: "Zombie",
        color: [0.25, 0.73, 0.25],
        uses_biped_model: true,
        uses_quadruped_model: false,
        scale: Vec3::ONE,
        name_y_offset: 2.05,
        texture_path: Some("entity/zombie/zombie.png"),
        quadruped_tuning: DEFAULT_QUAD_TUNING,
        biped_model: BipedModelKind::Tex64,
        quadruped_model: QuadrupedModelKind::PigTex32,
    },
    MobSpec {
        kind: MobKind::Slime,
        label: "Slime",
        color: [0.72, 0.35, 0.85],
        uses_biped_model: false,
        uses_quadruped_model: false,
        scale: Vec3::ONE,
        name_y_offset: 1.6,
        texture_path: None,
        quadruped_tuning: DEFAULT_QUAD_TUNING,
        biped_model: BipedModelKind::Tex32,
        quadruped_model: QuadrupedModelKind::PigTex32,
    },
    MobSpec {
        kind: MobKind::Ghast,
        label: "Ghast",
        color: [0.92, 0.45, 0.12],
        uses_biped_model: false,
        uses_quadruped_model: false,
        scale: Vec3::ONE,
        name_y_offset: 1.6,
        texture_path: None,
        quadruped_tuning: DEFAULT_QUAD_TUNING,
        biped_model: BipedModelKind::Tex32,
        quadruped_model: QuadrupedModelKind::PigTex32,
    },
    MobSpec {
        kind: MobKind::PigZombie,
        label: "Zombie Pigman",
        color: [0.25, 0.73, 0.25],
        uses_biped_model: true,
        uses_quadruped_model: false,
        scale: Vec3::ONE,
        name_y_offset: 2.05,
        texture_path: Some("entity/zombie_pigman.png"),
        quadruped_tuning: DEFAULT_QUAD_TUNING,
        biped_model: BipedModelKind::Tex64,
        quadruped_model: QuadrupedModelKind::PigTex32,
    },
    MobSpec {
        kind: MobKind::Enderman,
        label: "Enderman",
        color: [0.20, 0.10, 0.28],
        uses_biped_model: true,
        uses_quadruped_model: false,
        scale: Vec3::new(1.06, 1.38, 1.06),
        name_y_offset: 2.65,
        texture_path: Some("entity/enderman/enderman.png"),
        quadruped_tuning: DEFAULT_QUAD_TUNING,
        biped_model: BipedModelKind::Tex32,
        quadruped_model: QuadrupedModelKind::PigTex32,
    },
    MobSpec {
        kind: MobKind::CaveSpider,
        label: "Cave Spider",
        color: [0.22, 0.22, 0.22],
        uses_biped_model: false,
        uses_quadruped_model: false,
        scale: Vec3::ONE,
        name_y_offset: 1.6,
        texture_path: None,
        quadruped_tuning: DEFAULT_QUAD_TUNING,
        biped_model: BipedModelKind::Tex32,
        quadruped_model: QuadrupedModelKind::PigTex32,
    },
    MobSpec {
        kind: MobKind::Silverfish,
        label: "Silverfish",
        color: [0.72, 0.35, 0.85],
        uses_biped_model: false,
        uses_quadruped_model: false,
        scale: Vec3::ONE,
        name_y_offset: 1.6,
        texture_path: None,
        quadruped_tuning: DEFAULT_QUAD_TUNING,
        biped_model: BipedModelKind::Tex32,
        quadruped_model: QuadrupedModelKind::PigTex32,
    },
    MobSpec {
        kind: MobKind::Blaze,
        label: "Blaze",
        color: [0.92, 0.45, 0.12],
        uses_biped_model: false,
        uses_quadruped_model: false,
        scale: Vec3::ONE,
        name_y_offset: 1.6,
        texture_path: None,
        quadruped_tuning: DEFAULT_QUAD_TUNING,
        biped_model: BipedModelKind::Tex32,
        quadruped_model: QuadrupedModelKind::PigTex32,
    },
    MobSpec {
        kind: MobKind::MagmaCube,
        label: "Magma Cube",
        color: [0.92, 0.45, 0.12],
        uses_biped_model: false,
        uses_quadruped_model: false,
        scale: Vec3::ONE,
        name_y_offset: 1.6,
        texture_path: None,
        quadruped_tuning: DEFAULT_QUAD_TUNING,
        biped_model: BipedModelKind::Tex32,
        quadruped_model: QuadrupedModelKind::PigTex32,
    },
    MobSpec {
        kind: MobKind::EnderDragon,
        label: "Ender Dragon",
        color: [0.72, 0.35, 0.85],
        uses_biped_model: false,
        uses_quadruped_model: false,
        scale: Vec3::ONE,
        name_y_offset: 1.6,
        texture_path: None,
        quadruped_tuning: DEFAULT_QUAD_TUNING,
        biped_model: BipedModelKind::Tex32,
        quadruped_model: QuadrupedModelKind::PigTex32,
    },
    MobSpec {
        kind: MobKind::Wither,
        label: "Wither",
        color: [0.86, 0.86, 0.86],
        uses_biped_model: false,
        uses_quadruped_model: false,
        scale: Vec3::ONE,
        name_y_offset: 1.6,
        texture_path: None,
        quadruped_tuning: DEFAULT_QUAD_TUNING,
        biped_model: BipedModelKind::Tex32,
        quadruped_model: QuadrupedModelKind::PigTex32,
    },
    MobSpec {
        kind: MobKind::Bat,
        label: "Bat",
        color: [0.72, 0.35, 0.85],
        uses_biped_model: false,
        uses_quadruped_model: false,
        scale: Vec3::ONE,
        name_y_offset: 1.6,
        texture_path: None,
        quadruped_tuning: DEFAULT_QUAD_TUNING,
        biped_model: BipedModelKind::Tex32,
        quadruped_model: QuadrupedModelKind::PigTex32,
    },
    MobSpec {
        kind: MobKind::Witch,
        label: "Witch",
        color: [0.72, 0.35, 0.85],
        uses_biped_model: false,
        uses_quadruped_model: false,
        scale: Vec3::ONE,
        name_y_offset: 1.6,
        texture_path: None,
        quadruped_tuning: DEFAULT_QUAD_TUNING,
        biped_model: BipedModelKind::Tex32,
        quadruped_model: QuadrupedModelKind::PigTex32,
    },
    MobSpec {
        kind: MobKind::Endermite,
        label: "Endermite",
        color: [0.22, 0.22, 0.22],
        uses_biped_model: false,
        uses_quadruped_model: false,
        scale: Vec3::ONE,
        name_y_offset: 1.6,
        texture_path: None,
        quadruped_tuning: DEFAULT_QUAD_TUNING,
        biped_model: BipedModelKind::Tex32,
        quadruped_model: QuadrupedModelKind::PigTex32,
    },
    MobSpec {
        kind: MobKind::Guardian,
        label: "Guardian",
        color: [0.72, 0.35, 0.85],
        uses_biped_model: false,
        uses_quadruped_model: false,
        scale: Vec3::ONE,
        name_y_offset: 1.6,
        texture_path: None,
        quadruped_tuning: DEFAULT_QUAD_TUNING,
        biped_model: BipedModelKind::Tex32,
        quadruped_model: QuadrupedModelKind::PigTex32,
    },
    MobSpec {
        kind: MobKind::Pig,
        label: "Pig",
        color: [0.30, 0.55, 0.88],
        uses_biped_model: false,
        uses_quadruped_model: true,
        scale: Vec3::ONE,
        name_y_offset: 1.6,
        texture_path: Some("entity/pig/pig.png"),
        quadruped_tuning: DEFAULT_QUAD_TUNING,
        biped_model: BipedModelKind::Tex32,
        quadruped_model: QuadrupedModelKind::PigTex32,
    },
    MobSpec {
        kind: MobKind::Sheep,
        label: "Sheep",
        color: [0.30, 0.55, 0.88],
        uses_biped_model: false,
        uses_quadruped_model: true,
        scale: Vec3::ONE,
        name_y_offset: 1.6,
        texture_path: Some("entity/sheep/sheep.png"),
        quadruped_tuning: DEFAULT_QUAD_TUNING,
        biped_model: BipedModelKind::Tex32,
        quadruped_model: QuadrupedModelKind::SheepTex32,
    },
    MobSpec {
        kind: MobKind::Cow,
        label: "Cow",
        color: [0.30, 0.55, 0.88],
        uses_biped_model: false,
        uses_quadruped_model: true,
        scale: Vec3::ONE,
        name_y_offset: 1.8,
        texture_path: Some("entity/cow/cow.png"),
        quadruped_tuning: DEFAULT_QUAD_TUNING,
        biped_model: BipedModelKind::Tex32,
        quadruped_model: QuadrupedModelKind::CowTex32,
    },
    MobSpec {
        kind: MobKind::Chicken,
        label: "Chicken",
        color: [0.30, 0.55, 0.88],
        uses_biped_model: false,
        uses_quadruped_model: true,
        scale: Vec3::splat(0.62),
        name_y_offset: 1.2,
        texture_path: Some("entity/chicken.png"),
        quadruped_tuning: RemoteQuadrupedAnimTuning {
            body_pitch: -std::f32::consts::FRAC_PI_2,
            leg_swing_scale: 1.2,
        },
        biped_model: BipedModelKind::Tex32,
        quadruped_model: QuadrupedModelKind::PigTex32,
    },
    MobSpec {
        kind: MobKind::Squid,
        label: "Squid",
        color: [0.30, 0.55, 0.88],
        uses_biped_model: false,
        uses_quadruped_model: false,
        scale: Vec3::ONE,
        name_y_offset: 1.6,
        texture_path: Some("entity/squid.png"),
        quadruped_tuning: DEFAULT_QUAD_TUNING,
        biped_model: BipedModelKind::Tex32,
        quadruped_model: QuadrupedModelKind::PigTex32,
    },
    MobSpec {
        kind: MobKind::Wolf,
        label: "Wolf",
        color: [0.30, 0.55, 0.88],
        uses_biped_model: false,
        uses_quadruped_model: true,
        scale: Vec3::splat(0.78),
        name_y_offset: 1.6,
        texture_path: Some("entity/wolf/wolf.png"),
        quadruped_tuning: DEFAULT_QUAD_TUNING,
        biped_model: BipedModelKind::Tex32,
        quadruped_model: QuadrupedModelKind::SheepTex32,
    },
    MobSpec {
        kind: MobKind::Mooshroom,
        label: "Mooshroom",
        color: [0.30, 0.55, 0.88],
        uses_biped_model: false,
        uses_quadruped_model: true,
        scale: Vec3::ONE,
        name_y_offset: 1.8,
        texture_path: Some("entity/cow/mooshroom.png"),
        quadruped_tuning: DEFAULT_QUAD_TUNING,
        biped_model: BipedModelKind::Tex32,
        quadruped_model: QuadrupedModelKind::CowTex32,
    },
    MobSpec {
        kind: MobKind::SnowGolem,
        label: "Snow Golem",
        color: [0.30, 0.55, 0.88],
        uses_biped_model: false,
        uses_quadruped_model: false,
        scale: Vec3::ONE,
        name_y_offset: 1.6,
        texture_path: Some("entity/snow_golem.png"),
        quadruped_tuning: DEFAULT_QUAD_TUNING,
        biped_model: BipedModelKind::Tex32,
        quadruped_model: QuadrupedModelKind::PigTex32,
    },
    MobSpec {
        kind: MobKind::Ocelot,
        label: "Ocelot",
        color: [0.30, 0.55, 0.88],
        uses_biped_model: false,
        uses_quadruped_model: true,
        scale: Vec3::splat(0.78),
        name_y_offset: 1.6,
        texture_path: Some("entity/cat/ocelot.png"),
        quadruped_tuning: DEFAULT_QUAD_TUNING,
        biped_model: BipedModelKind::Tex32,
        quadruped_model: QuadrupedModelKind::SheepTex32,
    },
    MobSpec {
        kind: MobKind::IronGolem,
        label: "Iron Golem",
        color: [0.30, 0.55, 0.88],
        uses_biped_model: false,
        uses_quadruped_model: false,
        scale: Vec3::ONE,
        name_y_offset: 1.6,
        texture_path: Some("entity/iron_golem.png"),
        quadruped_tuning: DEFAULT_QUAD_TUNING,
        biped_model: BipedModelKind::Tex32,
        quadruped_model: QuadrupedModelKind::PigTex32,
    },
    MobSpec {
        kind: MobKind::Horse,
        label: "Horse",
        color: [0.30, 0.55, 0.88],
        uses_biped_model: false,
        uses_quadruped_model: true,
        scale: Vec3::splat(1.24),
        name_y_offset: 2.0,
        texture_path: Some("entity/horse/horse_white.png"),
        quadruped_tuning: RemoteQuadrupedAnimTuning {
            body_pitch: -std::f32::consts::FRAC_PI_2,
            leg_swing_scale: 0.82,
        },
        biped_model: BipedModelKind::Tex32,
        quadruped_model: QuadrupedModelKind::CowTex32,
    },
    MobSpec {
        kind: MobKind::Rabbit,
        label: "Rabbit",
        color: [0.30, 0.55, 0.88],
        uses_biped_model: false,
        uses_quadruped_model: true,
        scale: Vec3::splat(0.52),
        name_y_offset: 1.2,
        texture_path: Some("entity/rabbit/brown.png"),
        quadruped_tuning: RemoteQuadrupedAnimTuning {
            body_pitch: -std::f32::consts::FRAC_PI_2,
            leg_swing_scale: 1.2,
        },
        biped_model: BipedModelKind::Tex32,
        quadruped_model: QuadrupedModelKind::PigTex32,
    },
    MobSpec {
        kind: MobKind::Villager,
        label: "Villager",
        color: [0.30, 0.55, 0.88],
        uses_biped_model: true,
        uses_quadruped_model: false,
        scale: Vec3::new(0.96, 0.98, 0.96),
        name_y_offset: 2.05,
        texture_path: Some("entity/villager/villager.png"),
        quadruped_tuning: DEFAULT_QUAD_TUNING,
        biped_model: BipedModelKind::Tex64,
        quadruped_model: QuadrupedModelKind::PigTex32,
    },
];

const OBJECT_SPECS: &[ObjectSpec] = &[
    ObjectSpec { kind: ObjectKind::Boat, label: "Boat", color: [0.72, 0.56, 0.35] },
    ObjectSpec { kind: ObjectKind::Minecart, label: "Minecart", color: [0.72, 0.56, 0.35] },
    ObjectSpec { kind: ObjectKind::Arrow, label: "Arrow", color: [0.72, 0.72, 0.72] },
    ObjectSpec { kind: ObjectKind::Snowball, label: "Snowball", color: [0.72, 0.72, 0.72] },
    ObjectSpec { kind: ObjectKind::ItemFrame, label: "Item Frame", color: [0.72, 0.56, 0.35] },
    ObjectSpec { kind: ObjectKind::LeashKnot, label: "Leash Knot", color: [0.72, 0.56, 0.35] },
    ObjectSpec { kind: ObjectKind::EnderPearl, label: "Ender Pearl", color: [0.72, 0.72, 0.72] },
    ObjectSpec { kind: ObjectKind::EnderEye, label: "Ender Eye", color: [0.72, 0.72, 0.72] },
    ObjectSpec { kind: ObjectKind::Firework, label: "Firework", color: [0.45, 0.74, 0.88] },
    ObjectSpec { kind: ObjectKind::LargeFireball, label: "Fireball", color: [0.90, 0.25, 0.18] },
    ObjectSpec { kind: ObjectKind::SmallFireball, label: "Small Fireball", color: [0.90, 0.25, 0.18] },
    ObjectSpec { kind: ObjectKind::WitherSkull, label: "Wither Skull", color: [0.90, 0.25, 0.18] },
    ObjectSpec { kind: ObjectKind::Egg, label: "Egg", color: [0.72, 0.72, 0.72] },
    ObjectSpec { kind: ObjectKind::SplashPotion, label: "Splash Potion", color: [0.45, 0.74, 0.88] },
    ObjectSpec { kind: ObjectKind::ExpBottle, label: "XP Bottle", color: [0.45, 0.74, 0.88] },
    ObjectSpec { kind: ObjectKind::FishingHook, label: "Fishing Hook", color: [0.72, 0.56, 0.35] },
    ObjectSpec { kind: ObjectKind::PrimedTnt, label: "Primed TNT", color: [0.90, 0.25, 0.18] },
    ObjectSpec { kind: ObjectKind::ArmorStand, label: "Armor Stand", color: [0.72, 0.56, 0.35] },
    ObjectSpec { kind: ObjectKind::EndCrystal, label: "End Crystal", color: [0.45, 0.74, 0.88] },
    ObjectSpec { kind: ObjectKind::FallingBlock, label: "Falling Block", color: [0.45, 0.74, 0.88] },
];

fn mob_spec(kind: MobKind) -> Option<&'static MobSpec> {
    MOB_SPECS.iter().find(|s| s.kind == kind)
}

fn object_spec(kind: ObjectKind) -> Option<&'static ObjectSpec> {
    OBJECT_SPECS.iter().find(|s| s.kind == kind)
}

pub(crate) fn visual_spec(kind: NetEntityKind) -> VisualSpec {
    match kind {
        NetEntityKind::Player => VisualSpec {
            mesh: VisualMesh::Capsule,
            scale: PLAYER_SCALE,
            y_offset: PLAYER_Y_OFFSET,
            name_y_offset: PLAYER_NAME_Y_OFFSET,
            color: Color::srgb(0.85, 0.78, 0.72),
        },
        NetEntityKind::Mob(mob) => VisualSpec {
            mesh: VisualMesh::Capsule,
            scale: if mob_uses_entity_model(mob) {
                mob_model_scale(mob)
            } else {
                MOB_SCALE
            },
            y_offset: if mob_uses_entity_model(mob) { 0.0 } else { MOB_Y_OFFSET },
            name_y_offset: if mob_uses_entity_model(mob) {
                mob_model_name_y_offset(mob)
            } else {
                MOB_NAME_Y_OFFSET
            },
            color: mob_color(mob),
        },
        NetEntityKind::Item => VisualSpec {
            mesh: VisualMesh::Sphere,
            scale: ITEM_SCALE,
            y_offset: ITEM_Y_OFFSET,
            name_y_offset: ITEM_NAME_Y_OFFSET,
            color: Color::srgb(0.95, 0.85, 0.20),
        },
        NetEntityKind::ExperienceOrb => VisualSpec {
            mesh: VisualMesh::Sphere,
            scale: ORB_SCALE,
            y_offset: ORB_Y_OFFSET,
            name_y_offset: ORB_NAME_Y_OFFSET,
            color: Color::srgb(0.15, 0.95, 0.20),
        },
        NetEntityKind::Object(obj) => {
            let color = object_color(obj);
            VisualSpec {
                mesh: VisualMesh::Sphere,
                scale: OBJECT_SCALE,
                y_offset: OBJECT_Y_OFFSET,
                name_y_offset: OBJECT_NAME_Y_OFFSET,
                color,
            }
        }
    }
}

pub(crate) fn mob_color(kind: MobKind) -> Color {
    match mob_spec(kind) {
        Some(spec) => Color::srgb(spec.color[0], spec.color[1], spec.color[2]),
        None => Color::srgb(0.72, 0.35, 0.85),
    }
}

pub(crate) fn object_color(kind: ObjectKind) -> Color {
    match object_spec(kind) {
        Some(spec) => Color::srgb(spec.color[0], spec.color[1], spec.color[2]),
        None => Color::srgb(0.45, 0.74, 0.88),
    }
}

pub(crate) fn kind_label(kind: NetEntityKind) -> &'static str {
    match kind {
        NetEntityKind::Player => "Player",
        NetEntityKind::Item => "Dropped Item",
        NetEntityKind::ExperienceOrb => "XP Orb",
        NetEntityKind::Mob(mob) => mob_label(mob),
        NetEntityKind::Object(object) => object_label(object),
    }
}

pub(crate) fn mob_label(kind: MobKind) -> &'static str {
    mob_spec(kind).map_or("Mob", |s| s.label)
}

pub(crate) fn object_label(kind: ObjectKind) -> &'static str {
    object_spec(kind).map_or("Object", |s| s.label)
}

pub(crate) fn mob_uses_biped_model(mob: MobKind) -> bool {
    mob_spec(mob).is_some_and(|s| s.uses_biped_model)
}

pub(crate) fn mob_uses_quadruped_model(mob: MobKind) -> bool {
    mob_spec(mob).is_some_and(|s| s.uses_quadruped_model)
}

pub(crate) fn mob_uses_entity_model(mob: MobKind) -> bool {
    mob_uses_biped_model(mob) || mob_uses_quadruped_model(mob)
}

pub(crate) fn mob_model_scale(mob: MobKind) -> Vec3 {
    mob_spec(mob).map_or(Vec3::ONE, |s| s.scale)
}

pub(crate) fn mob_model_name_y_offset(mob: MobKind) -> f32 {
    mob_spec(mob).map_or(1.6, |s| s.name_y_offset)
}

pub(crate) fn mob_quadruped_anim_tuning(mob: MobKind) -> RemoteQuadrupedAnimTuning {
    mob_spec(mob).map_or(DEFAULT_QUAD_TUNING, |s| s.quadruped_tuning)
}

pub(crate) fn mob_texture_path(mob: MobKind) -> Option<&'static str> {
    mob_spec(mob).and_then(|s| s.texture_path)
}

pub(crate) fn mob_biped_model_kind(mob: MobKind) -> BipedModelKind {
    mob_spec(mob).map_or(BipedModelKind::Tex32, |s| s.biped_model)
}

pub(crate) fn mob_quadruped_model_kind(mob: MobKind) -> QuadrupedModelKind {
    mob_spec(mob).map_or(QuadrupedModelKind::PigTex32, |s| s.quadruped_model)
}
