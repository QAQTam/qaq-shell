#![windows_subsystem = "windows"]

use egui::{Color32, Context, RichText, Vec2};
use std::time::{Duration, Instant};

fn main() -> Result<(), eframe::Error> {
    eframe::run_native(
        "QAQ-PerfMon",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([460.0, 360.0])
                .with_decorations(true)
                .with_resizable(false),
            ..Default::default()
        },
        Box::new(|cc| {
            let mut fonts = egui::FontDefinitions::default();
            for p in &["C:\\Windows\\Fonts\\msyh.ttc", "X:\\Windows\\Fonts\\msyh.ttc"] {
                if let Ok(d) = std::fs::read(p) {
                    fonts.font_data.insert("CJK".into(), std::sync::Arc::new(egui::FontData::from_owned(d)));
                    fonts.families.entry(egui::FontFamily::Proportional).or_default().insert(0, "CJK".into());
                    break;
                }
            }
            cc.egui_ctx.set_fonts(fonts);
            Ok(Box::new(PerfApp::new()))
        }),
    )
}

#[repr(C)]
#[allow(non_snake_case)]
struct MEMORYSTATUSEX {
    dwLength: u32,
    dwMemoryLoad: u32,
    ullTotalPhys: u64,
    ullAvailPhys: u64,
    ullTotalPageFile: u64,
    ullAvailPageFile: u64,
    ullTotalVirtual: u64,
    ullAvailVirtual: u64,
    ullAvailExtendedVirtual: u64,
}

#[repr(C)]
#[allow(non_snake_case)]
struct DISK_PERFORMANCE {
    BytesRead: i64,
    BytesWritten: i64,
    ReadTime: i64,
    WriteTime: i64,
    IdleTime: i64,
    ReadCount: u32,
    WriteCount: u32,
    QueueDepth: u32,
    SplitCount: u32,
    QueryTime: i64,
    StorageDeviceNumber: u32,
    StorageManagerName: [u16; 8],
}

const IOCTL_DISK_PERFORMANCE: u32 = 0x00070020;

#[link(name = "kernel32")]
extern "system" {
    fn GetSystemTimes(idle: *mut u64, kernel: *mut u64, user: *mut u64) -> i32;
    fn GlobalMemoryStatusEx(buf: *mut MEMORYSTATUSEX) -> i32;
    fn GetDiskFreeSpaceExW(dir: *const u16, avail: *mut u64, total: *mut u64, free: *mut u64) -> i32;
    fn GetLogicalDrives() -> u32;
    fn CreateFileW(name: *const u16, access: u32, share: u32, sec: *mut std::ffi::c_void, creation: u32, flags: u32, tpl: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    fn DeviceIoControl(h: *mut std::ffi::c_void, code: u32, input: *const std::ffi::c_void, in_size: u32, output: *mut std::ffi::c_void, out_size: u32, returned: *mut u32, overlapped: *const std::ffi::c_void) -> i32;
    fn CloseHandle(h: *mut std::ffi::c_void) -> i32;
}

struct PerfApp {
    last_sample: Instant,
    cpu_pct: f32,
    cpu_hist: Vec<f32>,
    mem_hist: Vec<f32>,
    cpu_idle: u64,
    cpu_kernel: u64,
    cpu_user: u64,
    mem_load: u32,
    mem_used: u64,
    mem_total: u64,
    drives: Vec<DriveInfo>,
    disk_bytes_read: i64,
    disk_bytes_written: i64,
    disk_read_speed: u64,
    disk_write_speed: u64,
    last_disk_bytes_read: i64,
    last_disk_bytes_written: i64,
}

#[derive(Clone)]
struct DriveInfo {
    name: String,
    total: u64,
    free: u64,
}

fn open_physical_drive(idx: u32) -> *mut std::ffi::c_void {
    let path: Vec<u16> = format!("\\\\.\\PhysicalDrive{}\0", idx).encode_utf16().collect();
    unsafe {
        CreateFileW(path.as_ptr(), 0, 3, std::ptr::null_mut(), 3, 0, std::ptr::null_mut())
    }
}

fn query_disk_perf(drive_idx: u32) -> Option<DISK_PERFORMANCE> {
    let h = open_physical_drive(drive_idx);
    if h == -1isize as *mut std::ffi::c_void { return None; }
    let mut dp: DISK_PERFORMANCE = unsafe { std::mem::zeroed() };
    let mut returned: u32 = 0;
    let ok = unsafe {
        DeviceIoControl(h, IOCTL_DISK_PERFORMANCE, std::ptr::null(), 0,
            &mut dp as *mut DISK_PERFORMANCE as *mut std::ffi::c_void,
            std::mem::size_of::<DISK_PERFORMANCE>() as u32, &mut returned, std::ptr::null())
    };
    unsafe { CloseHandle(h); }
    if ok == 0 { None } else { Some(dp) }
}

impl PerfApp {
    fn new() -> Self {
        let mut s = Self {
            last_sample: Instant::now(),
            cpu_pct: 0.0, cpu_hist: vec![0.0; 60], mem_hist: vec![0.0; 60],
            cpu_idle: 0, cpu_kernel: 0, cpu_user: 0,
            mem_load: 0, mem_used: 0, mem_total: 0,
            drives: Vec::new(),
            disk_bytes_read: 0, disk_bytes_written: 0,
            disk_read_speed: 0, disk_write_speed: 0,
            last_disk_bytes_read: 0, last_disk_bytes_written: 0,
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

            let mut total_read: i64 = 0;
            let mut total_write: i64 = 0;
            for idx in 0..4 {
                if let Some(dp) = query_disk_perf(idx) {
                    total_read += dp.BytesRead;
                    total_write += dp.BytesWritten;
                }
            }
            let dr = (total_read - self.last_disk_bytes_read).max(0) as u64;
            let dw = (total_write - self.last_disk_bytes_written).max(0) as u64;
            self.disk_read_speed = dr;
            self.disk_write_speed = dw;
            self.disk_bytes_read = total_read;
            self.disk_bytes_written = total_write;
            self.last_disk_bytes_read = total_read;
            self.last_disk_bytes_written = total_write;
        }
    }
}

fn fmt_speed(bytes_per_sec: u64) -> String {
    if bytes_per_sec >= 1_000_000_000 { format!("{:.1} GB/s", bytes_per_sec as f64 / 1e9) }
    else if bytes_per_sec >= 1_000_000 { format!("{:.1} MB/s", bytes_per_sec as f64 / 1e6) }
    else if bytes_per_sec >= 1_000 { format!("{:.1} KB/s", bytes_per_sec as f64 / 1e3) }
    else { format!("{} B/s", bytes_per_sec) }
}

fn section_label(ui: &mut egui::Ui, text: &str, value: &str, color: Color32) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(text).size(14.0).color(ui.visuals().text_color()));
        ui.label(RichText::new(value).size(14.0).color(color));
    });
}

