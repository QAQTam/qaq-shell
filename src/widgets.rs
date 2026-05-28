use egui::{Color32, Context};
use std::sync::Arc;

pub struct Scale {
    pub dpi: f32,
    pub th: f32,
    pub bw: f32,
    pub iw: f32,
    pub is: f32,
    pub fl: f32,
    pub fm: f32,
    pub fs: f32,
    pub fd: f32,
    pub fx: f32,
    pub ww: f32,
}

impl Scale {
    pub fn new(s: f32) -> Self {
        Scale {
            dpi: s,
            th: (42.0 * s).round(),
            bw: (48.0 * s).round(),
            iw: (172.0 * s).round(),
            is: (32.0 * s).round(),
            fl: (18.0 * s).round(),
            fm: (13.0 * s).round(),
            fs: (10.0 * s).round(),
            fd: (14.0 * s).round(),
            fx: (12.0 * s).round(),
            ww: (100.0 * s).round(),
        }
    }
}

pub fn ds() -> Arc<egui::Style> {
    let mut v = egui::Visuals::dark();
    v.widgets.inactive.bg_fill = Color32::from_rgba_unmultiplied(255, 255, 255, 8);
    v.widgets.hovered.bg_fill = Color32::from_rgba_unmultiplied(255, 255, 255, 25);
    v.widgets.active.bg_fill = Color32::from_rgba_unmultiplied(255, 255, 255, 40);
    Arc::new(egui::Style { visuals: v, ..Default::default() })
}

pub trait ShellWidget {
    fn name(&self) -> &str;
    fn show(&mut self, ctx: &Context, sc: &Scale, scr: egui::Rect, open: &mut bool);
}

pub mod perfmon;
