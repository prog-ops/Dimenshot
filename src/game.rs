//! Gameplay: globe spin, circular aim zone, firing track, bullet travel and brick hits.

use std::f32::consts::FRAC_PI_2;

use bevy::{
    anti_alias::contrast_adaptive_sharpening::ContrastAdaptiveSharpening,
    camera::Hdr,
    core_pipeline::tonemapping::Tonemapping,
    input::mouse::MouseWheel,
    light::CascadeShadowConfigBuilder,
    post_process::bloom::Bloom,
    prelude::*,
    window::{CursorIcon, PrimaryWindow, SystemCursorIcon},
};

use crate::dev::DemoMode;
use crate::fx::{
    spawn_chips, spawn_spark, Debris, FxAssets, FxRng, MainCamera, ScreenShake, Transient, FOV,
};
use crate::globe::{spawn_globe, Brick, Globe, GlobeAssets, GlobeRoot, Hit, CORE_R, GLOBE_CENTER};

/// Center of the circular aiming zone at the bottom of the playfield.
pub const SHOOTER_CENTER: Vec3 = Vec3::new(0.0, -4.15, 4.2);
/// Radius of the circular area around `SHOOTER_CENTER` where the player steers the aim.
pub const AIM_ZONE_R: f32 = 1.45;
/// Maximum angular deflection from dead-center at the rim of the aiming circle.
pub const MAX_AIM_ANGLE: f32 = 38.0 * std::f32::consts::PI / 180.0;

const TRACK_DOT_COUNT: usize = 14;
const TRACK_STEP: f32 = 0.85;
const TRACK_FLOW_SPEED: f32 = 3.4;
const TRACK_MISS_LEN: f32 = 11.5;

const BULLET_SPEED: f32 = 62.0;
const BULLET_R: f32 = 0.06;
const FIRE_RATE: f32 = 11.0;
/// Base angular speed of the globe at multiplier 1.0 (rad/s).
const BASE_SPIN: f32 = 0.45;
pub const MAX_SPIN: f32 = 6.0;

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SpinSettings>()
            .init_resource::<Aim>()
            .init_resource::<Stats>()
            .add_systems(Startup, setup_scene)
            .add_systems(
                Update,
                (
                    spin_input,
                    restart_input,
                    spin_globe,
                    aim,
                    highlight_target,
                    fire,
                    move_bullets,
                    update_stats_clock,
                    aim_track_visuals,
                    reticle_visuals,
                    core_pulse,
                )
                    .chain(),
            );
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SpinMode {
    Tilt,
    Tumble,
    Roll,
}

impl SpinMode {
    pub fn label(self) -> &'static str {
        match self {
            SpinMode::Tilt => "MIRING",
            SpinMode::Tumble => "ACAK",
            SpinMode::Roll => "GULING",
        }
    }
    fn next(self) -> Self {
        match self {
            SpinMode::Tilt => SpinMode::Tumble,
            SpinMode::Tumble => SpinMode::Roll,
            SpinMode::Roll => SpinMode::Tilt,
        }
    }
}

#[derive(Resource)]
pub struct SpinSettings {
    pub multiplier: f32,
    pub paused: bool,
    pub reverse: bool,
    pub mode: SpinMode,
    /// Own clock so the tumble axis only evolves while spinning.
    clock: f32,
}

impl Default for SpinSettings {
    fn default() -> Self {
        Self {
            multiplier: 1.0,
            paused: false,
            reverse: false,
            mode: SpinMode::Tilt,
            clock: 0.0,
        }
    }
}

#[derive(Resource)]
pub struct Aim {
    /// Normalised 2D stick position inside the unit aiming circle (`|u| <= 1`).
    pub u: Vec2,
    pub dir: Vec3,
    pub target: Vec3,
    pub track_len: f32,
    pub on_globe: bool,
    pub brick: Option<Entity>,
}

impl Default for Aim {
    fn default() -> Self {
        Self {
            u: Vec2::ZERO,
            dir: (GLOBE_CENTER - SHOOTER_CENTER).normalize(),
            target: GLOBE_CENTER,
            track_len: TRACK_MISS_LEN,
            on_globe: false,
            brick: None,
        }
    }
}

