//! Visual effects and camera: knocked-out bricks, chips, sparks, screen shake, starfield.

use bevy::{prelude::*, window::PrimaryWindow};

use crate::rng::Rng;

pub struct FxPlugin;

impl Plugin for FxPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ScreenShake>()
            .insert_resource(FxRng(Rng::seeded()))
            .add_systems(PreStartup, build_fx_assets)
            .add_systems(Startup, spawn_starfield)
            .add_systems(
                Update,
                (update_debris, update_chips, update_sparks, camera_rig),
            );
    }
}

#[derive(Component)]
pub struct MainCamera;

/// Short-lived entities cleared on restart.
#[derive(Component)]
pub struct Transient;

/// A brick that has been shot out of the globe.
#[derive(Component)]
pub struct Debris {
    pub vel: Vec3,
    pub axis: Vec3,
    pub rate: f32,
    pub age: f32,
    pub life: f32,
}

#[derive(Component)]
pub struct Chip {
    pub vel: Vec3,
    pub axis: Vec3,
    pub rate: f32,
    pub age: f32,
    pub life: f32,
    pub size: f32,
}

#[derive(Component)]
pub struct Spark {
    pub age: f32,
    pub life: f32,
    pub size: f32,
}

#[derive(Resource, Default)]
pub struct ScreenShake {
    pub trauma: f32,
}

impl ScreenShake {
    pub fn add(&mut self, amount: f32) {
        self.trauma = (self.trauma + amount).min(1.0);
    }
}

#[derive(Resource)]
pub struct FxRng(pub Rng);

#[derive(Resource)]
pub struct FxAssets {
    pub chip_mesh: Handle<Mesh>,
    pub spark_mesh: Handle<Mesh>,
    pub spark_hot: Handle<StandardMaterial>,
    pub spark_core: Handle<StandardMaterial>,
    pub muzzle: Handle<StandardMaterial>,
}

fn build_fx_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let emissive = |materials: &mut Assets<StandardMaterial>, e: LinearRgba| {
        materials.add(StandardMaterial {
            base_color: Color::BLACK,
            emissive: e,
            unlit: true,
            ..default()
        })
    };
    commands.insert_resource(FxAssets {
        chip_mesh: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        spark_mesh: meshes.add(Sphere::new(1.0).mesh().ico(2).unwrap()),
        spark_hot: emissive(&mut materials, LinearRgba::rgb(9.0, 8.0, 6.0)),
        spark_core: emissive(&mut materials, LinearRgba::rgb(9.0, 3.5, 0.8)),
        muzzle: emissive(&mut materials, LinearRgba::rgb(3.0, 7.0, 10.0)),
    });
}

pub fn spawn_spark(
    commands: &mut Commands,
    fx: &FxAssets,
    material: Handle<StandardMaterial>,
    pos: Vec3,
    size: f32,
    life: f32,
) {
    commands.spawn((
        Mesh3d(fx.spark_mesh.clone()),
        MeshMaterial3d(material),
        Transform::from_translation(pos).with_scale(Vec3::splat(size)),
        Spark {
            age: 0.0,
            life,
            size,
        },
        Transient,
    ));
}

pub fn spawn_chips(
    commands: &mut Commands,
    fx: &FxAssets,
    rng: &mut Rng,
    material: &Handle<StandardMaterial>,
    pos: Vec3,
    push: Vec3,
    n: usize,
) {
    for _ in 0..n {
        let size = rng.range(0.07, 0.15);
        commands.spawn((
            Mesh3d(fx.chip_mesh.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(pos)
                .with_rotation(rng.rotation())
                .with_scale(Vec3::splat(size)),
            Chip {
                vel: push * rng.range(0.4, 1.0) + rng.unit_vec3() * rng.range(2.0, 6.0),
                axis: rng.unit_vec3(),
                rate: rng.range(8.0, 20.0),
                age: 0.0,
                life: rng.range(0.45, 0.8),
                size,
            },
            Transient,
        ));
    }
}

fn update_debris(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut Debris, &mut Transform)>,
) {
    let dt = time.delta_secs();
    for (e, mut d, mut tf) in &mut q {
        d.age += dt;
        if d.age >= d.life {
            commands.entity(e).despawn();
            continue;
        }
        d.vel.y -= 13.0 * dt;
        d.vel *= 1.0 - 0.35 * dt;
        tf.translation += d.vel * dt;
        if let Ok(axis) = Dir3::new(d.axis) {
            tf.rotate_axis(axis, d.rate * dt);
        }
        let p = d.age / d.life;
        let s = if p > 0.55 { 1.0 - (p - 0.55) / 0.45 } else { 1.0 };
        tf.scale = Vec3::splat(s.max(0.001));
    }
}

