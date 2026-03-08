use bevy::prelude::*;
use educational_missile_physics::{
    entities::{
        camera::{self, CameraSettings},
        terrain::{self, LoadedChunks, TerrainConfig, TerrainPlugin},
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
        .add_systems(Update, (camera::orbit, terrain::update_terrain_chunks))
        .run();
}
