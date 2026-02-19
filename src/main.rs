use bevy::{pbr::wireframe::WireframePlugin, prelude::*};
use educational_missile_physics::setup;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins.set(ImagePlugin::default_nearest()),))
        .add_systems(Startup, setup)
        .run();
}