#[derive(Resource, Default)]
pub struct Stats {
    pub shots: u32,
    pub hits: u32,
    pub elapsed: f32,
    pub running: bool,
    pub won: bool,
    cooldown: f32,
}

impl Stats {
    pub fn accuracy(&self) -> f32 {
        if self.shots == 0 { 0.0 } else { self.hits as f32 / self.shots as f32 }
    }
}

#[derive(Component)]
struct Bullet {
    vel: Vec3,
    life: f32,
}

#[derive(Component)]
struct AimHandle;

#[derive(Component)]
struct AimTrackBeam;

#[derive(Component)]
struct AimTrackDot(usize);

#[derive(Component)]
struct Reticle;

#[derive(Component)]
struct Core;

#[derive(Resource)]
struct GameAssets {
    bullet_mesh: Handle<Mesh>,
    bullet_mat: Handle<StandardMaterial>,
}

fn initial_rotation() -> Quat {
    Quat::from_rotation_x(0.35) * Quat::from_rotation_z(-0.2)
}

fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    globe_assets: Res<GlobeAssets>,
    mut globe: ResMut<Globe>,
    window: Single<Entity, With<PrimaryWindow>>,
) {
    commands
        .entity(*window)
        .insert(CursorIcon::from(SystemCursorIcon::Crosshair));

    // Camera: HDR + bloom for the glowing bits, MSAA for clean edges, CAS for extra crispness.
    commands.spawn((
        Camera3d::default(),
        Hdr,
        Msaa::Sample4,
        Tonemapping::TonyMcMapface,
        Bloom {
            intensity: 0.16,
            ..Bloom::NATURAL
        },
        ContrastAdaptiveSharpening {
            enabled: true,
            sharpening_strength: 0.45,
            ..default()
        },
        Projection::from(PerspectiveProjection {
            fov: FOV,
            near: 0.5,
            far: 400.0,
            ..default()
        }),
        Transform::from_xyz(0.0, 0.6, 36.0),
        MainCamera,
    ));

    // Key light with shadows: bricks shade the inside of the shell.
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(1.0, 0.97, 0.92),
            illuminance: 9_000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(-6.0, 9.0, 10.0).looking_at(GLOBE_CENTER, Vec3::Y),
        CascadeShadowConfigBuilder {
            num_cascades: 1,
            minimum_distance: 1.0,
            maximum_distance: 60.0,
            first_cascade_far_bound: 60.0,
            ..default()
        }
        .build(),
    ));

    // Cool fill light from bottom-right.
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(0.45, 0.65, 1.0),
            illuminance: 2_200.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(9.0, -6.0, 8.0).looking_at(GLOBE_CENTER, Vec3::Y),
    ));

    // Glowing core + a short-range light that only reaches the inner wall.
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(CORE_R).mesh().ico(5).unwrap())),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::BLACK,
            emissive: LinearRgba::rgb(7.0, 2.8, 0.7),
            ..default()
        })),
        Transform::from_translation(GLOBE_CENTER),
        Core,
    ));
    commands.spawn((
        PointLight {
            color: Color::srgb(1.0, 0.62, 0.3),
            intensity: 450_000.0,
            range: 5.2,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_translation(GLOBE_CENTER),
    ));

    let accent = materials.add(StandardMaterial {
        base_color: Color::BLACK,
        emissive: LinearRgba::rgb(0.4, 2.2, 3.2),
        reflectance: 0.0,
        ..default()
    });
    let zone_fill_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.2, 0.75, 1.0, 0.015),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    let zone_ring_mat = materials.add(StandardMaterial {
        base_color: Color::BLACK,
        emissive: LinearRgba::rgb(0.45, 2.0, 2.9),
        reflectance: 0.0,
        ..default()
    });
    let beam_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.35, 0.9, 1.0, 0.34),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    let track_dot_mat = materials.add(StandardMaterial {
        base_color: Color::BLACK,
        emissive: LinearRgba::rgb(1.5, 4.5, 6.5),
        reflectance: 0.0,
        ..default()
    });
    let handle_mat = materials.add(StandardMaterial {
        base_color: Color::BLACK,
        emissive: LinearRgba::rgb(2.4, 6.0, 8.0),
        reflectance: 0.0,
        ..default()
    });

    // Circular aiming zone around `SHOOTER_CENTER` (in XY plane facing camera).
    commands.spawn((
        Mesh3d(meshes.add(Circle::new(AIM_ZONE_R))),
        MeshMaterial3d(zone_fill_mat),
        Transform::from_translation(SHOOTER_CENTER - Vec3::Z * 0.01),
    ));
    commands.spawn((
        Mesh3d(meshes.add(Torus::new(AIM_ZONE_R - 0.022, AIM_ZONE_R + 0.022))),
        MeshMaterial3d(zone_ring_mat),
        Transform::from_translation(SHOOTER_CENTER).with_rotation(Quat::from_rotation_x(FRAC_PI_2)),
    ));
    // Subtle center origin pip.
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(0.05).mesh().ico(2).unwrap())),
        MeshMaterial3d(accent.clone()),
        Transform::from_translation(SHOOTER_CENTER),
    ));
    // Aim handle pip moving inside the circular aiming zone.
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(0.095).mesh().ico(3).unwrap())),
        MeshMaterial3d(handle_mat),
        Transform::from_translation(SHOOTER_CENTER + Vec3::Z * 0.02),
        AimHandle,
    ));

    // Firing direction track: thin glowing beam + animated flowing markers.
    commands.spawn((
        Mesh3d(meshes.add(Cylinder::new(0.022, 1.0))),
        MeshMaterial3d(beam_mat),
        Transform::from_translation(SHOOTER_CENTER),
        AimTrackBeam,
    ));
    let dot_mesh = meshes.add(Capsule3d::new(0.038, 0.14));
    for i in 0..TRACK_DOT_COUNT {
        commands.spawn((
            Mesh3d(dot_mesh.clone()),
            MeshMaterial3d(track_dot_mat.clone()),
            Transform::from_translation(SHOOTER_CENTER).with_scale(Vec3::splat(0.001)),
            AimTrackDot(i),
        ));
    }

    // Reticle ring shown on the brick under the track.
    commands.spawn((
        Mesh3d(meshes.add(Torus::new(0.17, 0.21))),
        MeshMaterial3d(accent),
        Transform::from_translation(GLOBE_CENTER).with_scale(Vec3::splat(0.001)),
        Reticle,
    ));

    commands.insert_resource(GameAssets {
        bullet_mesh: meshes.add(Capsule3d::new(BULLET_R, 0.55)),
        bullet_mat: materials.add(StandardMaterial {
            base_color: Color::BLACK,
            emissive: LinearRgba::rgb(4.0, 9.0, 12.0),
            reflectance: 0.0,
            ..default()
        }),
    });

    spawn_globe(&mut commands, &globe_assets, &mut globe, initial_rotation());
}

