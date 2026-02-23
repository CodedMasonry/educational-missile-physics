#[cfg(not(target_arch = "wasm32"))]
use bevy::anti_alias::taa::TemporalAntiAliasing;
use bevy::{core_pipeline::Skybox, pbr::ScreenSpaceAmbientOcclusion, prelude::*};

use crate::entities::{
    launchpad::spawn_launchpad, missile::spawn_missile, terrain::TerrainTextures,
};

pub mod entities;
pub mod physics;
pub mod plugins;

pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
) {
    let terrain_textures = TerrainTextures::load(&asset_server);
    commands.insert_resource(terrain_textures);

    // Objects
    spawn_launchpad(&mut commands, &mut meshes, &mut materials, &asset_server);
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

    let skybox_handle = asset_server.load("textures/skybox.ktx2");

    // Camera
    commands.spawn((
        Camera3d::default(),
        Msaa::Off,
        #[cfg(not(target_arch = "wasm32"))]
        TemporalAntiAliasing::default(),
        ScreenSpaceAmbientOcclusion::default(),
        Skybox {
            image: skybox_handle.clone(),
            brightness: 1000.0,
            ..default()
        },
    ));
}
