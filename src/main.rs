#![windows_subsystem = "windows"]

use std::os::windows::ffi::OsStrExt;
use egui::{Align, Color32, Context, Id, Layout, RichText, Vec2, FontDefinitions, FontData, FontFamily};
use std::sync::Arc;

fn main() -> Result<(), eframe::Error> {
    eframe::run_native(
        "QAQ-Shell",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_fullscreen(true).with_decorations(false),
            ..Default::default()
        },
        Box::new(|cc| { setup_fonts(&cc.egui_ctx); Ok(Box::new(ShellApp::default())) }),
    )
}

fn setup_fonts(ctx: &Context) {
    let mut f = FontDefinitions::default();
    for p in &["C:\\Windows\\Fonts\\msyh.ttc", "C:\\Windows\\Fonts\\simsun.ttc", "X:\\Windows\\Fonts\\msyh.ttc"] {
        if let Ok(d) = std::fs::read(p) {
            f.font_data.insert("CJK".into(), Arc::new(FontData::from_owned(d)));
            f.families.entry(FontFamily::Proportional).or_default().insert(0, "CJK".into());
            f.families.entry(FontFamily::Monospace).or_default().push("CJK".into());
            break;
        }
    }
    ctx.set_fonts(f);
}

// ── DPI-aware sizing ──

fn dpi(ctx: &Context) -> f32 { ctx.pixels_per_point().clamp(0.8, 1.5) }
fn tb_h(ctx: &Context) -> f32 { (42.0 * dpi(ctx)).round() }
fn icon_w(ctx: &Context) -> f32 { (172.0 * dpi(ctx)).round() }
fn icon_h(ctx: &Context) -> f32 { (38.0 * dpi(ctx)).round() }
fn tb_btn_w(ctx: &Context) -> f32 { (48.0 * dpi(ctx)).round() }
fn tb_app_w(ctx: &Context) -> f32 { (40.0 * dpi(ctx)).round() }

struct ShellApp { clock: String, start_open: bool, icons: Vec<DesktopIcon> }
#[derive(Clone)] struct DesktopIcon { name: String, path: String }

impl Default for ShellApp {
    fn default() -> Self { Self { clock: String::new(), start_open: false, icons: icons() } }
}

fn icons() -> Vec<DesktopIcon> {
    let mut v = vec![];
    for d in &["X:\\Users\\Default\\Desktop", "X:\\Users\\Public\\Desktop", "C:\\Users\\Public\\Desktop"] {
        if let Ok(entries) = std::fs::read_dir(d) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.extension().map(|e| e == "lnk").unwrap_or(false) {
                    v.push(DesktopIcon {
                        name: p.file_stem().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
                        path: p.to_string_lossy().to_string(),
                    });
                }
            }
        }
    }
    if v.is_empty() {
        for (n, p) in &[("CMD","cmd.exe"),("DiskGenius","X:\\WinXShell\\DiskGenius.exe"),
            ("WinNTSetup","X:\\Tools\\WinNTSetup\\WinNTSetup_x64.exe"),("servtool","X:\\Tools\\servtool.exe"),
            ("Firefox","X:\\Tools\\Firefox\\firefox.exe"),("7-Zip","X:\\Tools\\7-Zip\\7zFM.exe"),
            ("Notepad3","X:\\Tools\\Notepad3\\Notepad3.exe")] {
            v.push(DesktopIcon { name: n.to_string(), path: p.to_string() });
        }
    }
    v
}

fn clock() -> String {
    let s = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() + 28800;
    format!("{:02}:{:02}:{:02}", (s/3600)%24, (s/60)%60, s%60)
}

fn launch(app: &str) {
    let (exe, args) = if let Some(pos) = app.find(' ') { (&app[..pos], &app[pos+1..]) } else { (app, "") };
    let w_exe: Vec<u16> = std::ffi::OsStr::new(exe).encode_wide().chain(Some(0)).collect();
    let w_args: Vec<u16> = if args.is_empty() { vec![0] } else { std::ffi::OsStr::new(args).encode_wide().chain(Some(0)).collect() };
    let w_op: Vec<u16> = std::ffi::OsStr::new("open").encode_wide().chain(Some(0)).collect();
    unsafe { ShellExecuteW(std::ptr::null_mut(), w_op.as_ptr(), w_exe.as_ptr(), if args.is_empty() { std::ptr::null() } else { w_args.as_ptr() }, std::ptr::null(), 5); }
}

#[link(name = "shell32")] extern "system" {
    fn ShellExecuteW(hwnd: *mut std::ffi::c_void, op: *const u16, file: *const u16, params: *const u16, dir: *const u16, show: i32) -> isize;
}

fn dark_style() -> Arc<egui::Style> {
    let mut v = egui::Visuals::dark();
    v.widgets.inactive.bg_fill = Color32::from_rgba_unmultiplied(255,255,255,8);
    v.widgets.hovered.bg_fill = Color32::from_rgba_unmultiplied(255,255,255,25);
    v.widgets.active.bg_fill = Color32::from_rgba_unmultiplied(255,255,255,40);
    Arc::new(egui::Style { visuals: v, ..Default::default() })
}

