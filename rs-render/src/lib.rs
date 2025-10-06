use bevy::{core_pipeline::Skybox, input::mouse::MouseMotion, prelude::*};
use rs_utils::AppState;

pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_camera)
            .add_systems(Startup, setup_scene)
            .add_systems(Update, rotate_cube)
            .add_systems(Update, camera_mouse_look);
    }
}

#[derive(Resource)]
struct CameraState {
    pitch: f32,
    yaw: f32,
}

pub fn setup_camera(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
) {
    let image_handle = asset_server.load("skybox.ktx2");

    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0., 2., 5.).looking_at(Vec3::ZERO, Vec3::Z),
        Skybox {
            image: image_handle,
            brightness: 1000.,
            ..Default::default()
        },
    ));

    commands.insert_resource(CameraState { yaw: 0., pitch: 0. });
}

pub fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let cube_mesh = Mesh::from(Cuboid::new(1.0, 1.0, 1.0));

    commands.spawn((
        Mesh3d(meshes.add(cube_mesh)),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.2, 0.6, 0.9),
            ..default()
        })),
        Transform::from_xyz(0.0, 0.5, 0.0),
        Visibility::default(),
        Rotates,
    ));

    commands.spawn((
        PointLight {
            intensity: 1500.0,
            range: 20.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(4.0, 8.0, 4.0),
        Visibility::default(),
    ));
}

#[derive(Component)]
struct Rotates;

fn rotate_cube(mut query: Query<&mut Transform, With<Rotates>>, time: Res<Time>) {
    for mut transform in &mut query {
        transform.rotate_y(1.0 * time.delta_secs());
    }
}

fn camera_mouse_look(
    mut state: ResMut<CameraState>,
    mut query: Query<&mut Transform, With<Camera3d>>,
    mut motion_events: EventReader<MouseMotion>,
    app_state: ResMut<AppState>,
) {
    if !matches!(app_state.0, rs_utils::ApplicationState::Connected) {
        return;
    }
    let mut delta = Vec2::ZERO;
    for ev in motion_events.read() {
        delta += ev.delta;
    }

    // Sensitivity
    let sensitivity = 0.002;
    state.yaw -= delta.x * sensitivity;
    state.pitch -= delta.y * sensitivity;
    state.pitch = state.pitch.clamp(-1.54, 1.54); // limit pitch (~±88°)

    // Compute new rotation
    let rotation =
        Quat::from_axis_angle(Vec3::Y, state.yaw) * Quat::from_axis_angle(Vec3::X, state.pitch);

    for mut transform in query.iter_mut() {
        transform.rotation = rotation;
    }
}
