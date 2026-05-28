#![windows_subsystem = "windows"]

use egui::{Align, Color32, Context, FontData, FontDefinitions, FontFamily, Id, Layout, RichText, Vec2};
use std::os::windows::ffi::OsStrExt;
use std::sync::Arc;
use windows_sys::Win32::System::Time::{GetTimeZoneInformation, TIME_ZONE_INFORMATION};
use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindow, GetWindowLongW, GetWindowTextW, GetWindowThreadProcessId, IsIconic,
    IsWindowVisible, ShowWindow, SW_MINIMIZE, SW_RESTORE, SW_SHOW,
};

fn main() -> Result<(), eframe::Error> {
    eframe::run_native(
        "QAQ-Shell",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_fullscreen(true)
                .with_decorations(false),
            ..Default::default()
        },
        Box::new(|cc| {
            setup_fonts(&cc.egui_ctx);
            Ok(Box::new(ShellApp::default()))
        }),
    )
}

fn setup_fonts(ctx: &Context) {
    let mut f = FontDefinitions::default();
    for p in &[
        "C:\\Windows\\Fonts\\msyh.ttc",
        "C:\\Windows\\Fonts\\simsun.ttc",
        "X:\\Windows\\Fonts\\msyh.ttc",
    ] {
        if let Ok(d) = std::fs::read(p) {
            f.font_data
                .insert("CJK".into(), Arc::new(FontData::from_owned(d)));
            f.families
                .entry(FontFamily::Proportional)
                .or_default()
                .insert(0, "CJK".into());
            f.families
                .entry(FontFamily::Monospace)
                .or_default()
                .push("CJK".into());
            break;
        }
    }
    ctx.set_fonts(f);
}

fn dpi(ctx: &Context) -> f32 {
    ctx.pixels_per_point().clamp(0.8, 1.5)
}

fn timezone_bias() -> i32 {
    unsafe {
        let mut tz = std::mem::zeroed::<TIME_ZONE_INFORMATION>();
        let r = GetTimeZoneInformation(&mut tz);
        if r == 2 {
            tz.Bias + tz.DaylightBias
        } else {
            tz.Bias
        }
    }
}

fn clock(bias: i32) -> String {
    let s = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
        - bias as i64 * 60)
        .rem_euclid(86400);
    format!(
        "{:02}:{:02}:{:02}",
        (s / 3600) % 24,
        (s / 60) % 60,
        s % 60
    )
}

#[derive(Clone)]
struct DesktopIcon {
    name: String,
    path: String,
}

#[derive(Clone)]
struct WindowInfo {
    hwnd: isize,
    title: String,
}

struct ShellApp {
    clock: String,
    start_open: bool,
    icons: Vec<DesktopIcon>,
    windows: Vec<WindowInfo>,
    last_window_refresh: std::time::Instant,
    tz_bias: i32,
    active_window: isize,
}

impl Default for ShellApp {
    fn default() -> Self {
        Self {
            clock: String::new(),
            start_open: false,
            icons: icons(),
            windows: Vec::new(),
            last_window_refresh: std::time::Instant::now(),
            tz_bias: timezone_bias(),
            active_window: 0,
        }
    }
}

fn icons() -> Vec<DesktopIcon> {
    let mut v = vec![];
    for d in &[
        "X:\\Users\\Default\\Desktop",
        "X:\\Users\\Public\\Desktop",
        "C:\\Users\\Public\\Desktop",
    ] {
        if let Ok(e) = std::fs::read_dir(d) {
            for x in e.flatten() {
                let p = x.path();
                if p.extension().map(|e| e == "lnk").unwrap_or(false) {
                    v.push(DesktopIcon {
                        name: p
                            .file_stem()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default(),
                        path: p.to_string_lossy().to_string(),
                    });
                }
            }
        }
    }
    if v.is_empty() {
        for (n, p) in &[
            ("CMD", "cmd.exe"),
            ("DiskGenius", "X:\\WinXShell\\DiskGenius.exe"),
            ("WinNTSetup", "X:\\Tools\\WinNTSetup\\WinNTSetup_x64.exe"),
            ("servtool", "X:\\Tools\\servtool.exe"),
            ("Firefox", "X:\\Tools\\Firefox\\firefox.exe"),
            ("7-Zip", "X:\\Tools\\7-Zip\\7zFM.exe"),
            ("Notepad3", "X:\\Tools\\Notepad3\\Notepad3.exe"),
        ] {
            v.push(DesktopIcon {
                name: n.to_string(),
                path: p.to_string(),
            });
        }
    }
    v
}

