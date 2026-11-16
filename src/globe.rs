//! The hollow brick globe: procedural curved brick meshes, layout, and fast spatial lookup.
//!
//! The shell is split into latitude bands. Each band holds `count` identical bricks, so a
//! single mesh per band is shared by all its bricks (automatic GPU instancing). Hit tests
//! never iterate bricks: a point in globe-local space maps straight to `(band, index)`.

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use bevy::{
    asset::RenderAssetUsages, mesh::Indices, prelude::*,
    render::render_resource::PrimitiveTopology,
};

pub const GLOBE_CENTER: Vec3 = Vec3::new(0.0, 2.1, 0.0);
/// Mid radius of the brick shell.
pub const GLOBE_R: f32 = 4.2;
pub const BRICK_THICK: f32 = 0.34;
pub const R_OUT: f32 = GLOBE_R + BRICK_THICK * 0.5;
pub const R_IN: f32 = GLOBE_R - BRICK_THICK * 0.5;
pub const BANDS: usize = 26;
pub const DLAT: f32 = PI / BANDS as f32;
/// Target edge length so bricks come out (nearly) square.
pub const BRICK_SIZE: f32 = GLOBE_R * DLAT;
/// Grout gap on each side of a brick.
const GAP: f32 = 0.028;
/// Chamfer on the outer edges — catches the light and makes every brick read crisply.
const BEVEL: f32 = 0.06;
pub const CORE_R: f32 = 0.85;

pub struct GlobePlugin;

impl Plugin for GlobePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Globe>()
            .add_systems(PreStartup, build_globe_assets);
    }
}

#[derive(Component)]
pub struct GlobeRoot;

#[derive(Component)]
pub struct Brick {
    pub mat: Handle<StandardMaterial>,
}

pub struct Band {
    pub count: usize,
    pub dlon: f32,
    /// Running-bond stagger (fraction of a brick).
    pub offset: f32,
    pub cells: Vec<Option<Entity>>,
}

#[derive(Resource, Default)]
pub struct Globe {
    pub root: Option<Entity>,
    pub bands: Vec<Band>,
    pub total: usize,
    pub remaining: usize,
}

#[derive(Resource)]
pub struct GlobeAssets {
    /// Per band: shared mesh and the brick centre for longitude 0.
    pub band_meshes: Vec<(Handle<Mesh>, Vec3)>,
    /// Per band: two shades for a subtle checker variation.
    pub band_mats: Vec<[Handle<StandardMaterial>; 2]>,
    pub highlight: Handle<StandardMaterial>,
}

pub enum Hit {
    Brick(usize, usize, Entity),
    Core,
}

#[inline]
pub fn sph(lat: f32, lon: f32, r: f32) -> Vec3 {
    let (sl, cl) = lat.sin_cos();
    let (so, co) = lon.sin_cos();
    // Matches Quat::from_rotation_y(lon) * sph(lat, 0, r).
    Vec3::new(r * cl * co, r * sl, -r * cl * so)
}

fn band_count(i: usize) -> usize {
    let latc = -FRAC_PI_2 + (i as f32 + 0.5) * DLAT;
    ((TAU * GLOBE_R * latc.cos()) / BRICK_SIZE).round().max(3.0) as usize
}

fn band_color(i: usize, shade: usize) -> Color {
    let t = i as f32 / (BANDS - 1) as f32;
    // Cyan (south) -> blue -> violet -> magenta -> coral -> amber (north).
    let hue = (195.0 + t * 215.0) % 360.0;
    let light = if shade == 0 { 0.56 } else { 0.47 };
    Color::hsl(hue, 0.78, light)
}

/// One curved, chamfered brick for band `i`, centred at longitude 0.
/// Vertices are relative to the brick centre so knocked-out bricks tumble about themselves.
fn brick_mesh(i: usize, dlon: f32) -> (Mesh, Vec3) {
    let lat0 = -FRAC_PI_2 + i as f32 * DLAT;
    let lat1 = lat0 + DLAT;
    let center = sph((lat0 + lat1) * 0.5, 0.0, GLOBE_R);

    // Corner ring inset by tangent distance `d`, at radius `rad`:
    // [south-west, south-east, north-east, north-west] (CCW seen from outside).
    let ring = |d: f32, rad: f32| -> [Vec3; 4] {
        let e = d / GLOBE_R;
        let la = lat0 + e;
        let lb = lat1 - e;
        let lon_in = |lat: f32| (d / (GLOBE_R * lat.cos().max(0.02))).min(dlon * 0.24);
        let ha = dlon * 0.5 - lon_in(la);
        let hb = dlon * 0.5 - lon_in(lb);
        [
            sph(la, -ha, rad) - center,
            sph(la, ha, rad) - center,
            sph(lb, hb, rad) - center,
            sph(lb, -hb, rad) - center,
        ]
    };

    let top = ring(GAP + BEVEL, R_OUT);
    let edge = ring(GAP, R_OUT - BEVEL);
    let bottom = ring(0.0, R_IN);

    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(40);
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity(40);
    let mut indices: Vec<u32> = Vec::with_capacity(60);

    let mut quad = |q: [Vec3; 4]| {
        // Diagonal cross product: robust even when a quad degenerates near the poles.
        let n = (q[2] - q[0]).cross(q[3] - q[1]).normalize_or_zero();
        let b = positions.len() as u32;
        for v in q {
            positions.push(v.to_array());
            normals.push(n.to_array());
        }
        indices.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
    };

    // Outer face.
    quad(top);
    // Chamfer ring and side walls: (B[k], B[k+1], A[k+1], A[k]) with A the outer ring.
    for (a, b) in [(&top, &edge), (&edge, &bottom)] {
        for k in 0..4 {
            let k1 = (k + 1) % 4;
            quad([b[k], b[k1], a[k1], a[k]]);
        }
    }
    // Inner face (seen from inside the globe).
    quad([bottom[0], bottom[3], bottom[2], bottom[1]]);

    let uvs = vec![[0.0f32, 0.0]; positions.len()];
    let mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U32(indices));

    (mesh, center)
}