impl eframe::App for PerfApp {
    fn update(&mut self, ctx: &Context, _: &mut eframe::Frame) {
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
        ctx.set_visuals(egui::Visuals::dark());

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
            let tc = ui.visuals().text_color();

            section_label(ui, "CPU", &format!("{:.1}%", self.cpu_pct),
                if self.cpu_pct > 80.0 { Color32::RED } else { tc });

            let (resp, painter) = ui.allocate_painter(Vec2::new(ui.available_width(), 52.0), egui::Sense::hover());
            render_history(&painter, resp.rect, &self.cpu_hist, Color32::from_rgb(0, 180, 255));
            ui.separator();

            ui.horizontal(|ui| {
                ui.label(RichText::new("内存").size(14.0).color(tc));
                ui.label(RichText::new(format!("  {}%  已用 {:.1}/{:.1} GB", self.mem_load,
                    self.mem_used as f64 / 1e9, self.mem_total as f64 / 1e9)).size(14.0).color(tc));
            });
            let (resp2, painter2) = ui.allocate_painter(Vec2::new(ui.available_width(), 52.0), egui::Sense::hover());
            render_history(&painter2, resp2.rect, &self.mem_hist, Color32::from_rgb(255, 140, 0));
            ui.separator();

            ui.horizontal(|ui| {
                ui.label(RichText::new("磁盘").size(14.0).color(tc));
                ui.label(RichText::new(format!("  读取 {}  写入 {}",
                    fmt_speed(self.disk_read_speed), fmt_speed(self.disk_write_speed))).size(14.0).color(tc));
            });
            for d in &self.drives {
                let used = d.total - d.free;
                let pct = if d.total > 0 { used as f32 / d.total as f32 } else { 0.0 };
                ui.horizontal(|ui| { ui.label(RichText::new(format!(" {}  ", d.name)).size(13.0).color(tc)); });
                ui.add(egui::ProgressBar::new(pct).desired_width(ui.available_width())
                    .text(format!("{:.1}/{:.1} GB", used as f64 / 1e9, d.total as f64 / 1e9)));
            }

            ui.with_layout(egui::Layout::bottom_up(egui::Align::RIGHT), |ui| {
                ui.label(RichText::new("间隔 1s").size(11.0).color(Color32::DARK_GRAY));
            });
            });
        });
    }
}

fn render_history(painter: &egui::Painter, rect: egui::Rect, data: &[f32], color: Color32) {
    let n = data.len();
    if n < 2 { return; }
    let w = rect.width();
    let h = rect.height();
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
