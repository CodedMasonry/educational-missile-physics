// Core physics engine designed specifically for ballistics

use bevy::math::Vec3;

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

pub fn set_physics(pos: Vec3, velocity: Vec3) {
    // const
    let drag_coefficient = 0.5;
    let cross_section_area = 2.0; // m^2
    let mass = 25.0; // kg

    let thrust_force = 100.0; // newtons
    let drag_force = drag_force(pos.y, velocity, drag_coefficient, cross_section_area); // newtons
    let gravity_force = 9.81 * mass; // newtons
}
