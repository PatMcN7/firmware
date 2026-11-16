pub mod bootloader;
pub mod bus_load;
pub mod command_palette;
pub mod export_modal;
pub mod gps_plot;
pub mod hil;
pub mod jitter;
pub mod log_parser;
pub mod message_picker;
pub mod scope;
pub mod send;
pub mod sidebar;
pub mod signal_browser;
pub mod state_lane;
pub mod theme;
pub mod timeline;
mod timeline_controls;
pub mod viewer_list;
pub mod viewer_table;
pub mod xy_scatter;

/// Gentle proportional zoom shared by plots and the timeline.
/// A typical 40-point wheel step changes the span by about six percent.
pub(crate) fn scroll_zoom_factor(delta: f32) -> f64 {
    if !delta.is_finite() {
        return 1.0;
    }
    (-f64::from(delta) * 0.0015).clamp(-0.2, 0.2).exp()
}

#[cfg(test)]
mod tests {
    use super::scroll_zoom_factor;

    #[test]
    fn scroll_zoom_is_gentle_proportional_and_frame_independent() {
        assert_eq!(scroll_zoom_factor(0.0), 1.0);
        assert_eq!(scroll_zoom_factor(f32::NAN), 1.0);
        assert_eq!(scroll_zoom_factor(f32::INFINITY), 1.0);
        assert!(scroll_zoom_factor(1.0) > 0.998);
        assert!(scroll_zoom_factor(40.0) > 0.94);
        assert!(scroll_zoom_factor(40.0) < 1.0);
        assert!((scroll_zoom_factor(40.0) * scroll_zoom_factor(-40.0) - 1.0).abs() < 1e-12);
        assert!((scroll_zoom_factor(4.0).powi(10) - scroll_zoom_factor(40.0)).abs() < 1e-12);
        assert!(scroll_zoom_factor(f32::MAX) > 0.8);
    }
}
