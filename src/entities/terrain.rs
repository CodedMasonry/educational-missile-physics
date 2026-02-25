// terrain.rs — curvature-aware LTP terrain generation

use bevy::{
    asset::RenderAssetUsages,
    light::{NotShadowCaster, NotShadowReceiver},
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task, futures_lite::future},
};
use noise::{NoiseFn, Perlin};
use std::collections::HashMap;

use crate::entities::missile::Missile;
use crate::physics::georeference::{self, LLA};

// ── tunables ─────────────────────────────────────────────────────────────────

const CHUNK_SIZE: f32 = 512.0;
const CONTOUR_SUBDIVISIONS: u32 = 64;
const CONTOUR_LEVELS: u32 = 20;
const VIEW_DISTANCE: i32 = 6;
const DESPAWN_MARGIN: i32 = 1;

// ── resources ─────────────────────────────────────────────────────────────────

#[derive(Resource)]
pub struct SimulationOrigin {
    pub origin: LLA,
}

impl Default for SimulationOrigin {
    fn default() -> Self {
        Self {
            origin: LLA::new(0.0, 0.0, 0.0),
        }
    }
}

#[derive(Resource)]
pub struct TerrainConfig {
    pub noise: Perlin,
    pub warp_noise: Perlin,
    pub detail_noise: Perlin,
    pub height_scale: f32,
    pub noise_scale: f32,
    pub height_min: f32,
    pub height_max: f32,
    /// Origin snapshot baked in at generation time so async tasks don't need
    /// to access ECS resources.
    pub origin: LLA,
}

impl Default for TerrainConfig {
    fn default() -> Self {
        let height_scale = 120.0_f32;
        Self {
            noise: Perlin::new(2026),
            warp_noise: Perlin::new(1337),
            detail_noise: Perlin::new(9999),
            height_scale,
            noise_scale: 0.0002,
            height_min: -0.3 * height_scale,
            height_max: 1.6 * height_scale,
            origin: LLA::new(40.8295, -93.3725, 267.0),
        }
    }
}

#[derive(Resource, Default)]
pub struct LoadedChunks {
    pub chunks: HashMap<(i32, i32), Entity>,
    pending: HashMap<(i32, i32), Task<ChunkData>>,
}

/// CPU-side heightfield for missile collision queries.
/// Heights are in sim-space Y (curvature-corrected + Perlin offset) and can be
/// directly compared against a missile's `translation.y`.
#[derive(Resource, Default)]
pub struct HeightCache {
    chunks: HashMap<(i32, i32), Vec<f32>>,
}

const CACHE_VERTS: u32 = 32;

impl HeightCache {
    pub fn insert(&mut self, coord: (i32, i32), heights: Vec<f32>) {
        self.chunks.insert(coord, heights);
    }

    /// Bilinearly interpolated sim-space Y at world position (wx, wz).
    pub fn sample(&self, wx: f32, wz: f32) -> Option<f32> {
        let cx = (wx / CHUNK_SIZE).floor() as i32;
        let cz = (wz / CHUNK_SIZE).floor() as i32;
        let heights = self.chunks.get(&(cx, cz))?;

        let step = CHUNK_SIZE / (CACHE_VERTS - 1) as f32;
        let local_x = wx - cx as f32 * CHUNK_SIZE;
        let local_z = wz - cz as f32 * CHUNK_SIZE;

        let xi = (local_x / step).floor() as u32;
        let zi = (local_z / step).floor() as u32;
        let xi = xi.min(CACHE_VERTS - 2);
        let zi = zi.min(CACHE_VERTS - 2);

        let tx = (local_x / step) - xi as f32;
        let tz = (local_z / step) - zi as f32;

        let idx = |x: u32, z: u32| (z * CACHE_VERTS + x) as usize;
        let h00 = heights[idx(xi, zi)];
        let h10 = heights[idx(xi + 1, zi)];
        let h01 = heights[idx(xi, zi + 1)];
        let h11 = heights[idx(xi + 1, zi + 1)];

        Some(
            h00 * (1.0 - tx) * (1.0 - tz)
                + h10 * tx * (1.0 - tz)
                + h01 * (1.0 - tx) * tz
                + h11 * tx * tz,
        )
    }
}

// ── components ────────────────────────────────────────────────────────────────

#[derive(Component)]
pub struct TerrainChunk {
    pub coord: (i32, i32),
}

// ── noise helpers ─────────────────────────────────────────────────────────────

