use bevy::prelude::*;
use educational_missile_physics::{
    apply_velocity,
    entities::{
        camera::{self, CameraSettings},
        terrain::{self, LoadedChunks, TerrainConfig, TerrainPlugin},
    },
    physics::update_physics,
    plugins::{debug_axes::DebugAxesPlugin, debug_hud::spawn_debug_hud},
    setup,
};

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, DebugAxesPlugin, TerrainPlugin))
        .init_resource::<CameraSettings>()
        .init_resource::<TerrainConfig>()
        .init_resource::<LoadedChunks>()
        .add_systems(Startup, (setup, spawn_debug_hud))
        .add_systems(
            Update,
            (
                camera::orbit,
                terrain::update_terrain_chunks,
                (update_physics, apply_velocity).chain(),
            ),
        )
        .run();
}
