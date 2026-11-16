//! Compact transport controls. Navigation and annotation browsing live in the sidebar.
use crate::{ui, workspace_view};
use eframe::egui::{self, Button, Color32, Rect, RichText, Sense, Stroke};
use ui::timeline::{ChunkFilter, TimelineUiState};

type Router = daqcore::router::VirtualTimelineRouter;

#[derive(Clone, Copy)]
struct FooterLayout {
    play: Rect,
    back: Option<Rect>,
    forward: Option<Rect>,
    speed: Option<Rect>,
    live: Option<Rect>,
    time: Rect,
    actions: Rect,
}

impl FooterLayout {
    fn new(row: Rect) -> Self {
        let height = row.height();
        let play = Rect::from_center_size(row.center(), egui::vec2(26.0, height));
        let steps = row.width() >= 180.0;
        let extras = row.width() >= 480.0;
        let at =
            |x, width| Rect::from_min_size(egui::pos2(x, row.top()), egui::vec2(width, height));
        let back = steps.then(|| at(play.left() - 26.0, 22.0));
        let forward = steps.then(|| at(play.right() + 4.0, 22.0));
        let speed = extras.then(|| at(play.right() + 32.0, 52.0));
        let live = (row.width() >= 100.0).then(|| {
            at(
                if extras {
                    play.right() + 90.0
                } else if steps {
                    play.right() + 32.0
                } else {
                    play.right() + 4.0
                },
                22.0,
            )
        });
        let time_right = back.unwrap_or(play).left() - 10.0;
        let time = at(row.left(), (time_right - row.left()).max(0.0));
        let actions_left = live.unwrap_or(play).right() + 12.0;
        let actions = at(actions_left, (row.right() - actions_left).max(0.0));
        Self {
            play,
            back,
            forward,
            speed,
            live,
            time,
            actions,
        }
    }
}