#[inline]
fn ridged(noise: &Perlin, x: f64, z: f64) -> f64 {
    1.0 - noise.get([x, z]).abs()
}

/// Procedural height offset (meters) above the ellipsoid surface at sim-space (x, z).
/// Does NOT include the curvature offset — add to `ellipsoid_surface_y` for the
/// full sim-space Y.
fn sample_perlin_height(x: f32, z: f32, config: &TerrainConfig) -> f32 {
    let s = config.noise_scale as f64;
    let xd = x as f64;
    let zd = z as f64;

    let warp_strength = 400.0_f64;
    let ws = s * 0.5;
    let wx = config.warp_noise.get([xd * ws, zd * ws]) * warp_strength
        + config.warp_noise.get([xd * ws * 2.0, zd * ws * 2.0]) * warp_strength * 0.5;
    let wz = config.warp_noise.get([xd * ws + 3.7, zd * ws + 1.3]) * warp_strength
        + config
            .warp_noise
            .get([xd * ws * 2.0 + 3.7, zd * ws * 2.0 + 1.3])
            * warp_strength
            * 0.5;

    let sx = xd * s + wx * s;
    let sz = zd * s + wz * s;

    let h0 = ridged(&config.noise, sx, sz);
    let h1 = ridged(&config.noise, sx * 2.0, sz * 2.0) * 0.50;
    let h2 = ridged(&config.noise, sx * 4.0, sz * 4.0) * 0.25;
    let h3 = ridged(&config.noise, sx * 8.0, sz * 8.0) * 0.13;
    let ridge_base = h0 + h0 * h1 + h0 * h1 * h2 + h3;

    let eps = s * 2.0;
    let dh_x = (config.noise.get([sx + eps, sz]) - config.noise.get([sx - eps, sz])) / (2.0 * eps);
    let dh_z = (config.noise.get([sx, sz + eps]) - config.noise.get([sx, sz - eps])) / (2.0 * eps);
    let gradient_mag = (dh_x * dh_x + dh_z * dh_z).sqrt() as f32;
    let carving_depth = (1.0 - gradient_mag.clamp(0.0, 1.0)).powf(2.0) * 0.25;

    let detail = config.detail_noise.get([xd * s * 12.0, zd * s * 12.0]) as f32 * 0.04;

    let h = ridge_base as f32 - carving_depth + detail;
    h * config.height_scale
}

/// Sim-space Y of the ellipsoid surface (altitude = 0) at (x, z).
///
/// The LTP tangent plane (Y = 0) is only tangent to the ellipsoid at the origin;
/// everywhere else the ellipsoid curves below it. This function computes that dip
/// by converting (x, 0, z) to LLA, forcing alt = 0, then converting back to sim.
fn ellipsoid_surface_y(x: f32, z: f32, origin: LLA) -> f32 {
    use bevy::math::DVec3;

    let sim_guess = DVec3::new(x as f64, 0.0, z as f64);
    let lla_surface = georeference::sim_to_lla(sim_guess, origin);

    let lla_on_ellipsoid = LLA::new(lla_surface.lat, lla_surface.long, 0.0);
    let sim_on_ellipsoid = georeference::lla_to_sim(lla_on_ellipsoid, origin);

    sim_on_ellipsoid.y as f32
}

/// Full sim-space height at (x, z): ellipsoid surface + procedural terrain.
/// Use this value in HeightCache and for all collision tests.
pub fn sample_height(x: f32, z: f32, config: &TerrainConfig) -> f32 {
    let base_y = ellipsoid_surface_y(x, z, config.origin);
    let perlin_offset = sample_perlin_height(x, z, config);
    base_y + perlin_offset
}

// ── background chunk generation ───────────────────────────────────────────────

struct ChunkData {
    #[allow(unused)]
    coord: (i32, i32),
    contour_mesh: Mesh,
    fill_mesh: Mesh,
    /// Heights in sim-space Y (curvature-corrected + Perlin offset).
    heights: Vec<f32>,
}

const FILL_SUBDIVISIONS: u32 = 32;

