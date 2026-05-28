use super::{ds, Scale, ShellWidget};
use egui::{Color32, Context, Id, RichText, Vec2};
use std::time::{Duration, Instant};

#[repr(C)]
#[allow(non_snake_case)]
struct MEMORYSTATUSEX {
    dwLength: u32, dwMemoryLoad: u32,
    ullTotalPhys: u64, ullAvailPhys: u64,
    ullTotalPageFile: u64, ullAvailPageFile: u64,
    ullTotalVirtual: u64, ullAvailVirtual: u64,
    ullAvailExtendedVirtual: u64,
}

#[link(name = "kernel32")]
extern "system" {
    fn GetSystemTimes(idle: *mut u64, kernel: *mut u64, user: *mut u64) -> i32;
    fn GlobalMemoryStatusEx(buf: *mut MEMORYSTATUSEX) -> i32;
    fn GetDiskFreeSpaceExW(dir: *const u16, avail: *mut u64, total: *mut u64, free: *mut u64) -> i32;
    fn GetLogicalDrives() -> u32;
}

pub struct PerfMon {
    last_sample: Instant,
    cpu_pct: f32,
    cpu_hist: Vec<f32>,
    cpu_idle: u64, cpu_kernel: u64, cpu_user: u64,
    mem_hist: Vec<f32>,
    mem_load: u32, mem_used: u64, mem_total: u64,
    drives: Vec<DriveInfo>,
    style: std::sync::Arc<egui::Style>,
}

#[derive(Clone)]
struct DriveInfo { name: String, total: u64, free: u64 }

fn render_history(painter: &egui::Painter, rect: egui::Rect, data: &[f32], color: Color32) {
    let n = data.len();
    if n < 2 { return; }
    let w = rect.width(); let h = rect.height();
    let step = w / (n - 1) as f32;
    let fill = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 40);
    let line = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 180);
    let mut points: Vec<egui::Pos2> = Vec::with_capacity(n);
    for (i, &v) in data.iter().enumerate() {
        points.push(egui::pos2(rect.min.x + i as f32 * step, rect.max.y - (v / 100.0) * h));
    }
    for w in points.windows(2) {
        let y_t = w[0].y.min(w[1].y);
        painter.rect_filled(egui::Rect::from_min_max(egui::pos2(w[0].x, y_t), egui::pos2(w[1].x, rect.max.y)), 0.0, fill);
        painter.line_segment([w[0], w[1]], egui::Stroke::new(1.2, line));
    }
}

impl PerfMon {
    pub fn new() -> Self {
        let mut s = Self {
            last_sample: Instant::now(),
            cpu_pct: 0.0, cpu_hist: vec![0.0; 60], mem_hist: vec![0.0; 60],
            cpu_idle: 0, cpu_kernel: 0, cpu_user: 0,
            mem_load: 0, mem_used: 0, mem_total: 0,
            drives: Vec::new(),
            style: ds(),
        };
        s.sample();
        s
    }

    fn sample(&mut self) {
        unsafe {
            let (mut i, mut k, mut u) = (0u64, 0u64, 0u64);
            if GetSystemTimes(&mut i, &mut k, &mut u) != 0 {
                let di = i.saturating_sub(self.cpu_idle);
                let dt = (k + u).saturating_sub(self.cpu_kernel + self.cpu_user);
                if dt > 0 { self.cpu_pct = (100.0 * (1.0 - di as f32 / dt as f32)).clamp(0.0, 100.0); }
                self.cpu_idle = i; self.cpu_kernel = k; self.cpu_user = u;
            }
            let mut mem: MEMORYSTATUSEX = std::mem::zeroed();
            mem.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
            if GlobalMemoryStatusEx(&mut mem) != 0 {
                self.mem_load = mem.dwMemoryLoad;
                self.mem_used = mem.ullTotalPhys - mem.ullAvailPhys;
                self.mem_total = mem.ullTotalPhys;
            }
            let drives = GetLogicalDrives();
            self.drives.clear();
            for i in 0..26 {
                if drives & (1 << i) != 0 {
                    let pw: Vec<u16> = format!("{}:\\\0", (b'A' + i as u8) as char).encode_utf16().collect();
                    let (mut a, mut t, mut f) = (0u64, 0u64, 0u64);
                    GetDiskFreeSpaceExW(pw.as_ptr(), &mut a, &mut t, &mut f);
                    if t > 0 {
                        self.drives.push(DriveInfo { name: format!("{}:", (b'A' + i as u8) as char), total: t, free: f });
                    }
                }
            }
        }
    }
}