fn spin_input(
    mut spin: ResMut<SpinSettings>,
    mut wheel: MessageReader<MouseWheel>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    let mut delta = 0.0;
    for w in wheel.read() {
        delta += w.y.signum() * 0.25;
    }
    if keys.any_just_pressed([KeyCode::Equal, KeyCode::NumpadAdd, KeyCode::BracketRight]) {
        delta += 0.25;
    }
    if keys.any_just_pressed([KeyCode::Minus, KeyCode::NumpadSubtract, KeyCode::BracketLeft]) {
        delta -= 0.25;
    }
    if delta != 0.0 {
        spin.multiplier = (spin.multiplier + delta).clamp(0.0, MAX_SPIN);
        spin.paused = false;
    }
    if keys.just_pressed(KeyCode::Digit0) {
        spin.paused = !spin.paused;
    }
    for (key, value) in [
        (KeyCode::Digit1, 1.0),
        (KeyCode::Digit2, 2.0),
        (KeyCode::Digit3, 3.5),
    ] {
        if keys.just_pressed(key) {
            spin.multiplier = value;
            spin.paused = false;
        }
    }
    if keys.just_pressed(KeyCode::KeyT) {
        spin.reverse = !spin.reverse;
    }
    if keys.just_pressed(KeyCode::KeyM) {
        spin.mode = spin.mode.next();
    }
}

