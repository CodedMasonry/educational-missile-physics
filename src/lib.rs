use bevy::{
    anti_alias::fxaa::{Fxaa, Sensitivity},
    core_pipeline::Skybox,
    prelude::*,
};

use crate::entities::{launchpad::spawn_launchpad, missile::spawn_missile};

pub mod entities;
pub mod physics;
pub mod plugins;
pub mod routines;

pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
) {
    // Objects
    spawn_launchpad(&mut commands, &mut meshes, &mut materials);
    spawn_missile(&mut commands, &asset_server);

    // Light
    commands.spawn((
        DirectionalLight {
            illuminance: light_consts::lux::FULL_DAYLIGHT,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_rotation(Quat::from_xyzw(
            -0.13334629,
            -0.86597735,
            -0.3586996,
            0.3219264,
        )),
    ));

    // Skybox
    let skybox_handle = asset_server.load("textures/skybox.ktx2");

    // Camera
    commands.spawn((
        Camera3d::default(),
        Fxaa {
            enabled: true,
            edge_threshold: Sensitivity::High,
            edge_threshold_min: Sensitivity::Medium,
        },
        Skybox {
            image: skybox_handle.clone(),
            brightness: 1000.0,
            ..default()
        },
    ));
}
