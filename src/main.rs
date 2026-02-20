use bevy::{camera_controller::free_camera::FreeCameraPlugin, prelude::*};
use educational_missile_physics::setup;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(FreeCameraPlugin)
        .add_systems(Startup, setup)
        .run();
}