impl eframe::App for ShellApp {
    fn update(&mut self, ctx: &Context, _: &mut eframe::Frame) {
        self.clock = clock();
        let scr = ctx.screen_rect();
        let dp = dpi(ctx);
        let tbh = tb_h(ctx);
        let iw = icon_w(ctx);
        let ih = icon_h(ctx);
        let tbw = tb_btn_w(ctx);
        let taw = tb_app_w(ctx);

        let fs_lg = font_lg(dp);
        let fs_md = font_md(dp);
        let fs_sm = font_sm(dp);
        let fs_xs = font_xs(dp);

        ctx.set_visuals(egui::Visuals::dark());
        ctx.request_repaint_after(std::time::Duration::from_secs(1));

        // ── Desktop ──
        let desk = egui::Rect::from_min_size(scr.min, Vec2::new(scr.width(), scr.height()-tbh));
        egui::Area::new(Id::new("desk")).fixed_pos(desk.min).show(ctx, |ui| {
            ui.allocate_ui_at_rect(desk, |ui| { ui.painter().rect_filled(desk, 0.0, Color32::from_rgb(10,40,80)); });
        });

        // Desktop icons
        egui::Area::new(Id::new("icons")).fixed_pos(scr.min).show(ctx, |ui| {
            ui.allocate_ui_at_rect(
                egui::Rect::from_min_size(scr.min+Vec2::new(16.0*dp, 16.0*dp), Vec2::new(iw+20.0, scr.height()-tbh-32.0*dp)),
                |ui| {
                    for i in &self.icons {
                        if ui.add_sized([iw, ih], egui::Button::new(
                            RichText::new(format!("📄 {}", i.name)).size(fs_md).color(Color32::WHITE)
                        ).fill(Color32::TRANSPARENT)).clicked() { launch(&i.path); }
                    }
                });
        });

        // ── Taskbar ──
        let tb = egui::Rect::from_min_size(scr.min+Vec2::new(0.0, scr.height()-tbh), Vec2::new(scr.width(), tbh));
        let style = dark_style();
        egui::Area::new(Id::new("tb")).fixed_pos(tb.min).order(egui::Order::Foreground).show(ctx, |ui| {
            ui.allocate_ui_at_rect(tb, |ui| {
                ui.painter().rect_filled(tb, 0.0, Color32::from_rgb(24,24,28));
                ui.set_style(style);
                ui.horizontal(|ui| {
                    if ui.add_sized([tbw, tbh], egui::Button::new(
                        RichText::new("\u{229E}").size(fs_lg).color(Color32::WHITE)
                    ).fill(Color32::TRANSPARENT)).clicked() { self.start_open = !self.start_open; }

                    ui.add_space(4.0*dp);
                    for i in &self.icons {
                        let label = &i.name[..i.name.len().min(3)];
                        if ui.add_sized([taw, tbh], egui::Button::new(
                            RichText::new(label).size(fs_sm).color(Color32::WHITE)
                        ).fill(Color32::TRANSPARENT)).clicked() { launch(&i.path); }
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(RichText::new(&self.clock).color(Color32::WHITE).size(fs_xs));
                        if ui.add_sized([tbw, tbh], egui::Button::new(
                            RichText::new("\u{23FB}").size(fs_lg).color(Color32::RED)
                        ).fill(Color32::TRANSPARENT)).clicked() { launch("wpeutil.exe shutdown"); }
                    });
                });
            });
        });

        // ── Start menu ──
        if self.start_open {
            let mh = (280.0*dp).round();
            let mw = (240.0*dp).round();
            let mp = egui::pos2(scr.min.x, scr.min.y+scr.height()-tbh-mh);
            egui::Area::new(Id::new("sm")).fixed_pos(mp).order(egui::Order::Foreground).show(ctx, |ui| {
                ui.set_style(dark_style());
                ui.allocate_ui_at_rect(egui::Rect::from_min_size(mp, Vec2::new(mw, mh)), |uj| {
                    uj.painter().rect_filled(egui::Rect::from_min_size(mp, Vec2::new(mw, mh)), 8.0*dp,
                        Color32::from_rgba_unmultiplied(32,32,36,250));
                    uj.add_space(8.0*dp);
                    uj.heading(RichText::new("  QAQ-PE").color(Color32::CYAN).size(fs_lg));
                    uj.separator();
                    let scroll_h = (mh*0.6).round();
                    egui::ScrollArea::vertical().max_height(scroll_h).show(uj, |ui2| {
                        for icon in &self.icons {
                            if ui2.button(RichText::new(format!("  {}", icon.name)).color(Color32::WHITE).size(fs_md)).clicked() {
                                launch(&icon.path); self.start_open = false;
                            }
                        }
                    });
                    uj.separator();
                    uj.horizontal(|ui3| {
                        if ui3.button(RichText::new("  重启  ").color(Color32::WHITE).size(fs_md)).clicked() { launch("wpeutil.exe reboot"); }
                        if ui3.button(RichText::new("  关机  ").color(Color32::RED).size(fs_md)).clicked() { launch("wpeutil.exe shutdown"); }
                    });
                });
            });

            if ctx.input(|i| i.pointer.button_clicked(egui::PointerButton::Primary)) {
                if let Some(p) = ctx.input(|i| i.pointer.hover_pos()) {
                    if p.y < scr.height()-tbh-mh { self.start_open = false; }
                }
            }
        }
    }
}

fn font_lg(dp: f32) -> f32 { (18.0*dp).round() }
fn font_md(dp: f32) -> f32 { (13.0*dp).round() }
fn font_sm(dp: f32) -> f32 { (10.0*dp).round() }
fn font_xs(dp: f32) -> f32 { (12.0*dp).round() }