fn update_chips(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut Chip, &mut Transform)>,
) {
    let dt = time.delta_secs();
    for (e, mut c, mut tf) in &mut q {
        c.age += dt;
        if c.age >= c.life {
            commands.entity(e).despawn();
            continue;
        }
        c.vel.y -= 16.0 * dt;
        tf.translation += c.vel * dt;
        if let Ok(axis) = Dir3::new(c.axis) {
            tf.rotate_axis(axis, c.rate * dt);
        }
        tf.scale = Vec3::splat((c.size * (1.0 - c.age / c.life)).max(0.001));
    }
}

fn update_sparks(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut Spark, &mut Transform)>,
) {
    let dt = time.delta_secs();
    for (e, mut s, mut tf) in &mut q {
        s.age += dt;
        if s.age >= s.life {
            commands.entity(e).despawn();
            continue;
        }
        let p = s.age / s.life;
        // Quick flare then collapse.
        let k = if p < 0.25 { 1.0 + p * 2.0 } else { 1.5 * (1.0 - (p - 0.25) / 0.75) };
        tf.scale = Vec3::splat((s.size * k).max(0.001));
    }
}

/// Field of view and the world-space area that must always stay visible.
pub const FOV: f32 = 30.0 * std::f32::consts::PI / 180.0;
const CAM_Y: f32 = 0.6;
const NEED_HALF_W: f32 = 5.9;
const NEED_HALF_H: f32 = 8.9;

/// Fits the camera to the window aspect every frame and applies screen shake.
fn camera_rig(
    time: Res<Time>,
    mut shake: ResMut<ScreenShake>,
    window: Single<&Window, With<PrimaryWindow>>,
    mut cam: Single<&mut Transform, With<MainCamera>>,
) {
    let dt = time.delta_secs();
    shake.trauma = (shake.trauma - 1.6 * dt).max(0.0);

    let aspect = (window.width() / window.height().max(1.0)).max(0.1);
    let half_h = NEED_HALF_H.max(NEED_HALF_W / aspect);
    let dist = half_h / (FOV * 0.5).tan();

    let t = time.elapsed_secs();
    let s = shake.trauma * shake.trauma * 0.35;
    let offset = Vec3::new(
        (t * 61.0).sin() * s,
        (t * 53.0 + 1.7).sin() * s,
        0.0,
    );
    cam.translation = Vec3::new(0.0, CAM_Y, dist) + offset;
}

fn spawn_starfield(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut rng: ResMut<FxRng>,
) {
    let mesh = meshes.add(Sphere::new(1.0).mesh().ico(1).unwrap());
    let mats: Vec<_> = [
        LinearRgba::rgb(1.4, 1.5, 2.2),
        LinearRgba::rgb(2.4, 2.2, 1.8),
        LinearRgba::rgb(0.7, 0.8, 1.3),
    ]
    .into_iter()
    .map(|e| {
        materials.add(StandardMaterial {
            base_color: Color::BLACK,
            emissive: e,
            unlit: true,
            ..default()
        })
    })
    .collect();

    let rng = &mut rng.0;
    for i in 0..420 {
        let pos = Vec3::new(
            rng.range(-70.0, 70.0),
            rng.range(-95.0, 95.0),
            rng.range(-140.0, -60.0),
        );
        let size = rng.range(0.06, 0.2) * (1.0 + (rng.f32() < 0.04) as u8 as f32 * 1.5);
        commands.spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(mats[i % mats.len()].clone()),
            Transform::from_translation(pos).with_scale(Vec3::splat(size)),
        ));
    }
}

