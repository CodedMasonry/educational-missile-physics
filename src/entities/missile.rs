use std::f32::consts::PI;

use bevy::prelude::*;

use crate::plugins::debug_axes::DebugAxes;

#[derive(Component)]
pub struct Missile;

pub fn spawn_missile(commands: &mut Commands, asset_server: &Res<AssetServer>) {
    let missile_asset_handle =
        asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/Exocet_MM40.glb"));
    commands.spawn((
        SceneRoot(missile_asset_handle),
        Transform::from_xyz(-1010.0, 87.0, -525.0).with_rotation(Quat::from_rotation_x(PI / 2.0)),
        DebugAxes::new(20.0),
        Missile,
    ));
}
