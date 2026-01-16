use crate::common::ExternalImageSource;
use bevy::{
    asset::embedded_asset,
    camera_controller::free_camera::{FreeCamera, FreeCameraPlugin},
    log::LogPlugin,
    prelude::*,
    time::common_conditions::on_timer,
};
use bevy_dmabuf::{
    import::{DmabufImportPlugin, ExternalImageAssetLoader},
    wgpu_init::add_dmabuf_init_plugin,
};
use std::time::Duration;

mod common;

fn main() -> AppExit {
    let mut app = App::new();
    app.add_plugins((
        add_dmabuf_init_plugin(DefaultPlugins)
            .set(LogPlugin {
                filter: "info,bevy_dmabuf=trace".to_string(),
                ..default()
            })
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Dmabuf import example".to_string(),
                    ..default()
                }),
                ..default()
            }),
        FreeCameraPlugin,
        DmabufImportPlugin,
    ));

    embedded_asset!(app, "examples/", "test_img.png");

    app.insert_resource(ExternalImageSource::new())
        .add_systems(Startup, load_test_image)
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

fn spawn_base_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    info!("Spawning base scene");
    // camera
    let camera_transform = Transform::from_xyz(-2.5, 4.5, 9.0).looking_at(Vec3::ZERO, Vec3::Y);
    commands.spawn((
        Camera3d::default(),
        camera_transform,
        FreeCamera { ..default() },
    ));

    commands.spawn((
        Mesh3d(meshes.add(Circle::new(4.0))),
        MeshMaterial3d(materials.add(StandardMaterial::default())),
        Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
    ));

    let light_transform = Transform::from_translation(camera_transform.back().as_vec3() * 20_000.0)
        .looking_at(Vec3::ZERO, Vec3::Y);
    commands.spawn((DirectionalLight::default(), light_transform));
    info!("Base scene spawned");
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
    let mut material = StandardMaterial::from(event.0.clone());
    material.cull_mode = None;
    material.unlit = true;

    let entity = commands
        .spawn((
            Transform::default().with_translation(Vec3::new(0.0, 4.5, 0.0)),
            Mesh3d(meshes.add(Plane3d::new(Vec3::Z, Vec2::new(16.0, 9.0) / 2.0))),
            MeshMaterial3d(materials.add(material)),
        ))
        .id();

    commands.insert_resource(TestEntity(entity));
}

fn recreate_external_image(
    test_entity: Option<Res<TestEntity>>,
    test_img: Res<TestImg>,
    mut commands: Commands,
    mut ext_img_src: ResMut<ExternalImageSource>,
    mut external_image_loader: ExternalImageAssetLoader,
) {
    if let Some(entity) = test_entity {
        commands.entity(**entity).despawn();
        commands.remove_resource::<TestEntity>()
    } else {
        let handle = external_image_loader
            .load(ext_img_src.create_buffer(&test_img))
            .unwrap();
        commands.trigger(ExternalImageReadyEvent(handle))
    }
}

#[derive(Resource, Deref, DerefMut)]
struct TestImgHandle(Handle<Image>);

#[derive(Resource, Deref, DerefMut)]
struct TestImg(Image);

#[derive(Resource, Deref, DerefMut)]
struct TestEntity(Entity);

#[derive(Event, Debug)]
struct ExternalImageReadyEvent(Handle<Image>);