fn generate_chunk(coord: (i32, i32), config: &TerrainConfig) -> ChunkData {
    let (chunk_x, chunk_z) = coord;
    let world_ox = chunk_x as f32 * CHUNK_SIZE;
    let world_oz = chunk_z as f32 * CHUNK_SIZE;
    let n = CONTOUR_SUBDIVISIONS + 1;

    // Sample the full (curvature + Perlin) height grid.
    let mut grid = vec![0.0_f32; (n * n) as usize];
    for zi in 0..n {
        for xi in 0..n {
            let wx = world_ox + xi as f32 * CHUNK_SIZE / CONTOUR_SUBDIVISIONS as f32;
            let wz = world_oz + zi as f32 * CHUNK_SIZE / CONTOUR_SUBDIVISIONS as f32;
            grid[(zi * n + xi) as usize] = sample_height(wx, wz, config);
        }
    }

    // ── contour lines (marching squares) ─────────────────────────────────────
    let mut contour_positions: Vec<[f32; 3]> = Vec::new();
    let h_range = config.height_max - config.height_min;

    for level_i in 0..CONTOUR_LEVELS {
        let level = config.height_min + h_range * (level_i as f32 + 0.5) / CONTOUR_LEVELS as f32;

        for zi in 0..CONTOUR_SUBDIVISIONS {
            for xi in 0..CONTOUR_SUBDIVISIONS {
                let step = CHUNK_SIZE / CONTOUR_SUBDIVISIONS as f32;
                let lx = xi as f32 * step;
                let lz = zi as f32 * step;

                let idx = |x: u32, z: u32| (z * n + x) as usize;
                let h00 = grid[idx(xi, zi)];
                let h10 = grid[idx(xi + 1, zi)];
                let h01 = grid[idx(xi, zi + 1)];
                let h11 = grid[idx(xi + 1, zi + 1)];

                let mut pts: [Option<[f32; 3]>; 4] = [None; 4];
                let mut pt_count = 0;

                // Vertex Y is the full sim-space height from grid[] so rendered
                // contours match the collision cache. The chunk entity sits at
                // Transform::from_xyz(world_x, 0, world_z), so XZ is chunk-local.
                if (h00 - level).signum() != (h10 - level).signum() {
                    let t = (level - h00) / (h10 - h00);
                    pts[pt_count] = Some([lx + t * step, h00 + t * (h10 - h00), lz]);
                    pt_count += 1;
                }
                if (h10 - level).signum() != (h11 - level).signum() {
                    let t = (level - h10) / (h11 - h10);
                    pts[pt_count] = Some([lx + step, h10 + t * (h11 - h10), lz + t * step]);
                    pt_count += 1;
                }
                if (h01 - level).signum() != (h11 - level).signum() {
                    let t = (level - h01) / (h11 - h01);
                    pts[pt_count] = Some([lx + t * step, h01 + t * (h11 - h01), lz + step]);
                    pt_count += 1;
                }
                if (h00 - level).signum() != (h01 - level).signum() {
                    let t = (level - h00) / (h01 - h00);
                    pts[pt_count] = Some([lx, h00 + t * (h01 - h00), lz + t * step]);
                    pt_count += 1;
                }

                if pt_count >= 2 {
                    contour_positions.push(pts[0].unwrap());
                    contour_positions.push(pts[1].unwrap());
                }
                if pt_count == 4 {
                    contour_positions.push(pts[2].unwrap());
                    contour_positions.push(pts[3].unwrap());
                }
            }
        }
    }

    // Color by Perlin-only height (subtract curvature base) so the ramp is
    // consistent across all chunks regardless of distance from the origin.
    let contour_colors: Vec<[f32; 4]> = contour_positions
        .iter()
        .map(|p| {
            let cx_approx = world_ox + p[0];
            let cz_approx = world_oz + p[2];
            let base_y = ellipsoid_surface_y(cx_approx, cz_approx, config.origin);
            let perlin_h = p[1] - base_y;
            let t = ((perlin_h - config.height_min) / h_range).clamp(0.0, 1.0);
            if t < 0.5 {
                let s = t * 2.0;
                [0.0 + s * 0.6, 0.6 + s * 0.3, 0.4 - s * 0.35, 1.0]
            } else {
                let s = (t - 0.5) * 2.0;
                [0.6 + s * 0.4, 0.9 - s * 0.1, 0.05 + s * 0.45, 1.0]
            }
        })
        .collect();

    let mut contour_mesh = Mesh::new(PrimitiveTopology::LineList, RenderAssetUsages::RENDER_WORLD);
    contour_mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, contour_positions);
    contour_mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, contour_colors);

    // ── solid fill mesh ───────────────────────────────────────────────────────
    let fv = FILL_SUBDIVISIONS + 1;
    let f_step = CHUNK_SIZE / FILL_SUBDIVISIONS as f32;

    let sample_grid = |xi: u32, zi: u32| -> f32 {
        let gx = xi as f32 * (CONTOUR_SUBDIVISIONS as f32 / FILL_SUBDIVISIONS as f32);
        let gz = zi as f32 * (CONTOUR_SUBDIVISIONS as f32 / FILL_SUBDIVISIONS as f32);
        let gxi = (gx.floor() as u32).min(CONTOUR_SUBDIVISIONS - 1);
        let gzi = (gz.floor() as u32).min(CONTOUR_SUBDIVISIONS - 1);
        let tx = gx - gxi as f32;
        let tz = gz - gzi as f32;
        let idx = |x: u32, z: u32| (z * n + x) as usize;
        let h00 = grid[idx(gxi, gzi)];
        let h10 = grid[idx(gxi + 1, gzi)];
        let h01 = grid[idx(gxi, gzi + 1)];
        let h11 = grid[idx(gxi + 1, gzi + 1)];
        h00 * (1.0 - tx) * (1.0 - tz)
            + h10 * tx * (1.0 - tz)
            + h01 * (1.0 - tx) * tz
            + h11 * tx * tz
    };

    let mut fill_positions: Vec<[f32; 3]> = Vec::with_capacity((fv * fv) as usize);
    let mut fill_colors: Vec<[f32; 4]> = Vec::with_capacity((fv * fv) as usize);
    let mut fill_indices: Vec<u32> =
        Vec::with_capacity((FILL_SUBDIVISIONS * FILL_SUBDIVISIONS * 6) as usize);

    let sun = Vec3::new(-1.0, 2.0, -1.0).normalize();

    for zi in 0..fv {
        for xi in 0..fv {
            let h = sample_grid(xi, zi);
            fill_positions.push([xi as f32 * f_step, h, zi as f32 * f_step]);

            let xl = if xi > 0 { sample_grid(xi - 1, zi) } else { h };
            let xr = if xi < fv - 1 {
                sample_grid(xi + 1, zi)
            } else {
                h
            };
            let zd = if zi > 0 { sample_grid(xi, zi - 1) } else { h };
            let zu = if zi < fv - 1 {
                sample_grid(xi, zi + 1)
            } else {
                h
            };
            let normal = Vec3::new(xl - xr, 2.0 * f_step, zd - zu).normalize();
            let shade = normal.dot(sun).clamp(0.08, 1.0);

            // Color by Perlin offset so the ramp is consistent across chunks.
            let base_y = ellipsoid_surface_y(
                world_ox + xi as f32 * f_step,
                world_oz + zi as f32 * f_step,
                config.origin,
            );
            let perlin_h = h - base_y;
            let t = ((perlin_h - config.height_min) / h_range).clamp(0.0, 1.0);
            let (base_r, base_g, base_b) = if t < 0.5 {
                let s = t * 2.0;
                (0.05 + s * 0.10, 0.12 + s * 0.10, 0.10 + s * 0.02)
            } else {
                let s = (t - 0.5) * 2.0;
                (0.15 + s * 0.25, 0.22 + s * 0.18, 0.12 - s * 0.04)
            };

            fill_colors.push([
                (base_r * shade).clamp(0.0, 1.0),
                (base_g * shade).clamp(0.0, 1.0),
                (base_b * shade).clamp(0.0, 1.0),
                1.0,
            ]);
        }
    }

    for zi in 0..FILL_SUBDIVISIONS {
        for xi in 0..FILL_SUBDIVISIONS {
            let i = zi * fv + xi;
            fill_indices.push(i);
            fill_indices.push(i + fv);
            fill_indices.push(i + 1);
            fill_indices.push(i + 1);
            fill_indices.push(i + fv);
            fill_indices.push(i + fv + 1);
        }
    }

    let mut fill_mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    );
    fill_mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, fill_positions);
    fill_mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, fill_colors);
    fill_mesh.insert_indices(Indices::U32(fill_indices));

    // ── collision heightfield ─────────────────────────────────────────────────
    let cv = CACHE_VERTS;
    let mut heights = Vec::with_capacity((cv * cv) as usize);
    for zi in 0..cv {
        for xi in 0..cv {
            let wx = world_ox + xi as f32 * CHUNK_SIZE / (cv - 1) as f32;
            let wz = world_oz + zi as f32 * CHUNK_SIZE / (cv - 1) as f32;
            heights.push(sample_height(wx, wz, config));
        }
    }

    ChunkData {
        coord,
        contour_mesh,
        fill_mesh,
        heights,
    }
}

