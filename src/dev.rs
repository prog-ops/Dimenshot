//! Developer helpers for automated checks (no effect in normal play).
//!
//! * `DIMENSHOT_DEMO=1`      — auto-aim a sweeping virtual cursor and fire continuously.
//! * `DIMENSHOT_SHOT=<path>` — save a screenshot after `DIMENSHOT_FRAMES` frames (default 240), then exit.

use bevy::{
    diagnostic::FrameCount,
    prelude::*,
    render::view::screenshot::{save_to_disk, Screenshot},
};

pub struct DevPlugin;

impl Plugin for DevPlugin {
    fn build(&self, app: &mut App) {
        let demo = std::env::var("DIMENSHOT_DEMO").is_ok_and(|v| v == "1");
        let shot = std::env::var("DIMENSHOT_SHOT").ok();
        let frames = std::env::var("DIMENSHOT_FRAMES")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(240);
        app.insert_resource(DemoMode { enabled: demo });
        if let Some(path) = shot {
            app.insert_resource(AutoShot { path, frames })
                .add_systems(Update, auto_screenshot);
        }
    }
}

#[derive(Resource)]
pub struct DemoMode {
    pub enabled: bool,
}

#[derive(Resource)]
struct AutoShot {
    path: String,
    frames: u32,
}

fn auto_screenshot(
    mut commands: Commands,
    frames: Res<FrameCount>,
    shot: Res<AutoShot>,
    mut exit: MessageWriter<AppExit>,
) {
    if frames.0 == shot.frames {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(shot.path.clone()));
    }
    if frames.0 == shot.frames + 40 {
        exit.write(AppExit::Success);
    }
}

