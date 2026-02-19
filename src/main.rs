use bevy::{pbr::wireframe::WireframePlugin, prelude::*};

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(ImagePlugin::default_nearest()),
            WireframePlugin::default(),
        ))
        .run();
}
