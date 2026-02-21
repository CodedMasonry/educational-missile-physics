#[cfg(not(target_arch = "wasm32"))]
use bevy::anti_alias::taa::TemporalAntiAliasing;
use bevy::{core_pipeline::Skybox, pbr::ScreenSpaceAmbientOcclusion, prelude::*};

use crate::missile::spawn_missile;

pub mod camera;
pub mod missile;

pub fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    spawn_missile(&mut commands, &asset_server);

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
    // camera
    commands.spawn((
        Camera3d::default(),
        Msaa::Off,
        #[cfg(not(target_arch = "wasm32"))]
        TemporalAntiAliasing::default(),
        ScreenSpaceAmbientOcclusion::default(),
        Transform::from_xyz(5.0, 5.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
        Skybox {
            image: skybox_handle.clone(),
            brightness: 1000.0,
            ..default()
        },
    ));
}
