use crate::{missile::spawn_missile, terrain::TerrainTextures};
#[cfg(not(target_arch = "wasm32"))]
use bevy::anti_alias::taa::TemporalAntiAliasing;
use bevy::{core_pipeline::Skybox, pbr::ScreenSpaceAmbientOcclusion, prelude::*};

pub mod camera;
pub mod missile;
pub mod terrain;

pub fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    let terrain_textures = TerrainTextures::load(&asset_server);
    commands.insert_resource(terrain_textures);

    // Objects
    spawn_missile(&mut commands, &asset_server);

    // Light
    commands.spawn((
        DirectionalLight {
            illuminance: light_consts::lux::FULL_DAYLIGHT,
            shadows_enabled: true,
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
        Transform::from_xyz(5.0, 205.0, 5.0).looking_at(Vec3::new(0.0, 200.0, 0.0), Vec3::Y),
        Skybox {
            image: skybox_handle.clone(),
            brightness: 1000.0,
            ..default()
        },
    ));
}
