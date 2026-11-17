//! HUD: remaining bricks + progress bar, time, accuracy, spin speed, win overlay.

use bevy::{
    diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    prelude::*,
    window::PrimaryWindow,
};

use crate::game::{MAX_SPIN, SpinSettings, Stats};
use crate::globe::Globe;

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_hud)
            .add_systems(Update, (ui_scale, update_hud, toggle_fps));
    }
}

#[derive(Component)]
struct BricksText;
#[derive(Component)]
struct TimeText;
#[derive(Component)]
struct AccText;
#[derive(Component)]
struct ProgressFill;
#[derive(Component)]
struct SpinText;
#[derive(Component)]
struct SpinFill;
#[derive(Component)]
struct FpsText;
#[derive(Component)]
struct Overlay;
#[derive(Component)]
struct OverlaySub;

const LABEL: Color = Color::srgb(0.55, 0.62, 0.78);
const ACCENT: Color = Color::srgb(0.35, 0.9, 1.0);
const GOLD: Color = Color::srgb(1.0, 0.8, 0.3);

fn font(size: f32) -> TextFont {
    TextFont {
        font_size: FontSize::Px(size),
        ..default()
    }
}

fn stat_block(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    value: &str,
    align: AlignItems,
    marker: impl Component,
) {
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            align_items: align,
            ..default()
        })
        .with_children(|b| {
            b.spawn((Text::new(label), font(11.0), TextColor(LABEL)));
            b.spawn((
                Text::new(value),
                font(22.0),
                TextColor(Color::WHITE),
                TextShadow::default(),
                marker,
            ));
        });
}

fn bar(parent: &mut ChildSpawnerCommands, height: f32, fill: Color, marker: impl Component) {
    parent
        .spawn((
            Node {
                width: percent(100),
                height: px(height),
                ..default()
            },
            BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.08)),
        ))
        .with_children(|b| {
            b.spawn((
                Node {
                    width: percent(100),
                    height: percent(100),
                    ..default()
                },
                BackgroundColor(fill),
                marker,
            ));
        });
}

fn setup_hud(mut commands: Commands) {
    // Top panel.
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            top: px(14),
            left: px(16),
            right: px(16),
            flex_direction: FlexDirection::Column,
            row_gap: px(8),
            ..default()
        })
        .with_children(|top| {
            top.spawn(Node {
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            })
            .with_children(|row| {
                stat_block(row, "BRICKS", "0 / 0", AlignItems::FlexStart, BricksText);
                stat_block(row, "TIME", "00:00.0", AlignItems::Center, TimeText);
                stat_block(row, "ACCURACY", "--", AlignItems::FlexEnd, AccText);
            });
            bar(top, 5.0, ACCENT, ProgressFill);
        });

    // Bottom panel: spin speed + controls.
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            bottom: px(12),
            left: px(16),
            right: px(16),
            flex_direction: FlexDirection::Column,
            row_gap: px(6),
            ..default()
        })
        .with_children(|bottom| {
            bottom.spawn((
                Text::new("SPIN x1.00"),
                font(14.0),
                TextColor(GOLD),
                TextShadow::default(),
                SpinText,
            ));
            bar(bottom, 3.0, GOLD, SpinFill);
            bottom.spawn((
                Text::new(
                    "Left Click / Space (hold): fire   Scroll / + - : spin speed\n\
                     0: pause   1 2 3: presets   T: reverse   M: axis mode   R: reset   F: FPS",
                ),
                font(10.5),
                TextColor(Color::srgb(0.45, 0.5, 0.62)),
            ));
        });

    commands.spawn((
        Text::new(""),
        font(11.0),
        TextColor(Color::srgb(0.4, 0.95, 0.5)),
        Node {
            position_type: PositionType::Absolute,
            top: px(78),
            right: px(16),
            ..default()
        },
        FpsText,
    ));

    // Win overlay.
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: px(10),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.02, 0.55)),
            Visibility::Hidden,
            Overlay,
        ))
        .with_children(|o| {
            o.spawn((
                Text::new("GLOBE CLEARED!"),
                font(38.0),
                TextColor(ACCENT),
                TextShadow::default(),
            ));
            o.spawn((
                Text::new(""),
                font(16.0),
                TextColor(Color::WHITE),
                TextLayout::justify(Justify::Center),
                OverlaySub,
            ));
        });
}