impl ShellWidget for PerfMon {
    fn name(&self) -> &str { "性能监视器" }

    fn show(&mut self, ctx: &Context, sc: &Scale, scr: egui::Rect, _open: &mut bool) {
        let now = Instant::now();
        if now.duration_since(self.last_sample) >= Duration::from_secs(1) {
            self.sample();
            self.last_sample = now;
            self.cpu_hist.rotate_left(1);
            *self.cpu_hist.last_mut().unwrap() = self.cpu_pct;
            self.mem_hist.rotate_left(1);
            *self.mem_hist.last_mut().unwrap() = self.mem_load as f32;
        }
        ctx.request_repaint_after(Duration::from_millis(500));

        let s = sc.dpi;
        let pw = 460.0 * s; let ph = 380.0 * s;
        let px = (scr.width() - pw) / 2.0;
        let py = (scr.height() - sc.th - ph) / 2.0 + 24.0 * s;
        let pr = egui::Rect::from_min_size(egui::pos2(px, py), Vec2::new(pw, ph));

        egui::Area::new(Id::new("perfmon")).order(egui::Order::Foreground).fixed_pos(pr.min).show(ctx, |ui| {
            ui.set_style(self.style.clone());
            ui.allocate_ui_at_rect(pr, |ui| {
                ui.painter().rect_filled(pr, 6.0, Color32::from_rgba_unmultiplied(30, 30, 36, 248));
                ui.painter().rect_stroke(pr, 6.0, egui::Stroke::new(0.5, Color32::from_rgba_unmultiplied(255, 255, 255, 30)), egui::StrokeKind::Inside);
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.add_space(6.0);
                    let tc = ui.visuals().text_color();
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("CPU").size(14.0).color(tc));
                        ui.label(RichText::new(format!("  {:.1}%", self.cpu_pct)).size(14.0)
                            .color(if self.cpu_pct > 80.0 { Color32::RED } else { tc }));
                    });
                    let (r1, p1) = ui.allocate_painter(Vec2::new(pw - 16.0 * s, 52.0), egui::Sense::hover());
                    render_history(&p1, r1.rect, &self.cpu_hist, Color32::from_rgb(0, 180, 255));
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("内存").size(14.0).color(tc));
                        ui.label(RichText::new(format!("  {}%  已用 {:.1}/{:.1} GB", self.mem_load,
                            self.mem_used as f64 / 1e9, self.mem_total as f64 / 1e9)).size(14.0).color(tc));
                    });
                    let (r2, p2) = ui.allocate_painter(Vec2::new(pw - 16.0 * s, 52.0), egui::Sense::hover());
                    render_history(&p2, r2.rect, &self.mem_hist, Color32::from_rgb(255, 140, 0));
                    ui.separator();
                    ui.label(RichText::new("磁盘").size(14.0).color(tc));
                    for d in &self.drives {
                        let used = d.total - d.free;
                        let pct = if d.total > 0 { used as f32 / d.total as f32 } else { 0.0 };
                        ui.horizontal(|ui| { ui.label(RichText::new(format!(" {}  ", d.name)).size(13.0).color(tc)); });
                        ui.add(egui::ProgressBar::new(pct).desired_width(pw - 24.0 * s)
                            .text(format!("{:.1}/{:.1} GB", used as f64 / 1e9, d.total as f64 / 1e9)));
                    }
                    ui.add_space(4.0);
                    ui.with_layout(egui::Layout::bottom_up(egui::Align::RIGHT), |ui| {
                        ui.label(RichText::new("间隔 1s").size(11.0).color(Color32::DARK_GRAY));
                    });
                });
            });
        });
    }
}
