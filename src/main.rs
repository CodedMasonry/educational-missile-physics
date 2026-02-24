use bevy::prelude::*;
use educational_missile_physics::{
    entities::{
        camera::{CameraSettings, orbit},
        terrain::{LoadedChunks, TerrainConfig, TerrainPlugin, update_terrain_chunks},
    },
    plugins::debug_axes::DebugAxesPlugin,
    setup,
};

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, DebugAxesPlugin, TerrainPlugin))
        .init_resource::<CameraSettings>()
        .init_resource::<TerrainConfig>()
        .init_resource::<LoadedChunks>()
        .add_systems(Startup, setup)
        .add_systems(Update, (orbit, update_terrain_chunks))
        .run();
}