/// Keep the HUD proportional to the window height.
fn ui_scale(window: Single<&Window, With<PrimaryWindow>>, mut scale: ResMut<UiScale>) {
    let s = (window.height() / 880.0).clamp(0.55, 2.5);
    if (scale.0 - s).abs() > 0.01 {
        scale.0 = s;
    }
}

fn fmt_time(t: f32) -> String {
    let m = (t / 60.0) as u32;
    let s = t - m as f32 * 60.0;
    format!("{m:02}:{s:04.1}")
}

fn set_text(text: &mut Text, value: String) {
    if text.0 != value {
        text.0 = value;
    }
}

fn update_hud(
    globe: Res<Globe>,
    stats: Res<Stats>,
    spin: Res<SpinSettings>,
    diagnostics: Res<DiagnosticsStore>,
    mut texts: ParamSet<(
        Single<&mut Text, With<BricksText>>,
        Single<&mut Text, With<TimeText>>,
        Single<&mut Text, With<AccText>>,
        Single<&mut Text, With<SpinText>>,
        Single<&mut Text, With<FpsText>>,
        Single<&mut Text, With<OverlaySub>>,
    )>,
    mut progress: Single<&mut Node, (With<ProgressFill>, Without<SpinFill>)>,
    mut spin_fill: Single<&mut Node, (With<SpinFill>, Without<ProgressFill>)>,
    mut overlay: Single<&mut Visibility, With<Overlay>>,
) {
    set_text(
        &mut texts.p0(),
        format!("{} / {}", globe.remaining, globe.total),
    );
    set_text(&mut texts.p1(), fmt_time(stats.elapsed));
    let acc = if stats.shots == 0 {
        "--".to_string()
    } else {
        format!("{:.0}%", stats.accuracy() * 100.0)
    };
    set_text(&mut texts.p2(), acc.clone());

    let state = if spin.paused || spin.multiplier == 0.0 {
        "PAUSED"
    } else if spin.reverse {
        "REVERSE"
    } else {
        "FORWARD"
    };
    set_text(
        &mut texts.p3(),
        format!(
            "SPIN x{:.2}   {}   AXIS: {}",
            spin.multiplier,
            state,
            spin.mode.label()
        ),
    );

    if let Some(fps) = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|d| d.smoothed())
    {
        let mut t = texts.p4();
        if !t.0.is_empty() {
            set_text(&mut t, format!("{fps:.0} FPS"));
        }
    }

    let cleared = if globe.total == 0 {
        0.0
    } else {
        1.0 - globe.remaining as f32 / globe.total as f32
    };
    progress.width = percent(cleared * 100.0);
    let spin_frac = if spin.paused {
        0.0
    } else {
        spin.multiplier / MAX_SPIN
    };
    spin_fill.width = percent(spin_frac * 100.0);

    let vis = if stats.won {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    if **overlay != vis {
        **overlay = vis;
    }
    if stats.won {
        set_text(
            &mut texts.p5(),
            format!(
                "Time {}   Accuracy {}\n{} shots for {} bricks\n\nPress R to play again",
                fmt_time(stats.elapsed),
                acc,
                stats.shots,
                globe.total
            ),
        );
    }
}

fn toggle_fps(keys: Res<ButtonInput<KeyCode>>, mut fps: Single<&mut Text, With<FpsText>>) {
    if keys.just_pressed(KeyCode::KeyF) {
        fps.0 = if fps.0.is_empty() {
            "-- FPS".into()
        } else {
            String::new()
        };
    }
}
