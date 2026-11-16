//! Dimenshot — shoot the bricks off a giant spinning hollow globe.
//!
//! The globe's shell is built from ~850 small curved bricks. Every hit knocks one brick
//! out; once the front opens up, shots fly through the holes and hit the inside of the
//! far wall. The spin speed is adjustable live (mouse wheel, `+`/`-`, `0`, `M` for axis mode).

mod dev;
mod fx;
mod game;
mod globe;
mod hud;
mod rng;
mod window_fit;

use bevy::{
    diagnostic::FrameTimeDiagnosticsPlugin,
    light::{DirectionalLightShadowMap, GlobalAmbientLight},
    prelude::*,
    window::{PresentMode, WindowResolution},
};

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.006, 0.007, 0.016)))
        .insert_resource(GlobalAmbientLight {
            color: Color::srgb(0.55, 0.62, 1.0),
            brightness: 180.0,
            ..default()
        })
        // Crisp brick shadows on the inner wall.
        .insert_resource(DirectionalLightShadowMap { size: 4096 })
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Dimenshot".into(),
                    // Real size is derived from the monitor (3/4 of its height) by `WindowFitPlugin`.
                    resolution: WindowResolution::new(540, 900),
                    present_mode: PresentMode::AutoVsync,
                    // Hidden until sized and the first frames are rendered (no white flash).
                    visible: false,
                    ..default()
                }),
                ..default()
            }),
            FrameTimeDiagnosticsPlugin::default(),
        ))
        .add_plugins((
            window_fit::WindowFitPlugin,
            globe::GlobePlugin,
            fx::FxPlugin,
            game::GamePlugin,
            hud::HudPlugin,
            dev::DevPlugin,
        ))
        .run();
}
