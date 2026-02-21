use bevy::prelude::*;
use educational_missile_physics::{
    camera::{CameraSettings, instructions, orbit},
    setup,
};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .init_resource::<CameraSettings>()
        .add_systems(Startup, (setup, instructions))
        .add_systems(Update, orbit)
        .run();
}