fn spin_globe(
    time: Res<Time>,
    mut spin: ResMut<SpinSettings>,
    mut root: Single<&mut Transform, With<GlobeRoot>>,
) {
    if spin.paused || spin.multiplier <= 0.0 {
        return;
    }
    let dt = time.delta_secs();
    spin.clock += dt * spin.multiplier;
    let t = spin.clock;
    let axis = match spin.mode {
        SpinMode::Tilt => Vec3::new(0.3, 1.0, 0.25),
        SpinMode::Tumble => Vec3::new((t * 0.37).sin() * 1.1, 0.8, (t * 0.23).cos() * 1.1),
        SpinMode::Roll => Vec3::new(1.0, 0.12, 0.0),
    }
    .normalize();
    let w = BASE_SPIN * spin.multiplier * if spin.reverse { -1.0 } else { 1.0 };
    root.rotation = (Quat::from_axis_angle(axis, w * dt) * root.rotation).normalize();
}

fn aim(
    time: Res<Time>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform), With<MainCamera>>,
    root: Single<&Transform, With<GlobeRoot>>,
    globe: Res<Globe>,
    demo: Res<DemoMode>,
    mut aim: ResMut<Aim>,
) {
    if demo.enabled {
        // Sweep the stick smoothly inside the circular aiming zone for automated captures.
        let t = time.elapsed_secs();
        let rad = 0.88 * (0.3 + 0.7 * (t * 0.37).sin().abs());
        aim.u = Vec2::new((t * 1.7).cos(), (t * 1.1).sin()) * rad;
    } else if let Some(cursor) = window.cursor_position() {
        let (camera, cam_tf) = *camera;
        if let Ok(ray) = camera.viewport_to_world(cam_tf, cursor) {
            let d = ray.direction.as_vec3();
            if d.z.abs() > 1e-4 {
                let t = (SHOOTER_CENTER.z - ray.origin.z) / d.z;
                if t > 0.0 {
                    let hit_plane = ray.origin + d * t;
                    let offset = hit_plane.truncate() - SHOOTER_CENTER.truncate();
                    aim.u = (offset / AIM_ZONE_R).clamp_length_max(1.0);
                }
            }
        }
    }

    // Map circular stick `u` (`|u| <= 1`) to a 3D firing cone centered on `forward`.
    let forward = (GLOBE_CENTER - SHOOTER_CENTER).normalize();
    let right = Vec3::X;
    let up = right.cross(forward).normalize();
    let r = aim.u.length();
    aim.dir = if r > 1e-5 {
        let angle = r * MAX_AIM_ANGLE;
        let tilt = (right * aim.u.x + up * aim.u.y) / r;
        (forward * angle.cos() + tilt * angle.sin()).normalize()
    } else {
        forward
    };

    if let Some((t, hit)) = globe.raycast(&root, SHOOTER_CENTER, aim.dir) {
        aim.target = SHOOTER_CENTER + aim.dir * t;
        aim.track_len = t;
        aim.on_globe = true;
        aim.brick = match hit {
            Hit::Brick(_, _, e) => Some(e),
            Hit::Core => None,
        };
    } else {
        aim.target = SHOOTER_CENTER + aim.dir * TRACK_MISS_LEN;
        aim.track_len = TRACK_MISS_LEN;
        aim.on_globe = false;
        aim.brick = None;
    }
}

/// Lights up the brick under the track (only one brick ever uses the highlight material).
fn highlight_target(
    aim: Res<Aim>,
    assets: Res<GlobeAssets>,
    mut current: Local<Option<Entity>>,
    mut bricks: Query<(&Brick, &mut MeshMaterial3d<StandardMaterial>)>,
) {
    if *current == aim.brick {
        return;
    }
    if let Some(prev) = current.take()
        && let Ok((brick, mut mat)) = bricks.get_mut(prev)
    {
        mat.0 = brick.mat.clone();
    }
    if let Some(next) = aim.brick
        && let Ok((_, mut mat)) = bricks.get_mut(next)
    {
        mat.0 = assets.highlight.clone();
        *current = Some(next);
    }
}