pub(super) fn show(
    ui: &mut egui::Ui,
    router: &mut Router,
    state: &mut TimelineUiState,
    bus_load: &mut Vec<workspace_view::BusLoadSample>,
    diagnostic: &mut Option<String>,
) {
    let theme = ui::theme::get_theme(ui.ctx());
    let height = ui.spacing().interact_size.y.max(22.0);
    let (row, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), height), Sense::hover());
    let layout = FooterLayout::new(row);
    let live_available = !router.live_buffer().is_empty();
    let mut go_live = false;
    let mut clear_live = false;
    let mut add_bookmark = false;
    let mut add_interval = false;

    let mut controls = ui.new_child(egui::UiBuilder::new().max_rect(row));
    {
        let ui = &mut controls;
        // Keep every control on this row, even in small windows.
        ui.set_clip_rect(ui.clip_rect().intersect(row));
        if layout.time.width() >= 35.0 {
            let response = ui
                .scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(layout.time)
                        .layout(egui::Layout::left_to_right(egui::Align::Center)),
                    |ui| {
                        ui.add(
                            Button::new(
                                RichText::new(router.timeline().setpoint().label())
                                    .monospace()
                                    .size(11.0)
                                    .color(theme.muted_text_color()),
                            )
                            .frame(false)
                            .truncate(),
                        )
                    },
                )
                .inner
                .on_hover_text("Playhead details");
            egui::Popup::menu(&response).show(|ui| {
                let time = router.timeline().setpoint();
                if let Some(time) = chrono::DateTime::from_timestamp_millis(time.unix_millis()) {
                    ui.label(time.format("%Y-%m-%d").to_string());
                    ui.label(time.format("%H:%M:%S%.3f UTC").to_string());
                }
            });
        }

        let fine = ui.input(|input| input.modifiers.alt);
        for (rect, icon, direction, hover) in [
            (layout.back, "‹", -1, "Step backward (Alt fine)"),
            (layout.forward, "›", 1, "Step forward (Alt fine)"),
        ] {
            if let Some(rect) = rect {
                if ui
                    .put(
                        rect,
                        Button::new(RichText::new(icon).size(17.0)).frame(false),
                    )
                    .on_hover_text(hover)
                    .clicked()
                {
                    router
                        .timeline_mut()
                        .step_by_ms(direction * if fine { 10 } else { 100 });
                }
            }
        }

        // Match the actual selector frame rather than the taller toolbar row.
        let control_height = if let Some(rect) = layout.speed {
            ui.scope_builder(
                egui::UiBuilder::new()
                    .max_rect(rect)
                    .layout(egui::Layout::left_to_right(egui::Align::Center)),
                |ui| speed_control(ui, router),
            )
            .inner
            .min(row.height())
        } else {
            ui.spacing().interact_size.y.min(row.height())
        };

        let running =
            router.timeline().is_playing() || (live_available && router.timeline().is_live());
        let response = ui
            .put(
                Rect::from_center_size(layout.play.center(), egui::vec2(22.0, control_height)),
                Button::new("")
                    .fill(theme.accent_color().gamma_multiply(0.16))
                    .stroke(Stroke::new(
                        1.0_f32,
                        theme.accent_color().gamma_multiply(0.7),
                    ))
                    .corner_radius(3.0),
            )
            .on_hover_text("Play or pause (Space)");
        paint_play_icon(ui, response.rect, running, theme.text_color());
        if response.clicked() {
            router.timeline_mut().toggle_playing();
        }

        if let Some(rect) = layout.live {
            let response = ui
                .put(
                    Rect::from_center_size(rect.center(), egui::vec2(22.0, control_height)),
                    Button::new("")
                        .fill(theme.canvas_color())
                        .stroke(Stroke::new(1.0_f32, theme.border_color()))
                        .corner_radius(3.0)
                        .sense(if live_available {
                            Sense::click()
                        } else {
                            Sense::hover()
                        }),
                )
                .on_hover_text(if live_available {
                    "Follow incoming telemetry"
                } else {
                    "No live telemetry received yet"
                });
            go_live = live_available && response.clicked();
            ui.painter().circle_filled(
                response.rect.center(),
                4.0,
                if live_available && router.timeline().is_live() {
                    Color32::from_rgb(239, 68, 68)
                } else {
                    theme.muted_text_color().gamma_multiply(0.5)
                },
            );
        }

        if layout.actions.width() >= 24.0 {
            let show_events = layout.actions.width() >= 160.0;
            let show_add = layout.actions.width() >= 100.0;
            ui.scope_builder(
                egui::UiBuilder::new()
                    .max_rect(layout.actions)
                    .layout(egui::Layout::right_to_left(egui::Align::Center)),
                |ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    let response = ui
                        .add(
                            Button::new("")
                                .frame(false)
                                .min_size(egui::vec2(22.0, height)),
                        )
                        .on_hover_text("Timeline options");
                    for dx in [-4.0, 0.0, 4.0] {
                        ui.painter().circle_filled(
                            response.rect.center() + egui::vec2(dx, 0.0),
                            1.2,
                            theme.muted_text_color(),
                        );
                    }
                    egui::Popup::menu(&response).show(|ui| {
                        ui.menu_button("View", |ui| view_menu(ui, router, state, diagnostic));
                        ui.menu_button("Info", |ui| info_menu(ui, router));
                        ui.menu_button("Shortcuts", shortcuts_menu);
                        if layout.speed.is_none() {
                            ui.horizontal(|ui| {
                                ui.label("Speed");
                                speed_control(ui, router);
                            });
                        }
                        if layout.live.is_none()
                            && ui
                                .add_enabled(live_available, Button::new("Go live"))
                                .clicked()
                        {
                            go_live = true;
                            ui.close();
                        }
                        if !show_events && ui.button("Events").clicked() {
                            state.requested_sidebar_filter = Some(ChunkFilter::StintsAndEvents);
                            ui.close();
                        }
                        if !show_add {
                            ui.menu_button("Add", |ui| {
                                add_menu(ui, &mut add_bookmark, &mut add_interval);
                            });
                        }
                        if fault_count(router) > 0 && ui.button("Browse faults").clicked() {
                            state.requested_sidebar_filter = Some(ChunkFilter::Faults);
                            ui.close();
                        }
                        if router.timeline().is_comparison_mode()
                            && ui.button("Exit comparison").clicked()
                        {
                            router.timeline_mut().clear_selected_annotations();
                            ui.close();
                        }
                        if live_available {
                            ui.separator();
                            ui.menu_button("Clear live", |ui| {
                                ui.label("Discard buffered telemetry");
                                if ui.button("Clear buffer").clicked() {
                                    clear_live = true;
                                    ui.close();
                                }
                            });
                        }
                    });
                    if show_events
                        && ui
                            .add(Button::new("Events").frame(false))
                            .on_hover_text("Browse sessions and events")
                            .clicked()
                    {
                        state.requested_sidebar_filter = Some(ChunkFilter::StintsAndEvents);
                    }
                    if show_add {
                        ui.menu_button("Add", |ui| {
                            add_menu(ui, &mut add_bookmark, &mut add_interval)
                        });
                    }
                    let faults = fault_count(router);
                    if faults > 0 && ui.available_width() >= 85.0 {
                        if ui
                            .add(
                                Button::new(
                                    RichText::new(format!("Faults ({faults})"))
                                        .color(Color32::from_rgb(245, 158, 11)),
                                )
                                .frame(false),
                            )
                            .on_hover_text("Browse faults")
                            .clicked()
                        {
                            state.requested_sidebar_filter = Some(ChunkFilter::Faults);
                        }
                    }
                    if router.timeline().is_comparison_mode() && ui.available_width() >= 100.0 {
                        let count = router.timeline().selected_annotation_ids().len();
                        if ui
                            .add(Button::new(format!("Comparing ({count})")).frame(false))
                            .on_hover_text("Exit comparison")
                            .clicked()
                        {
                            router.timeline_mut().clear_selected_annotations();
                        }
                    }
                },
            );
        }
    }

    if go_live {
        router.go_live();
    }
    if clear_live {
        router.reset_live(daqcore::time::Time::now());
        bus_load.clear();
        state.reset_ruler();
    }
    if add_bookmark {
        state.open_new_at_playhead(router, true);
    }
    if add_interval {
        state.open_new_at_playhead(router, false);
    }
}

