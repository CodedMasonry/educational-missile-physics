use bevy::prelude::*;

#[derive(Component)]
pub struct LaunchPad;

pub fn spawn_launchpad(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    asset_server: &Res<AssetServer>,
) {
    let material = materials.add(StandardMaterial {
        base_color_texture: Some(asset_server.load("textures/launchpad/floor_diff.png")),
        normal_map_texture: Some(asset_server.load("textures/launchpad/floor_normal.png")),
        metallic_roughness_texture: Some(asset_server.load("textures/launchpad/floor_rough.png")),
        occlusion_texture: Some(asset_server.load("textures/launchpad/floor_ao.png")),
        flip_normal_map_y: false,
        ..default()
    });

    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(100.0, 5.0, 100.0))),
        MeshMaterial3d(material),
        Transform::from_xyz(-1010.0, -148.0, -425.0),
        LaunchPad,
    ));
}