fn fire(
    mut commands: Commands,
    time: Res<Time>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    demo: Res<DemoMode>,
    aim: Res<Aim>,
    assets: Res<GameAssets>,
    fx: Res<FxAssets>,
    mut stats: ResMut<Stats>,
) {
    stats.cooldown = (stats.cooldown - time.delta_secs()).max(0.0);
    let wants = demo.enabled || mouse.pressed(MouseButton::Left) || keys.pressed(KeyCode::Space);
    if !wants || stats.cooldown > 0.0 || stats.won {
        return;
    }
    stats.cooldown = 1.0 / FIRE_RATE;
    stats.shots += 1;
    stats.running = true;

    let muzzle = SHOOTER_CENTER + aim.dir * 0.15;
    commands.spawn((
        Mesh3d(assets.bullet_mesh.clone()),
        MeshMaterial3d(assets.bullet_mat.clone()),
        Transform::from_translation(muzzle).with_rotation(Quat::from_rotation_arc(Vec3::Y, aim.dir)),
        Bullet {
            vel: aim.dir * BULLET_SPEED,
            life: 2.0,
        },
        Transient,
    ));
    spawn_spark(&mut commands, &fx, fx.muzzle.clone(), muzzle, 0.12, 0.07);
}

fn move_bullets(
    mut commands: Commands,
    time: Res<Time>,
    root: Single<&Transform, (With<GlobeRoot>, Without<Bullet>)>,
    mut globe: ResMut<Globe>,
    mut stats: ResMut<Stats>,
    mut shake: ResMut<ScreenShake>,
    mut rng: ResMut<FxRng>,
    fx: Res<FxAssets>,
    mut bullets: Query<(Entity, &mut Transform, &mut Bullet), Without<GlobeRoot>>,
    bricks: Query<(&Brick, &Transform), (Without<Bullet>, Without<GlobeRoot>)>,
) {
    let dt = time.delta_secs();
    let root_tf: Transform = **root;
    let inv = root_tf.rotation.inverse();
    let rng = &mut rng.0;

    for (entity, mut tf, mut bullet) in &mut bullets {
        bullet.life -= dt;
        if bullet.life <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }

        // Sub-step so a fast bullet can never skip through the thin shell.
        let start = tf.translation;
        let delta = bullet.vel * dt;
        let steps = (delta.length() / 0.07).ceil().max(1.0) as usize;
        let mut hit = None;
        for k in 1..=steps {
            let p = start + delta * (k as f32 / steps as f32);
            if let Some(h) = globe.probe(inv * (p - root_tf.translation), BULLET_R) {
                hit = Some((p, h));
                break;
            }
        }

        let Some((p, hit)) = hit else {
            tf.translation = start + delta;
            continue;
        };
        commands.entity(entity).despawn();
        let dir = bullet.vel.normalize();

        match hit {
            Hit::Core => {
                spawn_spark(&mut commands, &fx, fx.spark_core.clone(), p, 0.3, 0.16);
            }
            Hit::Brick(i, j, brick_entity) => {
                globe.take(i, j);
                stats.hits += 1;
                if let Ok((brick, local)) = bricks.get(brick_entity) {
                    // Detach the brick in place and send it flying.
                    let world = root_tf.mul_transform(*local);
                    let normal = (world.translation - root_tf.translation).normalize_or_zero();
                    let vel = dir * rng.range(4.0, 7.5)
                        + normal * rng.range(1.0, 3.0)
                        + rng.unit_vec3() * 1.2;
                    commands
                        .entity(brick_entity)
                        .remove::<(ChildOf, Brick)>()
                        .insert((
                            world,
                            MeshMaterial3d(brick.mat.clone()),
                            Debris {
                                vel,
                                axis: rng.unit_vec3(),
                                rate: rng.range(3.0, 9.0),
                                age: 0.0,
                                life: 1.5,
                            },
                            Transient,
                        ));
                    spawn_chips(&mut commands, &fx, rng, &brick.mat, p, dir * 4.0 - normal, 5);
                }
                spawn_spark(&mut commands, &fx, fx.spark_hot.clone(), p, 0.22, 0.12);
                shake.add(0.045);
                if globe.remaining == 0 {
                    stats.won = true;
                    stats.running = false;
                    shake.add(0.6);
                }
            }
        }
    }
}

