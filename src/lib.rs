use std::f32::consts::PI;

#[cfg(not(target_arch = "wasm32"))]
use bevy::anti_alias::taa::TemporalAntiAliasing;
use bevy::{
    camera_controller::free_camera::FreeCamera, core_pipeline::Skybox,
    pbr::ScreenSpaceAmbientOcclusion, prelude::*,
};

use crate::missile::spawn_missile;

pub mod missile;

pub fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    spawn_missile(&mut commands, &asset_server);

    // let skybox_handle = asset_server.load("textures/EveningSky.exr");
    // camera
    commands.spawn((
        Camera3d::default(),
        Msaa::Off,
        #[cfg(not(target_arch = "wasm32"))]
        TemporalAntiAliasing::default(),
        ScreenSpaceAmbientOcclusion::default(),
        Transform::from_xyz(0.0, 0.0, 8.0).looking_at(Vec3::ZERO, Vec3::Y),
        FreeCamera::default(),
        // Skybox {
        //     image: skybox_handle.clone(),
        //     brightness: 1000.0,
        //     ..default()
        // },
        // EnvironmentMapLight {
        //     // diffuse_map: skybox_handle.clone(),
        //     // specular_map: skybox_handle,
        //     intensity: 2000.0,
        //     ..default()
        // },
    ));
}
