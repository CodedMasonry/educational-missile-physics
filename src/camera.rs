use std::{f32::consts::FRAC_PI_2, ops::Range};

use bevy::{
    input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll},
    prelude::*,
};

use crate::missile::Missile;

#[derive(Debug, Resource)]
pub struct CameraSettings {
    pub orbit_distance: f32,
    pub pitch_speed: f32,
    pub pitch_range: Range<f32>,
    pub yaw_speed: f32,
    pub zoom_speed: f32,
}

impl Default for CameraSettings {
    fn default() -> Self {
        // Limiting pitch stops some unexpected rotation past 90° up or down.
        let pitch_limit = FRAC_PI_2 - 0.01;
        Self {
            // These values are completely arbitrary, chosen because they seem to produce
            // "sensible" results for this example. Adjust as required.
            orbit_distance: 100.0,
            pitch_speed: 0.25,
            pitch_range: -pitch_limit..pitch_limit,
            yaw_speed: 0.25,
            zoom_speed: 10.0,
        }
    }
}

pub fn orbit(
    mut camera: Single<&mut Transform, (With<Camera>, Without<Missile>)>,
    missile: Single<&Transform, (With<Missile>, Without<Camera>)>,
    mut camera_settings: ResMut<CameraSettings>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    mouse_scroll: Res<AccumulatedMouseScroll>,
    time: Res<Time>,
) {
    let delta = mouse_motion.delta;

    // pitch & yaw when left clicking
    if mouse_buttons.pressed(MouseButton::Left) {
        // Factor in delta time for mouse button inputs.
        let delta_pitch = delta.y * camera_settings.pitch_speed * time.delta_secs();
        let delta_yaw = delta.x * camera_settings.yaw_speed * time.delta_secs();

        // Obtain the existing pitch, yaw, and roll values from the transform.
        let (yaw, pitch, roll) = camera.rotation.to_euler(EulerRot::YXZ);

        // Establish the new yaw and pitch, preventing the pitch value from exceeding our limits.
        let pitch = (pitch + delta_pitch).clamp(
            camera_settings.pitch_range.start,
            camera_settings.pitch_range.end,
        );
        let yaw = yaw + delta_yaw;
        camera.rotation = Quat::from_euler(EulerRot::YXZ, yaw, pitch, roll);
    }

    // zoom with scroll wheel
    camera_settings.orbit_distance -= mouse_scroll.delta.y * camera_settings.zoom_speed;

    // Adjust the translation to maintain the correct orientation toward the orbit target.
    // In our example it's a static target, but this could easily be customized.
    let target = missile.translation;
    camera.translation = target - camera.forward() * camera_settings.orbit_distance;
}