fn build_globe_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut band_meshes = Vec::with_capacity(BANDS);
    let mut band_mats = Vec::with_capacity(BANDS);

    for i in 0..BANDS {
        let dlon = TAU / band_count(i) as f32;
        let (mesh, center) = brick_mesh(i, dlon);
        band_meshes.push((meshes.add(mesh), center));

        let mats = [0, 1].map(|shade| {
            let c = band_color(i, shade);
            materials.add(StandardMaterial {
                base_color: c,
                emissive: c.to_linear() * 0.035,
                perceptual_roughness: 0.34,
                metallic: 0.05,
                reflectance: 0.55,
                clearcoat: 0.45,
                clearcoat_perceptual_roughness: 0.12,
                ..default()
            })
        });
        band_mats.push(mats);
    }

    let highlight = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 1.0, 1.0),
        emissive: LinearRgba::rgb(1.6, 1.7, 2.0),
        perceptual_roughness: 0.3,
        ..default()
    });

    commands.insert_resource(GlobeAssets {
        band_meshes,
        band_mats,
        highlight,
    });
}

/// Spawns a fresh globe (root + all bricks as children) and resets the lookup table.
pub fn spawn_globe(commands: &mut Commands, assets: &GlobeAssets, globe: &mut Globe, rotation: Quat) {
    let root = commands
        .spawn((
            Transform::from_translation(GLOBE_CENTER).with_rotation(rotation),
            Visibility::default(),
            GlobeRoot,
        ))
        .id();

    globe.bands.clear();
    globe.total = 0;

    for i in 0..BANDS {
        let count = band_count(i);
        let dlon = TAU / count as f32;
        let offset = if i % 2 == 1 { 0.5 } else { 0.0 };
        let (mesh, center) = &assets.band_meshes[i];
        let mut cells = Vec::with_capacity(count);

        for j in 0..count {
            let lon = (j as f32 + 0.5 + offset) * dlon;
            let rot = Quat::from_rotation_y(lon);
            let mat = assets.band_mats[i][(i + j) % 2].clone();
            let e = commands
                .spawn((
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(mat.clone()),
                    Transform {
                        translation: rot * *center,
                        rotation: rot,
                        ..default()
                    },
                    Brick { mat },
                    ChildOf(root),
                ))
                .id();
            cells.push(Some(e));
        }

        globe.total += count;
        globe.bands.push(Band {
            count,
            dlon,
            offset,
            cells,
        });
    }

    globe.remaining = globe.total;
    globe.root = Some(root);
}

impl Globe {
    /// Which cell a globe-local point falls into (ignores radius).
    pub fn cell_at(&self, local: Vec3) -> Option<(usize, usize)> {
        let rad = local.length();
        if rad < 1e-4 {
            return None;
        }
        let lat = (local.y / rad).clamp(-1.0, 1.0).asin();
        let i = (((lat + FRAC_PI_2) / DLAT) as isize).clamp(0, BANDS as isize - 1) as usize;
        let band = self.bands.get(i)?;
        let lon = (-local.z).atan2(local.x).rem_euclid(TAU);
        let j = ((lon / band.dlon - band.offset).floor() as isize).rem_euclid(band.count as isize)
            as usize;
        Some((i, j))
    }

    /// Does a sphere of radius `tol` at `local` touch a brick or the core?
    pub fn probe(&self, local: Vec3, tol: f32) -> Option<Hit> {
        let rad = local.length();
        if rad <= CORE_R + tol {
            return Some(Hit::Core);
        }
        if (rad - GLOBE_R).abs() <= BRICK_THICK * 0.5 + tol {
            let (i, j) = self.cell_at(local)?;
            if let Some(e) = self.bands[i].cells[j] {
                return Some(Hit::Brick(i, j, e));
            }
        }
        None
    }

    /// Remove a brick from the lookup table.
    pub fn take(&mut self, i: usize, j: usize) -> Option<Entity> {
        let e = self.bands.get_mut(i)?.cells.get_mut(j)?.take();
        if e.is_some() {
            self.remaining -= 1;
        }
        e
    }

    /// First brick/core along a world-space ray (`dir` normalised). Returns distance and hit.
    pub fn raycast(&self, root: &Transform, origin: Vec3, dir: Vec3) -> Option<(f32, Hit)> {
        let inv = root.rotation.inverse();
        let o = inv * (origin - root.translation);
        let d = inv * dir;
        let rr = R_OUT + 0.02;
        let b = o.dot(d);
        let c = o.length_squared() - rr * rr;
        let disc = b * b - c;
        if disc < 0.0 {
            return None;
        }
        let s = disc.sqrt();
        let t1 = -b + s;
        if t1 < 0.0 {
            return None;
        }
        let mut t = (-b - s).max(0.0);
        while t <= t1 {
            if let Some(hit) = self.probe(o + d * t, 0.0) {
                return Some((t, hit));
            }
            t += 0.025;
        }
        None
    }
}

