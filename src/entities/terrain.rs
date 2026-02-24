// terrain.rs — optimized for ballistic missile simulation
//
// KEY CHANGES FROM ORIGINAL:
//   1. TAA → disable at app level; use FXAA instead:
//        app.insert_resource(Msaa::Off)
//           .add_plugins(bevy::core_pipeline::experimental::taa::TemporalAntiAliasPlugin) // REMOVE THIS
//        In your CameraBundle, replace TemporalAntiAliasBundle with:
//           bevy_render::camera::TemporalJitter is gone; just add:
//           bevy::pbr::ScreenSpaceAmbientOcclusionBundle  // optional
//        Simplest fix: use FXAA on the camera component:
//           Camera { hdr: true, .. }, Fxaa { enabled: true, .. }
//
//   2. Chunk mesh → contour lines (LineList topology, ~10x fewer verts)
//   3. Async chunk generation via AsyncComputeTaskPool (no frame hitches)
//   4. Collision uses a separate lightweight HeightCache (pure CPU data)
//   5. Reduced VIEW_DISTANCE default; missile sim rarely needs wide terrain

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

// ── tunables ────────────────────────────────────────────────────────────────

const CHUNK_SIZE: f32 = 512.0;

/// Vertices per side for contour line sampling.
/// 64 gives smooth-enough contours; original 256 was for dense triangle fill.
const CONTOUR_SUBDIVISIONS: u32 = 64;

/// How many evenly-spaced height bands to draw contour lines at.
const CONTOUR_LEVELS: u32 = 20;

/// Chunks loaded in each cardinal direction around the viewer.
const VIEW_DISTANCE: i32 = 6;

/// Extra chunk margin kept alive beyond VIEW_DISTANCE before despawn.
const DESPAWN_MARGIN: i32 = 1;

// ── resources ───────────────────────────────────────────────────────────────

#[derive(Resource)]
pub struct TerrainConfig {
    /// Primary shape noise.
    pub noise: Perlin,
    /// Domain-warp noise — offsets sample coords to break grid symmetry
    /// and produce meandering ridges/valleys instead of uniform blobs.
    pub warp_noise: Perlin,
    /// Detail noise — fine-scale surface roughness on top of the base shape.
    pub detail_noise: Perlin,
    pub height_scale: f32,
    pub noise_scale: f32,
    /// Min/max height used to distribute CONTOUR_LEVELS evenly.
    pub height_min: f32,
    pub height_max: f32,
}

impl Default for TerrainConfig {
    fn default() -> Self {
        let height_scale = 120.0_f32;
        // Ridged noise output range is roughly [0, 1] after abs+invert,
        // combined octave sum ≈ [0, ~1.75]; valley carving subtracts a little.
        // Empirical min/max — adjust if terrain clips.
        Self {
            noise: Perlin::new(2026),
            warp_noise: Perlin::new(1337),
            detail_noise: Perlin::new(9999),
            height_scale,
            noise_scale: 0.0002,
            height_min: -0.3 * height_scale,
            height_max: 1.6 * height_scale,
        }
    }
}

#[derive(Resource, Default)]
pub struct LoadedChunks {
    /// Spawned and fully rendered chunks.
    pub chunks: HashMap<(i32, i32), Entity>,
    /// Chunks currently being generated on a background thread.
    pending: HashMap<(i32, i32), Task<ChunkData>>,
}

/// Cheap CPU-side heightfield used for missile collision queries.
/// Stored separately from the GPU mesh so we never read back from the GPU.
#[derive(Resource, Default)]
pub struct HeightCache {
    /// Maps chunk coord → flat array of heights, row-major, CACHE_VERTS × CACHE_VERTS.
    chunks: HashMap<(i32, i32), Vec<f32>>,
}

/// Resolution of the collision heightfield (independent of visual subdivisions).
const CACHE_VERTS: u32 = 32;

impl HeightCache {
    pub fn insert(&mut self, coord: (i32, i32), heights: Vec<f32>) {
        self.chunks.insert(coord, heights);
    }

