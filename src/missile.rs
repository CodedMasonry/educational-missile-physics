use bevy::prelude::*;

pub fn spawn_missile(commands: &mut Commands, asset_server: &Res<AssetServer>) {
    let missile_asset_handle =
        asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/Exocet_MM40.glb"));

    commands.spawn((
        SceneRoot(missile_asset_handle),
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));
}