fn paint_play_icon(ui: &egui::Ui, rect: Rect, running: bool, color: Color32) {
    let center = rect.center();
    if running {
        for dx in [-2.5, 2.5] {
            ui.painter().rect_filled(
                Rect::from_center_size(center + egui::vec2(dx, 0.0), egui::vec2(2.0, 9.0)),
                0.0,
                color,
            );
        }
    } else {
        ui.painter().add(egui::Shape::convex_polygon(
            vec![
                center + egui::vec2(-3.0, -4.5),
                center + egui::vec2(4.0, 0.0),
                center + egui::vec2(-3.0, 4.5),
            ],
            color,
            Stroke::NONE,
        ));
    }
}

fn speed_control(ui: &mut egui::Ui, router: &mut Router) -> f32 {
    let current = router.timeline().playback_speed();
    egui::ComboBox::from_id_salt("timeline_playback_speed")
        .width(46.0)
        .selected_text(format!("{current}x"))
        .show_ui(ui, |ui| {
            for speed in daqcore::timeline::PLAYBACK_SPEED_PRESETS {
                if ui
                    .selectable_label((current - speed).abs() < 1e-4, format!("{speed}x"))
                    .clicked()
                {
                    router.timeline_mut().set_playback_speed(speed);
                }
            }
        })
        .response
        .rect
        .height()
}

fn add_menu(ui: &mut egui::Ui, bookmark: &mut bool, interval: &mut bool) {
    if ui.button("Bookmark (M)").clicked() {
        *bookmark = true;
        ui.close();
    }
    if ui.button("Interval event").clicked() {
        *interval = true;
        ui.close();
    }
}

fn view_menu(
    ui: &mut egui::Ui,
    router: &mut Router,
    state: &mut TimelineUiState,
    diagnostic: &mut Option<String>,
) {
    let timeline = router.timeline_mut();
    if ui.button("Zoom in").clicked() {
        timeline.zoom_around_ms(timeline.setpoint_utc_ms(), 0.5);
        ui.close();
    }
    if ui.button("Zoom out").clicked() {
        timeline.zoom_around_ms(timeline.setpoint_utc_ms(), 2.0);
        ui.close();
    }
    if ui.button("Fit day (F)").clicked() {
        if let Some((start, end)) = timeline.day_bounds_ms() {
            timeline.snap_to_range_ms(start, end);
            state.reset_ruler();
        }
        ui.close();
    }
    ui.horizontal(|ui| {
        ui.label("Duration");
        let mut span = timeline.window_secs();
        if ui
            .add(
                egui::DragValue::new(&mut span)
                    .range(0.1..=86_400.0)
                    .speed(0.5)
                    .suffix(" s"),
            )
            .changed()
        {
            let center = timeline.setpoint_utc_ms();
            let half = ((span * 500.0).round() as i64).max(50);
            timeline.set_start(daqcore::time::Time::from_unix_millis(center - half));
            timeline.set_end(daqcore::time::Time::from_unix_millis(center + half));
            if let Err(error) = timeline.set_window_secs(span) {
                *diagnostic = Some(error);
            }
        }
    });
    let mut fold = timeline.collapse_idle_gaps();
    if ui.checkbox(&mut fold, "Fold gaps (G)").changed() {
        timeline.set_collapse_idle_gaps(fold);
    }
    ui.separator();
    ui.checkbox(&mut state.show_stints, "Show events");
    ui.checkbox(&mut state.show_faults, "Show faults");
}