    /// Bilinearly interpolated world-space height at (wx, wz).
    /// Returns None if the chunk isn't cached yet.
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

// ── components ──────────────────────────────────────────────────────────────

#[derive(Component)]
pub struct TerrainChunk {
    pub coord: (i32, i32),
}

// ── noise helpers ────────────────────────────────────────────────────────────

/// Ridged FBM octave — inverts the absolute value of Perlin noise so that
/// high values become sharp ridges and low values become wide flat basins.
/// This is the core of what makes terrain look eroded rather than blobby.
#[inline]
fn ridged(noise: &Perlin, x: f64, z: f64) -> f64 {
    1.0 - noise.get([x, z]).abs()
}

pub fn sample_height(x: f32, z: f32, config: &TerrainConfig) -> f32 {
    let s = config.noise_scale as f64;
    let xd = x as f64;
    let zd = z as f64;

    // ── Stage 1: domain warp ─────────────────────────────────────────────────
    // Warp the sample coordinates using a low-frequency noise field.
    // This breaks the radial symmetry of FBM and makes valleys meander
    // naturally rather than sitting in perfectly circular basins.
    let warp_strength = 400.0_f64; // world-unit displacement; tune to taste
    let ws = s * 0.5; // warp at half the base frequency so it's large-scale
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

    // ── Stage 2: ridged FBM base shape ──────────────────────────────────────
    // Ridged noise: valleys are wide and flat, ridges are sharp.
    // Octave weights chosen so detail falls off quickly — keeps the
    // large-scale shape dominant, which is how real terrain reads at distance.
    let h0 = ridged(&config.noise, sx, sz); // 1.000  large hills
    let h1 = ridged(&config.noise, sx * 2.0, sz * 2.0) * 0.50; // mid detail
    let h2 = ridged(&config.noise, sx * 4.0, sz * 4.0) * 0.25; // small bumps
    let h3 = ridged(&config.noise, sx * 8.0, sz * 8.0) * 0.13; // surface texture
    // Multiply successive octaves together (common trick) so ridge features
    // from coarser octaves sharpen the finer ones — prevents mush at peaks.
    let ridge_base = h0 + h0 * h1 + h0 * h1 * h2 + h3;

    // ── Stage 3: valley carving via gradient proxy ───────────────────────────
    // Real erosion deepens basins where flow accumulates.  We approximate
    // this cheaply: sample the gradient magnitude of the base shape.
    // High gradient = steep slope = ridge.  Low gradient = flat = basin.
    // We then subtract a "flow carving" term that is strongest in basins.
    let eps = s * 2.0;
    let dh_x = (config.noise.get([sx + eps, sz]) - config.noise.get([sx - eps, sz])) / (2.0 * eps);
    let dh_z = (config.noise.get([sx, sz + eps]) - config.noise.get([sx, sz - eps])) / (2.0 * eps);
    let gradient_mag = (dh_x * dh_x + dh_z * dh_z).sqrt() as f32;
    // carving_depth: strongest where gradient is near zero (flat basin centres).
    // Tuned so valleys sit ~15-30 units below surrounding terrain.
    let carving_depth = (1.0 - gradient_mag.clamp(0.0, 1.0)).powf(2.0) * 0.25;

    // ── Stage 4: fine detail ─────────────────────────────────────────────────
    // High-frequency detail noise adds surface roughness that reads as rocks /
    // ground texture at low altitude — completely invisible from high up.
    let detail = config.detail_noise.get([xd * s * 12.0, zd * s * 12.0]) as f32 * 0.04;

    let h = ridge_base as f32 - carving_depth + detail;
    h * config.height_scale
}

// ── background chunk data ────────────────────────────────────────────────────

/// Everything produced off-thread for one chunk.
struct ChunkData {
    #[allow(unused)]
    coord: (i32, i32),
    /// LineList contour mesh.
    contour_mesh: Mesh,
    /// Low-poly TriangleList fill mesh — gives terrain a solid floor.
    fill_mesh: Mesh,
    /// Collision heightfield (CACHE_VERTS² floats).
    heights: Vec<f32>,
}

/// Resolution of the solid fill mesh (vertices per side).
/// Much coarser than contours — just enough to show broad elevation shape.
const FILL_SUBDIVISIONS: u32 = 32;

/// Generates contour-line mesh + solid fill mesh + collision cache for one chunk.
/// Pure computation — no ECS access.
fn generate_chunk(coord: (i32, i32), config: &TerrainConfig) -> ChunkData {
    let (chunk_x, chunk_z) = coord;
    let world_ox = chunk_x as f32 * CHUNK_SIZE;
    let world_oz = chunk_z as f32 * CHUNK_SIZE;
    let n = CONTOUR_SUBDIVISIONS + 1;

    // ── sample the height grid (shared by contours + fill) ──────────────────
    let mut grid = vec![0.0_f32; (n * n) as usize];
    for zi in 0..n {
        for xi in 0..n {
            let wx = world_ox + xi as f32 * CHUNK_SIZE / CONTOUR_SUBDIVISIONS as f32;
            let wz = world_oz + zi as f32 * CHUNK_SIZE / CONTOUR_SUBDIVISIONS as f32;
            grid[(zi * n + xi) as usize] = sample_height(wx, wz, config);
        }
    }

    // ── contour line extraction (marching-squares) ───────────────────────────
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

    // Height-dependent contour colours — gives immediate visual depth cues:
    //   low  (t≈0): deep blue-green  [0.0, 0.6, 0.4]
    //   mid  (t≈0.5): yellow-green   [0.6, 0.9, 0.1]
    //   high (t≈1): bright white-red [1.0, 0.8, 0.5]
    // We stored world-Y in position[1], so we read it back here.
    let contour_colors: Vec<[f32; 4]> = contour_positions
        .iter()
        .map(|p| {
            let t = ((p[1] - config.height_min) / h_range).clamp(0.0, 1.0);
            // Lerp through three colour stops.
            if t < 0.5 {
                let s = t * 2.0;
                [
                    0.0 + s * 0.6,  // R
                    0.6 + s * 0.3,  // G
                    0.4 - s * 0.35, // B
                    1.0,
                ]
            } else {
                let s = (t - 0.5) * 2.0;
                [
                    0.6 + s * 0.4,   // R
                    0.9 - s * 0.1,   // G
                    0.05 + s * 0.45, // B
                    1.0,
                ]
            }
        })
        .collect();

    let mut contour_mesh = Mesh::new(PrimitiveTopology::LineList, RenderAssetUsages::RENDER_WORLD);
    contour_mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, contour_positions);
    contour_mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, contour_colors);

    // ── solid fill mesh (low-poly triangle grid) ─────────────────────────────
    // Uses FILL_SUBDIVISIONS (32) — coarser than contours, enough to match the
    // broad terrain shape without being expensive. Unlit flat-shaded dark green
    // sits behind the contour lines and gives the terrain a solid floor.
    let fv = FILL_SUBDIVISIONS + 1; // vertices per side
    let f_step = CHUNK_SIZE / FILL_SUBDIVISIONS as f32;

    // Sample heights at fill resolution by bilinear interpolation from the
    // contour grid — avoids redundant noise calls.
    let sample_grid = |xi: u32, zi: u32| -> f32 {
        // Map fill vertex to contour grid float coords.
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

    // Fixed sun direction for hillshading — NW high angle, classic topo map style.
    // Baked into vertex colours so it works at any camera angle, not view-dependent.
    let sun = Vec3::new(-1.0, 2.0, -1.0).normalize();

    for zi in 0..fv {
        for xi in 0..fv {
            let h = sample_grid(xi, zi);
            fill_positions.push([xi as f32 * f_step, h, zi as f32 * f_step]);

            // ── hillshade via finite-difference surface normal ───────────────
            // Clamp neighbours to grid edge so border vertices don't go OOB.
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
            // Central-difference normal: (-dh/dx, 2*step, -dh/dz) normalised.
            let normal = Vec3::new(xl - xr, 2.0 * f_step, zd - zu).normalize();
            // Lambert term — floor at 0.08 so shadowed faces stay readable.
            let shade = normal.dot(sun).clamp(0.08, 1.0);

            // ── height-based colour tint ─────────────────────────────────────
            // Three stops tuned to contrast with the contour colour scheme:
            //   low  (t≈0): dark blue-grey  — valley floor
            //   mid  (t≈0.5): muted olive   — hillside
            //   high (t≈1): pale stone      — ridge / peak
            let t = ((h - config.height_min) / h_range).clamp(0.0, 1.0);
            let (base_r, base_g, base_b) = if t < 0.5 {
                let s = t * 2.0;
                (0.05 + s * 0.10, 0.12 + s * 0.10, 0.10 + s * 0.02)
            } else {
                let s = (t - 0.5) * 2.0;
                (0.15 + s * 0.25, 0.22 + s * 0.18, 0.12 - s * 0.04)
            };

            // Multiply tint by hillshade — steep shadowed slopes go dark,
            // lit slopes brighten. This is the primary low-angle depth cue.
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
    // No normals — unlit material doesn't use them.

    // ── collision heightfield (coarser grid, CPU-only) ──────────────────────
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

// ── main system ──────────────────────────────────────────────────────────────

/// Kick off async generation for chunks entering view;
/// poll completed tasks and spawn entities; despawn distant chunks.
pub fn update_terrain_chunks(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut loaded_chunks: ResMut<LoadedChunks>,
    mut height_cache: ResMut<HeightCache>,
    config: Res<TerrainConfig>,
    viewer: Single<&Transform, With<Missile>>,
) {
    let cx = (viewer.translation.x / CHUNK_SIZE).floor() as i32;
    let cz = (viewer.translation.z / CHUNK_SIZE).floor() as i32;

    let task_pool = AsyncComputeTaskPool::get();

    // ── enqueue missing chunks ───────────────────────────────────────────────
    for dz in -VIEW_DISTANCE..=VIEW_DISTANCE {
        for dx in -VIEW_DISTANCE..=VIEW_DISTANCE {
            let coord = (cx + dx, cz + dz);
            if loaded_chunks.chunks.contains_key(&coord)
                || loaded_chunks.pending.contains_key(&coord)
            {
                continue;
            }

            // Clone only what the task needs (no Arc needed; Perlin is Copy).
            let cfg_clone = TerrainConfig {
                noise: config.noise,
                warp_noise: config.warp_noise,
                detail_noise: config.detail_noise,
                height_scale: config.height_scale,
                noise_scale: config.noise_scale,
                height_min: config.height_min,
                height_max: config.height_max,
            };

            let task = task_pool.spawn(async move { generate_chunk(coord, &cfg_clone) });

            loaded_chunks.pending.insert(coord, task);
        }
    }

    // ── poll completed tasks ─────────────────────────────────────────────────
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

        // Unlit materials — no lighting cost, no PBR overdraw.
        // vertex colours on the mesh are read automatically when ATTRIBUTE_COLOR
        // is present; there is no `vertex_colors` field in StandardMaterial.
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

        // Spawn fill mesh first so it renders behind the contour lines.
        // The contour mesh is a child so they move together and despawn together.
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
                    // Offset contour lines 0.5 units up to prevent z-fighting
                    // with the fill surface at the same elevation.
                    Transform::from_xyz(0.0, 0.5, 0.0),
                    NotShadowCaster,
                    NotShadowReceiver,
                ));
            })
            .id();

        loaded_chunks.chunks.insert(coord, entity);
    }

    // ── despawn distant chunks ───────────────────────────────────────────────
    let limit = VIEW_DISTANCE + DESPAWN_MARGIN;
    loaded_chunks.chunks.retain(|&(x, z), &mut entity| {
        let in_range = (x - cx).abs() <= limit && (z - cz).abs() <= limit;
        if !in_range {
            // In Bevy 0.16+, despawn() automatically despawns children.
            // despawn_recursive() was removed.
            commands.entity(entity).despawn();
            height_cache.chunks.remove(&(x, z));
        }
        in_range
    });
    loaded_chunks
        .pending
        .retain(|&(x, z), _| (x - cx).abs() <= limit && (z - cz).abs() <= limit);
}

// ── missile collision helper ─────────────────────────────────────────────────

/// Call from your missile update system to check terrain impact.
///
/// ```rust
/// if let Some(ground_y) = terrain_height_at(missile_pos, &height_cache) {
///     if missile_pos.y <= ground_y {
///         // impact!
///     }
/// }
/// ```
pub fn terrain_height_at(pos: Vec3, cache: &HeightCache) -> Option<f32> {
    cache.sample(pos.x, pos.z)
}

// ── plugin ───────────────────────────────────────────────────────────────────

pub struct TerrainPlugin;

impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TerrainConfig>()
            .init_resource::<LoadedChunks>()
            .init_resource::<HeightCache>()
            .add_systems(Update, update_terrain_chunks);
    }
}
