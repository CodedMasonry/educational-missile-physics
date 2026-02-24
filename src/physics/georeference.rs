// Georeference Library
//
// For this project `Earth-Centered, Earth-Fixed` is used.

use bevy::math::DVec3;

pub const SEMI_MAJOR_AXIS: f64 = 6378137.0;
pub const INVERSE_FLATTENING: f64 = 298.257223563;
pub const ECCENTRICITY_SQUARED: f64 = 0.00669437999014;

#[derive(Debug, Clone, Copy)]
pub struct LLA {
    /// Geodetic latitude in degrees
    pub lat: f64,
    /// Longitude in degrees
    pub long: f64,
    /// Altitude above WGS-84 ellipsoid in meters
    pub alt: f64,
}

impl LLA {
    /// latitude ( degrees), long (degrees), altitude (meters)
    pub fn new(lat: f64, long: f64, alt: f64) -> Self {
        Self { lat, long, alt }
    }
}

/// WGS-84 radius of curvature in the prime vertical
///
/// a / sqrt(1 - e² · sin²(φ))
fn prime_vertical_radius(lat_rad: f64) -> f64 {
    return SEMI_MAJOR_AXIS / (1.0 - (ECCENTRICITY_SQUARED * lat_rad.sin().powi(2))).sqrt();
}

/// LLA → ECEF  (DVec3: x, y, z in meters)
pub fn lla_to_ecef(lla: LLA) -> DVec3 {
    let lat = lla.lat.to_radians();
    let lon = lla.long.to_radians();
    let n = prime_vertical_radius(lat);

    DVec3::new(
        (n + lla.alt) * lat.cos() * lon.cos(),
        (n + lla.alt) * lat.cos() * lon.sin(),
        (n * (1.0 - ECCENTRICITY_SQUARED) + lla.alt) * lat.sin(),
    )
}

/// ECEF Δ → Y-up right-handed simulation space
///
///   sim X  =  East
///   sim Y  =  Up
///   sim Z  = -North
///
///   | sim_x | = | -sin(λ)          cos(λ)          0      |   | ΔX |
///
///   | sim_y | = |  cos(φ)cos(λ)   cos(φ)sin(λ)   sin(φ)  | · | ΔY |
///
///   | sim_z | = |  sin(φ)cos(λ)   sin(φ)sin(λ)  -cos(φ)  |   | ΔZ |
pub fn ecef_delta_to_sim(delta: DVec3, origin: LLA) -> DVec3 {
    let phi = origin.lat.to_radians();
    let lam = origin.long.to_radians();

    let (sin_phi, cos_phi) = phi.sin_cos();
    let (sin_lam, cos_lam) = lam.sin_cos();

    DVec3::new(
        -sin_lam * delta.x + cos_lam * delta.y,
        cos_phi * cos_lam * delta.x + cos_phi * sin_lam * delta.y + sin_phi * delta.z,
        sin_phi * cos_lam * delta.x + sin_phi * sin_lam * delta.y - cos_phi * delta.z,
    )
}

/// LLA → simulation space position (relative to origin)
pub fn lla_to_sim(point: LLA, origin: LLA) -> DVec3 {
    let delta = lla_to_ecef(point) - lla_to_ecef(origin);
    ecef_delta_to_sim(delta, origin)
}
