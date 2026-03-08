// Core loop and sequences

use bevy::prelude::*;

use crate::entities::missile::Missile;

// Main loop for controlling the missile
pub fn update_missile(missile: Single<&Transform, With<Missile>>) {}

pub fn launch_missile(missile: Single<&Transform, With<Missile>>) {}
