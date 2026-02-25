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

// -----------------------------------------------------------------------------
// LLA → SIM functions
// -----------------------------------------------------------------------------

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

/// LLA → Sim position (relative to origin)
pub fn lla_to_sim(point: LLA, origin: LLA) -> DVec3 {
    let delta = lla_to_ecef(point) - lla_to_ecef(origin);
    ecef_delta_to_sim(delta, origin)
}

// -----------------------------------------------------------------------------
// SIM → LLA functions
// -----------------------------------------------------------------------------

/// Simulation space → ECEF Δ
pub fn sim_to_ecef_delta(sim_pos: DVec3, origin: LLA) -> DVec3 {
    let phi = origin.lat.to_radians();
    let lam = origin.long.to_radians();

    let (sin_phi, cos_phi) = phi.sin_cos();
    let (sin_lam, cos_lam) = lam.sin_cos();

    // This is the transpose of the matrix used in ecef_delta_to_sim
    DVec3::new(
        -sin_lam * sim_pos.x + cos_phi * cos_lam * sim_pos.y + sin_phi * cos_lam * sim_pos.z,
        cos_lam * sim_pos.x + cos_phi * sin_lam * sim_pos.y + sin_phi * sin_lam * sim_pos.z,
        0.0 * sim_pos.x + sin_phi * sim_pos.y - cos_phi * sim_pos.z,
    )
}

/// ECEF (meters) → LLA
pub fn ecef_to_lla(ecef: DVec3) -> LLA {
    let x = ecef.x;
    let y = ecef.y;
    let z = ecef.z;

    let long = y.atan2(x);
    let p = (x.powi(2) + y.powi(2)).sqrt();

    // Initial guess for latitude
    let mut lat = z.atan2(p * (1.0 - ECCENTRICITY_SQUARED));
    let mut alt = 0.0;
    let mut n;

    // Iterative refinement (usually converges in 3-5 iterations)
    for _ in 0..5 {
        n = prime_vertical_radius(lat);
        alt = (p / lat.cos()) - n;
        lat = z.atan2(p * (1.0 - ECCENTRICITY_SQUARED * (n / (n + alt))));
    }

    LLA::new(lat.to_degrees(), long.to_degrees(), alt)
}

/// Sim position → LLA (relative to origin)
pub fn sim_to_lla(sim_pos: DVec3, origin: LLA) -> LLA {
    let delta_ecef = sim_to_ecef_delta(sim_pos, origin);
    let origin_ecef = lla_to_ecef(origin);
    let point_ecef = origin_ecef + delta_ecef;

    ecef_to_lla(point_ecef)
}
