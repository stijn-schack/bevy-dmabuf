use bevy::{
    camera_controller::free_camera::FreeCameraPlugin, input::common_conditions::input_just_pressed,
    prelude::*,
};
use bevy_dmabuf::import::ExternalBufferAssetLoader;
use common::*;
mod common;

fn main() {
    App::new()
        .add_plugins((
            ExamplePlugins {
                window_title: "Dmabuf render target example",
                ..default()
            },
            FreeCameraPlugin,
        ))
        .add_systems(
            Startup,
            (spawn_base_scene, spawn_cubes, spawn_external_render_target),
        )
        .add_systems(
            Update,
            save_render_target_to_disk
                .run_if(resource_exists::<OriginalBuffer>.and(input_just_pressed(KeyCode::Space))),
        )
        .run();
}

fn spawn_cubes(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    const CUBE_HALF_SIZE: f32 = 1.0;
    let cube = meshes.add(Cuboid::from_size(Vec3::splat(CUBE_HALF_SIZE * 2.0)));
    for (idx, color) in [Srgba::RED, Srgba::GREEN, Srgba::BLUE]
        .into_iter()
        .enumerate()
    {
        let material = materials.add(StandardMaterial::from_color(color));
        let transform = Transform::from_xyz(
            (CUBE_HALF_SIZE + 1.5) * (idx as f32 - 1.0),
            CUBE_HALF_SIZE,
            0.0,
        );
        commands.spawn((Mesh3d(cube.clone()), MeshMaterial3d(material), transform));
    }
}

fn spawn_external_render_target(
    mut commands: Commands,
    mut external_buffer_source: ResMut<ExternalBufferSource>,
    mut external_image_loader: ExternalBufferAssetLoader,
) {
    let (buffer_id, creation_data) = external_buffer_source.create_empty_buffer(1920, 1080);
    let external_target_bundle = external_image_loader.load_render_target(creation_data);

    commands.spawn((
        Transform::from_translation(vec3(12.0, 8.0, 12.0)).looking_at(Vec3::ZERO, Dir3::Y),
        Camera3d::default(),
        external_target_bundle,
    ));

    commands.insert_resource(OriginalBuffer(buffer_id))
}

fn save_render_target_to_disk(
    external_buffers: ResMut<ExternalBufferSource>,
    buffer_id: Res<OriginalBuffer>,
) {
    external_buffers.write_to_disk(**buffer_id);
}

#[derive(Resource, Deref)]
struct OriginalBuffer(BufferId);
