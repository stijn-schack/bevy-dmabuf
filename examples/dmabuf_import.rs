use bevy::{
    asset::embedded_asset, camera_controller::free_camera::FreeCameraPlugin, prelude::*,
    time::common_conditions::on_timer,
};
use bevy_dmabuf::import::ExternalBufferAssetLoader;
use common::*;
use std::time::Duration;

mod common;

fn main() -> AppExit {
    let mut app = App::new();
    app.add_plugins((
        ExamplePlugins {
            window_title: "Dmabuf import example",
            ..default()
        },
        FreeCameraPlugin,
    ));

    embedded_asset!(app, "examples/", "test_img.png");

    app.add_systems(Startup, load_test_image)
        .add_systems(PostStartup, spawn_base_scene)
        .add_systems(
            First,
            add_test_img_on_loaded.run_if(not(resource_exists::<TestImg>)),
        )
        .add_systems(
            Update,
            (
                recreate_external_image.run_if(on_timer(Duration::from_secs(2))),
                spawn_raw_img_entity.run_if(resource_added::<TestImg>),
            ),
        )
        .add_observer(init_external_image_entity);

    app.run()
}

fn spawn_raw_img_entity(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    test_img: Res<TestImgHandle>,
) {
    let mut material = StandardMaterial::from(test_img.clone());
    material.cull_mode = None;
    material.unlit = true;
    commands.spawn((
        Transform::default().with_translation(Vec3::new(24.0, 4.5, 0.0)),
        Mesh3d(meshes.add(Plane3d::new(Vec3::Z, Vec2::new(16.0, 9.0) / 2.0))),
        MeshMaterial3d(materials.add(material)),
    ));
}

fn load_test_image(asset_server: Res<AssetServer>, mut commands: Commands) {
    let handle: Handle<Image> = asset_server.load("embedded://dmabuf_import/test_img.png");
    commands.insert_resource(TestImgHandle(handle));
}

fn add_test_img_on_loaded(
    mut asset_events: MessageReader<AssetEvent<Image>>,
    test_img: Res<TestImgHandle>,
    images: Res<Assets<Image>>,
    mut commands: Commands,
) {
    for event in asset_events.read() {
        if event.is_loaded_with_dependencies(test_img.id()) {
            let img = images.get(test_img.id()).unwrap().clone();
            commands.insert_resource(TestImg(img.clone()));
            info!("Test image loaded");
            break;
        }
    }
}

fn init_external_image_entity(
    event: On<ExternalImageReadyEvent>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut material = StandardMaterial::from(event.image_handle.clone());
    material.cull_mode = None;
    material.unlit = true;

    let entity = commands
        .spawn((
            Transform::default().with_translation(Vec3::new(0.0, 4.5, 0.0)),
            Mesh3d(meshes.add(Plane3d::new(Vec3::Z, Vec2::new(16.0, 9.0) / 2.0))),
            MeshMaterial3d(materials.add(material)),
        ))
        .id();

    commands.insert_resource(TestEntity {
        entity,
        buffer_id: event.buffer_id,
    });
}

fn recreate_external_image(
    test_entity: Option<Res<TestEntity>>,
    test_img: Res<TestImg>,
    mut commands: Commands,
    mut ext_img_src: ResMut<ExternalBufferSource>,
    mut external_image_loader: ExternalBufferAssetLoader,
) {
    if let Some(test_entity) = test_entity {
        commands.entity(test_entity.entity).despawn();
        ext_img_src.remove(test_entity.buffer_id);
        commands.remove_resource::<TestEntity>()
    } else {
        let (buffer_id, creation_data) = ext_img_src.create_buffer_from_image(&test_img);
        let image_handle = external_image_loader.load_texture(creation_data);
        commands.trigger(ExternalImageReadyEvent {
            image_handle,
            buffer_id,
        })
    }
}

#[derive(Resource, Deref, DerefMut)]
struct TestImgHandle(Handle<Image>);

#[derive(Resource, Deref, DerefMut)]
struct TestImg(Image);

#[derive(Resource)]
struct TestEntity {
    entity: Entity,
    buffer_id: BufferId,
}

#[derive(Event, Debug)]
struct ExternalImageReadyEvent {
    image_handle: Handle<Image>,
    buffer_id: BufferId,
}
