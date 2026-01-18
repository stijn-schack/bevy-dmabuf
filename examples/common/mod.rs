use crate::common::buffer_allocator::ExternalImageSourcePlugin;
use bevy::camera_controller::free_camera::FreeCamera;
use bevy::{app::PluginGroupBuilder, log::LogPlugin, prelude::*};
use bevy_dmabuf::{import::DmabufImportPlugin, wgpu_init::add_dmabuf_init_plugin};
use std::path::Path;

mod buffer_allocator;
pub use buffer_allocator::*;

pub struct ExamplePlugins {
    pub window_title: &'static str,
    pub capture_dir: &'static str
}

impl Default for ExamplePlugins {
    fn default() -> Self {
        Self {
            window_title: "Example",
            capture_dir: "/tmp",
        }
    }
}

impl PluginGroup for ExamplePlugins {
    fn build(self) -> PluginGroupBuilder {
        let group = PluginGroupBuilder::start::<Self>()
            .add_group(DefaultPlugins)
            .set(LogPlugin {
                filter: "info,bevy_dmabuf=trace".to_string(),
                ..default()
            })
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: self.window_title.to_string(),
                    ..default()
                }),
                ..default()
            })
            .add(DmabufImportPlugin)
            .add(ExternalImageSourcePlugin { capture_dir: Path::new(self.capture_dir) });

        add_dmabuf_init_plugin(group)
    }
}

pub fn spawn_base_scene(
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
        FreeCamera::default(),
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