fn info_menu(ui: &mut egui::Ui, router: &Router) {
    let timeline = router.timeline();
    let provenance = router.clock_provenance_at(timeline.setpoint_utc_ms());
    let resolution = match timeline.resolution_for_width(ui.ctx().content_rect().width()) {
        daqcore::router::Resolution::Overview1Hz => "1 Hz",
        daqcore::router::Resolution::Overview10Hz => "10 Hz",
        daqcore::router::Resolution::FullRes => "Full resolution",
    };
    egui::Grid::new("timeline_info").show(ui, |ui| {
        for (key, value) in [
            (
                "Clock",
                provenance
                    .map(|p| p.badge_label())
                    .unwrap_or_else(|| "No telemetry".to_owned()),
            ),
            ("Resolution", resolution.to_owned()),
            (
                "Cache",
                format!("{} MiB", router.tile_cache().lru_bytes() / (1024 * 1024)),
            ),
            ("Partitions", router.partitions().len().to_string()),
            ("Live frames", router.live_buffer().len().to_string()),
            ("Window start", timeline.start().label()),
            ("Window end", timeline.end().label()),
        ] {
            ui.label(key);
            ui.label(value);
            ui.end_row();
        }
    });
}

fn shortcuts_menu(ui: &mut egui::Ui) {
    egui::Grid::new("timeline_shortcuts").show(ui, |ui| {
        for (action, keys) in [
            ("Play or pause", "Space"),
            ("Step backward", "H or Left"),
            ("Step forward", "L or Right"),
            ("Fine step", "Alt and horizontal arrows"),
            ("Pan window", "Shift and horizontal arrows"),
            ("Zoom", "J or K"),
            ("Previous event", "Left bracket"),
            ("Next event", "Right bracket"),
            ("Fit day", "F"),
            ("Fold gaps", "G"),
            ("Bookmark", "M"),
        ] {
            ui.label(action);
            ui.label(keys);
            ui.end_row();
        }
    });
}

