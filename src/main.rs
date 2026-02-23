use bevy::prelude::*;
use educational_missile_physics::{
    entities::{
        camera::{CameraSettings, orbit},
        terrain::{LoadedChunks, TerrainConfig, update_terrain_chunks},
    },
    setup,
};

fn main() {
    App::new()
        .add_plugins((DefaultPlugins))
        .init_resource::<CameraSettings>()
        .init_resource::<TerrainConfig>()
        .init_resource::<LoadedChunks>()
        .add_systems(Startup, setup)
        .add_systems(Update, (orbit, update_terrain_chunks))
        .run();
}
