//! # Debug Axes — Gizmos Edition (lightweight)
//!
//! Zero meshes, zero child entities. Just add `DebugAxes` to any entity.
//!
//! ## Setup
//! ```rust
//! // main.rs
//! mod debug_axes;
//! use debug_axes::DebugAxesPlugin;
//!
//! app.add_plugins(DebugAxesPlugin);
//! ```
//!
//! ## Usage
//! ```rust
//! commands.spawn((
//!     Transform::default(),
//!     GlobalTransform::default(),
//!     DebugAxes::default(),      // length = 1.0
//!     // DebugAxes::new(0.5),    // custom length
//! ));
//! ```
//!
//! ## Toggle all axes at once
//! ```rust
//! fn my_system(mut config: ResMut<DebugAxesConfig>) {
//!     config.enabled = false;
//! }
//! ```

use bevy::prelude::*;

/// Add to any entity to show X (red) / Y (green) / Z (blue) arrows.
#[derive(Component, Clone, Copy, Debug)]
pub struct DebugAxes {
    pub length: f32,
}

impl DebugAxes {
    pub fn new(length: f32) -> Self {
        Self { length }
    }
}

impl Default for DebugAxes {
    fn default() -> Self {
        Self { length: 1.0 }
    }
}

/// Global toggle. Defaults to `true` (visible).
#[derive(Resource)]
pub struct DebugAxesConfig {
    pub enabled: bool,
}

impl Default for DebugAxesConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

pub struct DebugAxesPlugin;

impl Plugin for DebugAxesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DebugAxesConfig>()
            .add_systems(Update, draw_axes);
    }
}

fn draw_axes(
    mut gizmos: Gizmos,
    query: Query<(&GlobalTransform, &DebugAxes)>,
    config: Res<DebugAxesConfig>,
) {
    if !config.enabled {
        return;
    }
    for (gt, axes) in &query {
        let origin = gt.translation();
        let rot = gt.to_scale_rotation_translation().1;
        let len = axes.length;

        gizmos.arrow(
            origin,
            origin + rot * Vec3::X * len,
            Color::srgb(1.0, 0.15, 0.15),
        );
        gizmos.arrow(
            origin,
            origin + rot * Vec3::Y * len,
            Color::srgb(0.15, 1.0, 0.15),
        );
        gizmos.arrow(
            origin,
            origin + rot * Vec3::Z * len,
            Color::srgb(0.15, 0.45, 1.0),
        );
    }
}