fn launch(app: &str) {
    let (exe, args) = if let Some(p) = app.find(' ') {
        let head = &app[..p];
        if head.contains('\\') || head.contains(':') {
            (app, "")
        } else {
            (head, &app[p + 1..])
        }
    } else {
        (app, "")
    };
    let we: Vec<u16> = std::ffi::OsStr::new(exe)
        .encode_wide()
        .chain(Some(0))
        .collect();
    let wo: Vec<u16> = std::ffi::OsStr::new("open")
        .encode_wide()
        .chain(Some(0))
        .collect();
    let wa: Vec<u16> = if args.is_empty() {
        vec![0]
    } else {
        std::ffi::OsStr::new(args)
            .encode_wide()
            .chain(Some(0))
            .collect()
    };
    let wa_ptr = if args.is_empty() {
        std::ptr::null()
    } else {
        wa.as_ptr()
    };
    unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            wo.as_ptr(),
            we.as_ptr(),
            wa_ptr,
            std::ptr::null(),
            5,
        );
    }
}

fn enumerate_windows() -> Vec<WindowInfo> {
    struct Ctx {
        windows: Vec<WindowInfo>,
        our_pid: u32,
    }
    extern "system" fn callback(hwnd: *mut std::ffi::c_void, lparam: isize) -> i32 {
        unsafe {
            let ctx = &mut *(lparam as *mut Ctx);
            if IsWindowVisible(hwnd) == 0 {
                return 1;
            }
            let ex = GetWindowLongW(hwnd, -20) as u32;
            if ex & 0x80 != 0 {
                return 1;
            }
            if (ex & 0x08000000) != 0 {
                return 1;
            }
            let style = GetWindowLongW(hwnd, -16) as u32;
            if (style & 0x40000000) != 0 {
                return 1;
            }
            let owner = GetWindow(hwnd, 4);
            if !owner.is_null() && ex & 0x40000 == 0 {
                return 1;
            }
            let mut pid: u32 = 0;
            GetWindowThreadProcessId(hwnd, &mut pid);
            if pid == ctx.our_pid {
                return 1;
            }
            let mut text: [u16; 512] = [0; 512];
            let len = GetWindowTextW(hwnd, text.as_mut_ptr(), 512);
            if len == 0 {
                return 1;
            }
            let title = String::from_utf16_lossy(&text[..len as usize]);
            ctx.windows
                .push(WindowInfo { hwnd: hwnd as isize, title });
            1
        }
    }
    let mut ctx = Ctx {
        windows: Vec::new(),
        our_pid: std::process::id(),
    };
    unsafe {
        EnumWindows(Some(callback), &mut ctx as *mut Ctx as isize);
    }
    ctx.windows
}

fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        format!("{}...", s.chars().take(max_chars).collect::<String>())
    }
}

fn merge_windows(old: &[WindowInfo], new: &[WindowInfo]) -> Vec<WindowInfo> {
    let new_map: std::collections::HashMap<isize, &WindowInfo> =
        new.iter().map(|w| (w.hwnd, w)).collect();
    let mut result: Vec<WindowInfo> = old
        .iter()
        .filter_map(|w| {
            new_map.get(&w.hwnd).map(|nw| WindowInfo {
                hwnd: w.hwnd,
                title: nw.title.clone(),
            })
        })
        .collect();
    let existing: std::collections::HashSet<isize> = result.iter().map(|w| w.hwnd).collect();
    for w in new {
        if !existing.contains(&w.hwnd) {
            result.push(w.clone());
        }
    }
    result
}

