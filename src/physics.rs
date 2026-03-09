// Core physics engine designed specifically for ballistics

use bevy::prelude::*;

use crate::{Velocity, entities::missile::Missile, plugins::debug_hud::DebugHud};

pub mod georeference;

pub fn air_density_at_altitude(altitude: f32) -> f32 {
    let p0 = 1.225; // kg/m^3, sea level air density
    let scale_limit = 8_500.0; // m

    // p0 * e^(-h/H)
    return p0 * ((-altitude) / scale_limit).exp();
}

pub fn drag_force(
    altitude: f32,
    velocity: Vec3,
    drag_coefficient: f32,
    cross_section_area: f32,
) -> f32 {
    let air_density = air_density_at_altitude(altitude);

    // Fd = 1/2 * p * v^2 * C_d * A
    return 0.5 * air_density * velocity.length_squared() * drag_coefficient * cross_section_area;
}

pub fn update_physics(
    missile: Single<(&Transform, &mut Velocity), With<Missile>>,
    mut hud: Single<&mut Text, With<DebugHud>>,
    time: Res<Time>,
) {
    let (transform, mut velocity) = missile.into_inner();

    let drag_coefficient = 0.5;
    let cross_section_area = 2.0; // m^2
    let mass = 25.0; // kg

    let pos = transform.translation;
    let altitude = -pos.y; // convert to real-world altitude

    // Gravity pulls in +Y
    let gravity_force = Vec3::new(0.0, 9.81 * mass, 0.0);

    // Thrust Pushes towards the nose
    let thrust_dir = transform.rotation * Vec3::Z;
    let thrust_force = thrust_dir * 100.0;

    // Drag opposes velocity direction
    let drag_magnitude = drag_force(altitude, velocity.0, drag_coefficient, cross_section_area);
    let drag_force_vec = if velocity.0.length() > 0.0 {
        -velocity.0.normalize() * drag_magnitude
    } else {
        Vec3::ZERO
    };

    let net_force = thrust_force + gravity_force + drag_force_vec;
    let acceleration = net_force / mass;
    velocity.0 += acceleration * time.delta_secs();

    hud.0 = format!(
        "Position:     {:.1} {:.1} {:.1}\n\
         Altitude:     {:.1} m\n\
         Velocity:     {:.1} {:.1} {:.1} ({:.1} m/s)\n\
         Thrust:       {:.1} N\n\
         Drag:         {:.1} N\n\
         Gravity:      {:.1} N\n\
         Net Force:    {:.1} {:.1} {:.1}\n\
         Acceleration: {:.1} {:.1} {:.1} m/s/s",
        pos.x,
        pos.y,
        pos.z,
        altitude,
        velocity.0.x,
        velocity.0.y,
        velocity.0.z,
        velocity.0.length(),
        thrust_force.length(),
        drag_magnitude,
        gravity_force.y,
        net_force.x,
        net_force.y,
        net_force.z,
        acceleration.x,
        acceleration.y,
        acceleration.z,
    );
}
