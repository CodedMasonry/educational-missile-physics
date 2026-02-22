use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use noise::{NoiseFn, Perlin};
use std::collections::HashMap;

use crate::missile::Missile;

const CHUNK_SIZE: f32 = 512.0;
const CHUNK_SUBDIVISIONS: u32 = 256; // vertices per side
const VIEW_DISTANCE: i32 = 6; // chunks in each direction

#[derive(Resource)]
pub struct TerrainConfig {
    pub noise: Perlin,
    pub height_scale: f32,
    pub noise_scale: f32,
}

impl Default for TerrainConfig {
    fn default() -> Self {
        Self {
            noise: Perlin::new(2026),
            height_scale: 150.0,
            noise_scale: 0.0002,
        }
    }
}

#[derive(Resource, Default)]
pub struct LoadedChunks {
    pub chunks: HashMap<(i32, i32), Entity>,
}

#[derive(Component)]
pub struct TerrainChunk {
    pub coord: (i32, i32),
}

#[derive(Resource)]
pub struct TerrainTextures {
    pub color: Handle<Image>,
    pub normal: Handle<Image>,
    pub roughness: Handle<Image>,
}

impl TerrainTextures {
    pub fn load(asset_server: &AssetServer) -> Self {
        let color_sampler = |settings: &mut bevy::image::ImageLoaderSettings| {
            settings.sampler =
                bevy::image::ImageSampler::Descriptor(bevy::image::ImageSamplerDescriptor {
                    address_mode_u: bevy::image::ImageAddressMode::Repeat,
                    address_mode_v: bevy::image::ImageAddressMode::Repeat,
                    mag_filter: bevy::image::ImageFilterMode::Linear,
                    min_filter: bevy::image::ImageFilterMode::Linear,
                    mipmap_filter: bevy::image::ImageFilterMode::Linear,
                    ..default()
                });
        };

        let linear_sampler = |settings: &mut bevy::image::ImageLoaderSettings| {
            settings.is_srgb = false; // roughness/normal must be linear, not sRGB
            settings.sampler =
                bevy::image::ImageSampler::Descriptor(bevy::image::ImageSamplerDescriptor {
                    address_mode_u: bevy::image::ImageAddressMode::Repeat,
                    address_mode_v: bevy::image::ImageAddressMode::Repeat,
                    mag_filter: bevy::image::ImageFilterMode::Linear,
                    min_filter: bevy::image::ImageFilterMode::Linear,
                    mipmap_filter: bevy::image::ImageFilterMode::Linear,
                    ..default()
                });
        };

        Self {
            color: asset_server.load_with_settings("textures/ground_color.png", color_sampler),
            normal: asset_server.load_with_settings("textures/ground_normal.png", linear_sampler),
            roughness: asset_server
                .load_with_settings("textures/ground_roughness.png", linear_sampler),
        }
    }
}

fn generate_chunk_mesh(chunk_x: i32, chunk_z: i32, config: &TerrainConfig) -> Mesh {
    let verts_per_side = CHUNK_SUBDIVISIONS + 1;
    let step = CHUNK_SIZE / CHUNK_SUBDIVISIONS as f32;

    let world_offset_x = chunk_x as f32 * CHUNK_SIZE;
    let world_offset_z = chunk_z as f32 * CHUNK_SIZE;

    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();

    for z in 0..verts_per_side {
        for x in 0..verts_per_side {
            let lx = x as f32 * step;
            let lz = z as f32 * step;
            let wx = world_offset_x + lx;
            let wz = world_offset_z + lz;

            let height = sample_height(wx, wz, config);

            positions.push([lx, height, lz]);
            uvs.push([lx / CHUNK_SIZE, lz / CHUNK_SIZE]);

            // Approximate normal using finite differences
            let h_l = sample_height(wx - 1.0, wz, config);
            let h_r = sample_height(wx + 1.0, wz, config);
            let h_d = sample_height(wx, wz - 1.0, config);
            let h_u = sample_height(wx, wz + 1.0, config);
            let normal = Vec3::new(h_l - h_r, 2.0, h_d - h_u).normalize();
            normals.push(normal.to_array());
        }
    }

    // Generate indices
    for z in 0..CHUNK_SUBDIVISIONS {
        for x in 0..CHUNK_SUBDIVISIONS {
            let i = z * verts_per_side + x;
            indices.push(i);
            indices.push(i + verts_per_side);
            indices.push(i + 1);
            indices.push(i + 1);
            indices.push(i + verts_per_side);
            indices.push(i + verts_per_side + 1);
        }
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

pub fn sample_height(x: f32, z: f32, config: &TerrainConfig) -> f32 {
    let s = config.noise_scale as f64;
    let h = config.noise.get([x as f64 * s, z as f64 * s])
        + 0.5 * config.noise.get([x as f64 * s * 2.0, z as f64 * s * 2.0])
        + 0.25 * config.noise.get([x as f64 * s * 4.0, z as f64 * s * 4.0])
        + 0.125 * config.noise.get([x as f64 * s * 8.0, z as f64 * s * 8.0])
        + 0.0625 * config.noise.get([x as f64 * s * 16.0, z as f64 * s * 16.0]);
    (h as f32) * config.height_scale
}

pub fn update_terrain_chunks(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut loaded_chunks: ResMut<LoadedChunks>,
    config: Res<TerrainConfig>,
    textures: Res<TerrainTextures>,
    viewer: Single<&Transform, With<Missile>>,
) {
    let cx = (viewer.translation.x / CHUNK_SIZE).floor() as i32;
    let cz = (viewer.translation.z / CHUNK_SIZE).floor() as i32;

    // Spawn chunks that should be visible
    for dz in -VIEW_DISTANCE..=VIEW_DISTANCE {
        for dx in -VIEW_DISTANCE..=VIEW_DISTANCE {
            let coord = (cx + dx, cz + dz);
            if loaded_chunks.chunks.contains_key(&coord) {
                continue;
            }

            let mesh = generate_chunk_mesh(coord.0, coord.1, &config);
            let world_x = coord.0 as f32 * CHUNK_SIZE;
            let world_z = coord.1 as f32 * CHUNK_SIZE;

            // spawn loop...
            let entity = commands
                .spawn((
                    Mesh3d(meshes.add(mesh)),
                    MeshMaterial3d(materials.add(StandardMaterial {
                        base_color_texture: Some(textures.color.clone()),
                        normal_map_texture: Some(textures.normal.clone()),
                        metallic_roughness_texture: Some(textures.roughness.clone()),
                        // Each chunk is 256 units, tile every ~32 units = 8 tiles per chunk
                        uv_transform: bevy::math::Affine2::from_scale(Vec2::splat(8.0)),
                        perceptual_roughness: 1.0,
                        metallic: 0.0,
                        reflectance: 0.1,
                        ..default()
                    })),
                    Transform::from_xyz(world_x, 0.0, world_z),
                    TerrainChunk { coord },
                ))
                .id();

            loaded_chunks.chunks.insert(coord, entity);
        }
    }

    // Despawn chunks that are too far away
    loaded_chunks.chunks.retain(|&(x, z), &mut entity| {
        let in_range = (x - cx).abs() <= VIEW_DISTANCE + 1 && (z - cz).abs() <= VIEW_DISTANCE + 1;
        if !in_range {
            commands.entity(entity).despawn();
        }
        in_range
    });
}
