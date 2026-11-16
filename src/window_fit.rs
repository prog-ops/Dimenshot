use bevy::{
    diagnostic::FrameCount,
    prelude::*,
    window::{Monitor, PrimaryMonitor, PrimaryWindow, WindowPosition},
};

/// Portrait window: 3/4 of the monitor height, width = height * ASPECT.
const SCREEN_FRACTION: f32 = 0.75;
const ASPECT: f32 = 0.62;

pub struct WindowFitPlugin;

impl Plugin for WindowFitPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (fit_window_to_screen, make_window_visible));
    }
}

fn fit_window_to_screen(
    mut done: Local<bool>,
    primary: Query<&Monitor, With<PrimaryMonitor>>,
    any: Query<&Monitor>,
    mut window: Single<&mut Window, With<PrimaryWindow>>,
) {
    if *done {
        return;
    }
    let Some(monitor) = primary.iter().next().or_else(|| any.iter().next()) else {
        return;
    };

    let h = (monitor.physical_height as f32 * SCREEN_FRACTION).round().max(480.0);
    let w = (h * ASPECT).round();
    window.resolution.set_physical_resolution(w as u32, h as u32);
    window.position = WindowPosition::Centered(MonitorSelection::Primary);
    *done = true;
}

fn make_window_visible(mut window: Single<&mut Window, With<PrimaryWindow>>, frames: Res<FrameCount>) {
    if frames.0 >= 4 && !window.visible {
        window.visible = true;
    }
}