fn update_stats_clock(time: Res<Time>, mut stats: ResMut<Stats>) {
    if stats.running {
        stats.elapsed += time.delta_secs();
    }
}

fn restart_input(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    assets: Res<GlobeAssets>,
    mut globe: ResMut<Globe>,
    mut stats: ResMut<Stats>,
    transients: Query<Entity, With<Transient>>,
) {
    if !keys.just_pressed(KeyCode::KeyR) {
        return;
    }
    if let Some(root) = globe.root.take() {
        commands.entity(root).despawn();
    }
    for e in &transients {
        commands.entity(e).despawn();
    }
    *stats = Stats::default();
    spawn_globe(&mut commands, &assets, &mut globe, initial_rotation());
}

fn aim_track_visuals(
    time: Res<Time>,
    aim: Res<Aim>,
    stats: Res<Stats>,
    mut handle: Single<&mut Transform, (With<AimHandle>, Without<AimTrackBeam>, Without<AimTrackDot>)>,
    mut beam: Single<&mut Transform, (With<AimTrackBeam>, Without<AimHandle>, Without<AimTrackDot>)>,
    mut dots: Query<(&AimTrackDot, &mut Transform), (Without<AimHandle>, Without<AimTrackBeam>)>,
) {
    handle.translation =
        SHOOTER_CENTER + Vec3::new(aim.u.x * AIM_ZONE_R, aim.u.y * AIM_ZONE_R, 0.02);

    if stats.won {
        beam.scale = Vec3::splat(0.001);
        for (_, mut tf) in &mut dots {
            tf.scale = Vec3::splat(0.001);
        }
        return;
    }

    let track_rot = Quat::from_rotation_arc(Vec3::Y, aim.dir);
    beam.translation = SHOOTER_CENTER + aim.dir * (aim.track_len * 0.5);
    beam.rotation = track_rot;
    beam.scale = Vec3::new(1.0, aim.track_len, 1.0);

    let flow = (time.elapsed_secs() * TRACK_FLOW_SPEED).fract();
    for (dot, mut tf) in &mut dots {
        let d = (dot.0 as f32 + flow) * TRACK_STEP;
        if d <= aim.track_len {
            let fade_in = (d / 0.55).clamp(0.0, 1.0);
            let fade_out = ((aim.track_len - d) / 0.55).clamp(0.0, 1.0);
            let s = fade_in.min(fade_out);
            tf.translation = SHOOTER_CENTER + aim.dir * d;
            tf.rotation = track_rot;
            tf.scale = Vec3::splat(s.max(0.001));
        } else {
            tf.scale = Vec3::splat(0.001);
        }
    }
}

fn reticle_visuals(
    time: Res<Time>,
    aim: Res<Aim>,
    stats: Res<Stats>,
    camera: Single<&GlobalTransform, With<MainCamera>>,
    mut reticle: Single<&mut Transform, (With<Reticle>, Without<MainCamera>)>,
) {
    if !aim.on_globe || stats.won {
        reticle.scale = Vec3::splat(0.001);
        return;
    }
    let to_cam = (camera.translation() - aim.target).normalize_or_zero();
    reticle.translation = aim.target + to_cam * 0.05;
    reticle.rotation = Quat::from_rotation_arc(Vec3::Y, to_cam);
    let pulse = 1.0 + (time.elapsed_secs() * 9.0).sin() * 0.08;
    reticle.scale = Vec3::splat(pulse);
}

fn core_pulse(time: Res<Time>, mut core: Single<&mut Transform, With<Core>>) {
    let t = time.elapsed_secs();
    core.scale = Vec3::splat(1.0 + (t * 2.3).sin() * 0.05 + (t * 5.1).sin() * 0.02);
}