fn focus_window(hwnd: isize) {
    #[link(name = "user32")]
    extern "system" {
        fn SwitchToThisWindow(hwnd: *mut std::ffi::c_void, fAltTab: i32);
    }
    unsafe {
        let h = hwnd as *mut std::ffi::c_void;
        ShowWindow(h, SW_SHOW);
        if IsIconic(h) != 0 {
            ShowWindow(h, SW_RESTORE);
        }
        SwitchToThisWindow(h, 1);
    }
}

fn ds() -> Arc<egui::Style> {
    let mut v = egui::Visuals::dark();
    v.widgets.inactive.bg_fill = Color32::from_rgba_unmultiplied(255, 255, 255, 8);
    v.widgets.hovered.bg_fill = Color32::from_rgba_unmultiplied(255, 255, 255, 25);
    v.widgets.active.bg_fill = Color32::from_rgba_unmultiplied(255, 255, 255, 40);
    Arc::new(egui::Style {
        visuals: v,
        ..Default::default()
    })
}

impl eframe::App for ShellApp {
    fn update(&mut self, ctx: &Context, _: &mut eframe::Frame) {
        self.clock = clock(self.tz_bias);
        let scr = ctx.screen_rect();
        let s = dpi(ctx);
        ctx.set_visuals(egui::Visuals::dark());
        ctx.request_repaint_after(std::time::Duration::from_secs(1));

        let th = (42.0 * s).round();
        let bw = (48.0 * s).round();
        let aw = (40.0 * s).round();
        let iw = (172.0 * s).round();
        let ih = (38.0 * s).round();
        let fl = (18.0 * s).round();
        let fm = (13.0 * s).round();
        let fs = (10.0 * s).round();
        let fx = (12.0 * s).round();
        let ww = (132.0 * s).round();

        let now = std::time::Instant::now();
        if now.duration_since(self.last_window_refresh).as_secs() >= 2 {
            self.windows = merge_windows(&self.windows, &enumerate_windows());
            self.last_window_refresh = now;
            if self.active_window != 0
                && !self.windows.iter().any(|w| w.hwnd == self.active_window)
            {
                self.active_window = 0;
            }
        }

        // Desktop
        let dk = egui::Rect::from_min_size(scr.min, Vec2::new(scr.width(), scr.height() - th));
        egui::Area::new(Id::new("dk"))
            .fixed_pos(dk.min)
            .show(ctx, |ui| {
                ui.allocate_ui_at_rect(dk, |ui| {
                    ui.painter()
                        .rect_filled(dk, 0.0, Color32::from_rgb(10, 40, 80));
                });
            });

        // Icons
        egui::Area::new(Id::new("ic"))
            .fixed_pos(scr.min)
            .show(ctx, |ui| {
                ui.allocate_ui_at_rect(
                    egui::Rect::from_min_size(
                        scr.min + Vec2::new(16.0 * s, 16.0 * s),
                        Vec2::new(iw + 20.0, scr.height() - th - 32.0 * s),
                    ),
                    |ui| {
                        for i in &self.icons {
                            if ui
                                .add_sized(
                                    [iw, ih],
                                    egui::Button::new(
                                        RichText::new(format!("📄 {}", i.name))
                                            .size(fm)
                                            .color(Color32::WHITE),
                                    )
                                    .fill(Color32::TRANSPARENT),
                                )
                                .clicked()
                            {
                                launch(&i.path);
                            }
                        }
                    },
                );
            });

        // Taskbar
        let tb = egui::Rect::from_min_size(
            scr.min + Vec2::new(0.0, scr.height() - th),
            Vec2::new(scr.width(), th),
        );
        egui::Area::new(Id::new("tb"))
            .fixed_pos(tb.min)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                ui.allocate_ui_at_rect(tb, |ui| {
                    ui.painter()
                        .rect_filled(tb, 0.0, Color32::from_rgb(24, 24, 28));
                    ui.set_style(ds());
                    ui.horizontal(|ui| {
                        // Start button
                        if ui
                            .add_sized(
                                [bw, th],
                                egui::Button::new(
                                    RichText::new("\u{229E}")
                                        .size(fl)
                                        .color(Color32::WHITE),
                                )
                                .fill(Color32::TRANSPARENT),
                            )
                            .clicked()
                        {
                            self.start_open = !self.start_open;
                        }
                        ui.add_space(4.0 * s);

                        // Running windows
                        for w in self.windows.iter().take(5) {
                            let label = truncate(&w.title, 12);
                            let active_fill =
                                Color32::from_rgba_unmultiplied(255, 255, 255, 40);
                            if ui
                                .add_sized(
                                    [ww, th],
                                    egui::Button::new(
                                        RichText::new(label)
                                            .size(fs)
                                            .color(Color32::WHITE),
                                    )
                                    .fill(active_fill),
                                )
                                .clicked()
                            {
                                if self.active_window == w.hwnd {
                                    unsafe {
                                        ShowWindow(
                                            w.hwnd as *mut std::ffi::c_void,
                                            SW_MINIMIZE,
                                        );
                                    }
                                    self.active_window = 0;
                                } else {
                                    focus_window(w.hwnd);
                                    self.active_window = w.hwnd;
                                }
                            }
                        }

                        // Pinned launchers (left group, before the right-aligned section)
                        for i in &self.icons {
                            let l = &i.name[..i.name.len().min(3)];
                            if ui
                                .add_sized(
                                    [aw, th],
                                    egui::Button::new(
                                        RichText::new(l)
                                            .size(fs)
                                            .color(Color32::WHITE),
                                    )
                                    .fill(Color32::TRANSPARENT),
                                )
                                .clicked()
                            {
                                launch(&i.path);
                            }
                        }

                        // Right side: clock + shutdown
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(
                                RichText::new(&self.clock)
                                    .color(Color32::WHITE)
                                    .size(fx),
                            );
                            if ui
                                .add_sized(
                                    [bw, th],
                                    egui::Button::new(
                                        RichText::new("\u{23FB}")
                                            .size(fl)
                                            .color(Color32::RED),
                                    )
                                    .fill(Color32::TRANSPARENT),
                                )
                                .clicked()
                            {
                                launch("wpeutil.exe shutdown");
                            }
                        });
                    });
                });
            });

        // Start menu
        if self.start_open {
            let mh = (280.0 * s).round();
            let mw = (240.0 * s).round();
            let mp = egui::pos2(scr.min.x, scr.min.y + scr.height() - th - mh);
            egui::Area::new(Id::new("sm"))
                .fixed_pos(mp)
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    ui.set_style(ds());
                    ui.allocate_ui_at_rect(
                        egui::Rect::from_min_size(mp, Vec2::new(mw, mh)),
                        |uj| {
                            uj.painter().rect_filled(
                                egui::Rect::from_min_size(mp, Vec2::new(mw, mh)),
                                8.0 * s,
                                Color32::from_rgba_unmultiplied(32, 32, 36, 250),
                            );
                            uj.add_space(8.0 * s);
                            uj.heading(
                                RichText::new("  QAQ-PE")
                                    .color(Color32::CYAN)
                                    .size(fl),
                            );
                            uj.separator();
                            egui::ScrollArea::vertical()
                                .max_height((mh * 0.6).round())
                                .show(uj, |ui2| {
                                    for icon in &self.icons {
                                        if ui2
                                            .button(
                                                RichText::new(format!("  {}", icon.name))
                                                    .color(Color32::WHITE)
                                                    .size(fm),
                                            )
                                            .clicked()
                                        {
                                            launch(&icon.path);
                                            self.start_open = false;
                                        }
                                    }
                                });
                            uj.separator();
                            uj.horizontal(|ui3| {
                                if ui3
                                    .button(
                                        RichText::new("  重启  ")
                                            .color(Color32::WHITE)
                                            .size(fm),
                                    )
                                    .clicked()
                                {
                                    launch("wpeutil.exe reboot");
                                }
                                if ui3
                                    .button(
                                        RichText::new("  关机  ")
                                            .color(Color32::RED)
                                            .size(fm),
                                    )
                                    .clicked()
                                {
                                    launch("wpeutil.exe shutdown");
                                }
                            });
                        },
                    );
                });
            if ctx.input(|i| i.pointer.button_clicked(egui::PointerButton::Primary)) {
                if let Some(p) = ctx.input(|i| i.pointer.hover_pos()) {
                    if p.y < scr.height() - th - mh {
                        self.start_open = false;
                    }
                }
            }
        }
    }
}