// ── main system ───────────────────────────────────────────────────────────────

pub fn update_terrain_chunks(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut loaded_chunks: ResMut<LoadedChunks>,
    mut height_cache: ResMut<HeightCache>,
    config: Res<TerrainConfig>,
    sim_origin: Res<SimulationOrigin>,
    viewer: Single<&Transform, With<Missile>>,
) {
    let viewer_pos = viewer.translation;

    let cx = (viewer_pos.x / CHUNK_SIZE).floor() as i32;
    let cz = (viewer_pos.z / CHUNK_SIZE).floor() as i32;

    let task_pool = AsyncComputeTaskPool::get();

    for dz in -VIEW_DISTANCE..=VIEW_DISTANCE {
        for dx in -VIEW_DISTANCE..=VIEW_DISTANCE {
            let coord = (cx + dx, cz + dz);
            if loaded_chunks.chunks.contains_key(&coord)
                || loaded_chunks.pending.contains_key(&coord)
            {
                continue;
            }

            let cfg_clone = TerrainConfig {
                noise: config.noise,
                warp_noise: config.warp_noise,
                detail_noise: config.detail_noise,
                height_scale: config.height_scale,
                noise_scale: config.noise_scale,
                height_min: config.height_min,
                height_max: config.height_max,
                origin: sim_origin.origin,
            };

            let task = task_pool.spawn(async move { generate_chunk(coord, &cfg_clone) });
            loaded_chunks.pending.insert(coord, task);
        }
    }

    let mut ready = Vec::new();
    for (coord, task) in &mut loaded_chunks.pending {
        if let Some(data) = future::block_on(future::poll_once(task)) {
            ready.push((*coord, data));
        }
    }

    for (coord, data) in ready {
        loaded_chunks.pending.remove(&coord);
        height_cache.insert(coord, data.heights);

        let world_x = coord.0 as f32 * CHUNK_SIZE;
        let world_z = coord.1 as f32 * CHUNK_SIZE;

        let fill_mat = materials.add(StandardMaterial {
            base_color: Color::WHITE,
            unlit: true,
            ..default()
        });
        let contour_mat = materials.add(StandardMaterial {
            base_color: Color::WHITE,
            unlit: true,
            ..default()
        });

        let entity = commands
            .spawn((
                Mesh3d(meshes.add(data.fill_mesh)),
                MeshMaterial3d(fill_mat),
                Transform::from_xyz(world_x, 0.0, world_z),
                TerrainChunk { coord },
                NotShadowCaster,
                NotShadowReceiver,
            ))
            .with_children(|parent| {
                parent.spawn((
                    Mesh3d(meshes.add(data.contour_mesh)),
                    MeshMaterial3d(contour_mat),
                    Transform::from_xyz(0.0, 0.5, 0.0),
                    NotShadowCaster,
                    NotShadowReceiver,
                ));
            })
            .id();

        loaded_chunks.chunks.insert(coord, entity);
    }

    let limit = VIEW_DISTANCE + DESPAWN_MARGIN;
    loaded_chunks.chunks.retain(|&(x, z), &mut entity| {
        let in_range = (x - cx).abs() <= limit && (z - cz).abs() <= limit;
        if !in_range {
            commands.entity(entity).despawn();
            height_cache.chunks.remove(&(x, z));
        }
        in_range
    });
    loaded_chunks
        .pending
        .retain(|&(x, z), _| (x - cx).abs() <= limit && (z - cz).abs() <= limit);
}

// ── collision helpers ─────────────────────────────────────────────────────────

/// Returns the sim-space Y of the terrain surface below `pos`.
/// Compare against `pos.y` to detect ground impact.
pub fn terrain_height_at(pos: Vec3, cache: &HeightCache) -> Option<f32> {
    cache.sample(pos.x, pos.z)
}

/// Returns the sim-space Y of the terrain surface at a geographic coordinate.
pub fn terrain_height_at_lla(
    point: LLA,
    origin: &SimulationOrigin,
    cache: &HeightCache,
) -> Option<f32> {
    let sim_pos = georeference::lla_to_sim(point, origin.origin);
    cache.sample(sim_pos.x as f32, sim_pos.z as f32)
}

// ── plugin ────────────────────────────────────────────────────────────────────

pub struct TerrainPlugin;

impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TerrainConfig>()
            .init_resource::<SimulationOrigin>()
            .init_resource::<LoadedChunks>()
            .init_resource::<HeightCache>()
            .add_systems(Update, update_terrain_chunks);
    }
}
