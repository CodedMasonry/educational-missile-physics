use bevy::prelude::*;

#[derive(Component)]
pub struct LaunchPad;

pub fn spawn_launchpad(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
) {
    let material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.25, 0.26, 0.27),
        perceptual_roughness: 0.92,
        metallic: 0.05,
        reflectance: 0.2,
        ..default()
    });

    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(100.0, 5.0, 100.0))),
        MeshMaterial3d(material),
        Transform::from_xyz(-1010.0, 70.0, -525.0),
        LaunchPad,
    ));
}