fn fault_count(router: &Router) -> usize {
    let timeline = router.timeline();
    let (start, end) = timeline
        .day_bounds_ms()
        .unwrap_or((timeline.start_utc_ms(), timeline.end_utc_ms()));
    router
        .query_annotations_in_range(start..=end)
        .unwrap_or_default()
        .iter()
        .filter(|annotation| {
            annotation.category == "fault"
                && timeline
                    .active_track_day_id()
                    .is_none_or(|day| annotation.track_day_id == day)
        })
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(
        ctx: &egui::Context,
        router: &mut Router,
        state: &mut TimelineUiState,
        events: Vec<egui::Event>,
    ) -> (egui::FullOutput, Rect) {
        let mut row = Rect::NOTHING;
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1000.0, 320.0),
                )),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    row = Rect::from_min_size(
                        ui.next_widget_position(),
                        egui::vec2(ui.available_width(), ui.spacing().interact_size.y.max(22.0)),
                    );
                    show(ui, router, state, &mut Vec::new(), &mut None);
                    assert!(
                        ui.next_widget_position().y
                            <= row.bottom() + ui.spacing().item_spacing.y + 1.0,
                        "Footer must stay on one row"
                    );
                });
            },
        );
        (output, row)
    }

    fn label_center(output: &egui::FullOutput, label: &str) -> Option<egui::Pos2> {
        fn find(shape: &egui::Shape, label: &str) -> Option<egui::Pos2> {
            match shape {
                egui::Shape::Text(text) if text.galley.text() == label => {
                    Some(text.pos + text.galley.size() * 0.5)
                }
                egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| find(shape, label)),
                _ => None,
            }
        }
        output
            .shapes
            .iter()
            .find_map(|shape| find(&shape.shape, label))
    }

    fn click(
        ctx: &egui::Context,
        router: &mut Router,
        state: &mut TimelineUiState,
        pos: egui::Pos2,
    ) -> egui::FullOutput {
        frame(ctx, router, state, vec![egui::Event::PointerMoved(pos)]);
        for pressed in [true, false] {
            frame(
                ctx,
                router,
                state,
                vec![egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
        }
        frame(ctx, router, state, vec![]).0
    }

    #[test]
    fn compact_footer_playback_events_and_add_work() {
        let ctx = egui::Context::default();
        ui::theme::store_theme(&ctx, ui::theme::ThemeSelection::default().get_colors());
        let mut router =
            Router::live(daqcore::time::Time::from_unix_millis(100_000), 30.0).unwrap();
        router.timeline_mut().snap_to_range_ms(20_000, 60_000);
        let mut state = TimelineUiState::default();
        let (output, row) = frame(&ctx, &mut router, &mut state, vec![]);
        for hidden in [
            "Clear Live",
            "Win:",
            "Fold Gaps",
            "Stints (0)",
            "Faults (0)",
            "Full-Res",
        ] {
            assert!(label_center(&output, hidden).is_none());
        }
        click(&ctx, &mut router, &mut state, row.center());
        assert!(router.timeline().is_playing());
        click(&ctx, &mut router, &mut state, row.center());
        assert!(!router.timeline().is_playing());
        let output = frame(&ctx, &mut router, &mut state, vec![]).0;
        let forward = label_center(&output, "›").unwrap();
        click(&ctx, &mut router, &mut state, forward);
        assert_eq!(router.timeline().setpoint_utc_ms(), 20_100);
        let output = frame(&ctx, &mut router, &mut state, vec![]).0;
        let events = label_center(&output, "Events").unwrap();
        click(&ctx, &mut router, &mut state, events);
        assert_eq!(
            state.requested_sidebar_filter.take(),
            Some(ChunkFilter::StintsAndEvents)
        );
        let output = frame(&ctx, &mut router, &mut state, vec![]).0;
        let add = label_center(&output, "Add").unwrap();
        let output = click(&ctx, &mut router, &mut state, add);
        let bookmark = label_center(&output, "Bookmark (M)").expect("Add opens annotation choices");
        click(&ctx, &mut router, &mut state, bookmark);
        assert!(state.editor.as_ref().unwrap().is_point_bookmark);
    }

    #[test]
    fn overflow_retains_view_settings_and_diagnostics() {
        let ctx = egui::Context::default();
        ui::theme::store_theme(&ctx, ui::theme::ThemeSelection::default().get_colors());
        let mut router =
            Router::live(daqcore::time::Time::from_unix_millis(100_000), 30.0).unwrap();
        router.timeline_mut().snap_to_range_ms(20_000, 60_000);
        let mut state = TimelineUiState::default();
        let (_, row) = frame(&ctx, &mut router, &mut state, vec![]);
        let output = click(
            &ctx,
            &mut router,
            &mut state,
            egui::pos2(row.right() - 11.0, row.center().y),
        );
        for label in ["View", "Info", "Shortcuts"] {
            assert!(label_center(&output, label).is_some(), "Missing {label}");
        }
        let view = label_center(&output, "View").unwrap();
        let output = click(&ctx, &mut router, &mut state, view);
        for label in [
            "Zoom in",
            "Zoom out",
            "Fit day (F)",
            "Duration",
            "Fold gaps (G)",
            "Show events",
            "Show faults",
        ] {
            assert!(label_center(&output, label).is_some(), "Missing {label}");
        }
        let zoom = label_center(&output, "Zoom in").unwrap();
        click(&ctx, &mut router, &mut state, zoom);
        assert_eq!(router.timeline().window_ms(), 20_000);
    }

    #[test]
    fn footer_keeps_play_centered_without_wrapping() {
        for width in [16.0, 100.0, 320.0, 640.0, 1440.0, 2400.0] {
            let row = Rect::from_min_size(egui::pos2(8.0, 70.0), egui::vec2(width, 22.0));
            let layout = FooterLayout::new(row);
            assert_eq!(layout.play.center(), row.center());
            assert_eq!(layout.time.height(), row.height());
            if layout.time.width() > 0.0 {
                assert!(layout.time.right() < layout.play.left());
            }
            if let Some(back) = layout.back {
                assert!(back.right() < layout.play.left());
            }
            if let Some(forward) = layout.forward {
                assert!(forward.left() > layout.play.right());
            }
            if let Some(live) = layout.live {
                assert!(layout.actions.left() > live.right());
            }
        }
    }
}
