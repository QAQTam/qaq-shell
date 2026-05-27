#![windows_subsystem = "windows"]

use std::os::windows::ffi::OsStrExt;
use egui::{Align, Color32, Context, Id, Layout, RichText, Vec2, FontDefinitions, FontData, FontFamily};
use std::sync::Arc;

fn main() -> Result<(), eframe::Error> {
    eframe::run_native(
        "QAQ-Shell",
        eframe::NativeOptions { viewport: egui::ViewportBuilder::default().with_fullscreen(true).with_decorations(false), ..Default::default() },
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

fn dpi(ctx: &Context) -> f32 { ctx.pixels_per_point().clamp(0.8, 1.5) }

struct ShellApp { clock: String, start_open: bool, icons: Vec<DesktopIcon> }
#[derive(Clone)] struct DesktopIcon { name: String, path: String }

impl Default for ShellApp {
    fn default() -> Self { Self { clock: String::new(), start_open: false, icons: icons() } }
}

fn icons() -> Vec<DesktopIcon> {
    let mut v = vec![];
    for d in &["X:\\Users\\Default\\Desktop", "X:\\Users\\Public\\Desktop", "C:\\Users\\Public\\Desktop"] {
        if let Ok(e) = std::fs::read_dir(d) {
            for x in e.flatten() {
                let p = x.path();
                if p.extension().map(|e| e == "lnk").unwrap_or(false) {
                    v.push(DesktopIcon { name: p.file_stem().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(), path: p.to_string_lossy().to_string() });
                }
            }
        }
    }
    if v.is_empty() {
        for (n, p) in &[("CMD","cmd.exe"),("DiskGenius","X:\\WinXShell\\DiskGenius.exe"),("WinNTSetup","X:\\Tools\\WinNTSetup\\WinNTSetup_x64.exe"),("servtool","X:\\Tools\\servtool.exe"),("Firefox","X:\\Tools\\Firefox\\firefox.exe"),("7-Zip","X:\\Tools\\7-Zip\\7zFM.exe"),("Notepad3","X:\\Tools\\Notepad3\\Notepad3.exe")] {
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
    let (exe, args) = if let Some(p) = app.find(' ') { (&app[..p], &app[p+1..]) } else { (app, "") };
    let we: Vec<u16> = std::ffi::OsStr::new(exe).encode_wide().chain(Some(0)).collect();
    let wa: Vec<u16> = if args.is_empty() { vec![0] } else { std::ffi::OsStr::new(args).encode_wide().chain(Some(0)).collect() };
    let wo: Vec<u16> = std::ffi::OsStr::new("open").encode_wide().chain(Some(0)).collect();
    unsafe { ShellExecuteW(std::ptr::null_mut(), wo.as_ptr(), we.as_ptr(), if args.is_empty() { std::ptr::null() } else { wa.as_ptr() }, std::ptr::null(), 5); }
}

#[link(name = "shell32")] extern "system" { fn ShellExecuteW(h: *mut std::ffi::c_void, o: *const u16, f: *const u16, p: *const u16, d: *const u16, s: i32) -> isize; }

fn ds() -> Arc<egui::Style> {
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
        let s = dpi(ctx);
        ctx.set_visuals(egui::Visuals::dark());
        ctx.request_repaint_after(std::time::Duration::from_secs(1));

        let th = (42.0*s).round();
        let bw = (48.0*s).round();
        let aw = (40.0*s).round();
        let iw = (172.0*s).round();
        let ih = (38.0*s).round();
        let fl = (18.0*s).round();
        let fm = (13.0*s).round();
        let fs = (10.0*s).round();
        let fx = (12.0*s).round();

        // Desktop
        let dk = egui::Rect::from_min_size(scr.min, Vec2::new(scr.width(), scr.height()-th));
        egui::Area::new(Id::new("dk")).fixed_pos(dk.min).show(ctx, |ui| {
            ui.allocate_ui_at_rect(dk, |ui| { ui.painter().rect_filled(dk, 0.0, Color32::from_rgb(10,40,80)); });
        });

        // Icons
        egui::Area::new(Id::new("ic")).fixed_pos(scr.min).show(ctx, |ui| {
            ui.allocate_ui_at_rect(egui::Rect::from_min_size(scr.min+Vec2::new(16.0*s,16.0*s), Vec2::new(iw+20.0, scr.height()-th-32.0*s)), |ui| {
                for i in &self.icons {
                    if ui.add_sized([iw,ih], egui::Button::new(RichText::new(format!("📄 {}",i.name)).size(fm).color(Color32::WHITE)).fill(Color32::TRANSPARENT)).clicked() { launch(&i.path); }
                }
            });
        });

        // Taskbar
        let tb = egui::Rect::from_min_size(scr.min+Vec2::new(0.0,scr.height()-th), Vec2::new(scr.width(),th));
        egui::Area::new(Id::new("tb")).fixed_pos(tb.min).order(egui::Order::Foreground).show(ctx, |ui| {
            ui.allocate_ui_at_rect(tb, |ui| {
                ui.painter().rect_filled(tb, 0.0, Color32::from_rgb(24,24,28));
                ui.set_style(ds());
                ui.horizontal(|ui| {
                    if ui.add_sized([bw,th], egui::Button::new(RichText::new("\u{229E}").size(fl).color(Color32::WHITE)).fill(Color32::TRANSPARENT)).clicked() { self.start_open = !self.start_open; }
                    ui.add_space(4.0*s);
                    for i in &self.icons {
                        let l = &i.name[..i.name.len().min(3)];
                        if ui.add_sized([aw,th], egui::Button::new(RichText::new(l).size(fs).color(Color32::WHITE)).fill(Color32::TRANSPARENT)).clicked() { launch(&i.path); }
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(RichText::new(&self.clock).color(Color32::WHITE).size(fx));
                        if ui.add_sized([bw,th], egui::Button::new(RichText::new("\u{23FB}").size(fl).color(Color32::RED)).fill(Color32::TRANSPARENT)).clicked() { launch("wpeutil.exe shutdown"); }
                    });
                });
            });
        });

        // Start menu
        if self.start_open {
            let mh = (280.0*s).round(); let mw = (240.0*s).round();
            let mp = egui::pos2(scr.min.x, scr.min.y+scr.height()-th-mh);
            egui::Area::new(Id::new("sm")).fixed_pos(mp).order(egui::Order::Foreground).show(ctx, |ui| {
                ui.set_style(ds());
                ui.allocate_ui_at_rect(egui::Rect::from_min_size(mp, Vec2::new(mw,mh)), |uj| {
                    uj.painter().rect_filled(egui::Rect::from_min_size(mp, Vec2::new(mw,mh)), 8.0*s, Color32::from_rgba_unmultiplied(32,32,36,250));
                    uj.add_space(8.0*s); uj.heading(RichText::new("  QAQ-PE").color(Color32::CYAN).size(fl)); uj.separator();
                    egui::ScrollArea::vertical().max_height((mh*0.6).round()).show(uj, |ui2| {
                        for icon in &self.icons {
                            if ui2.button(RichText::new(format!("  {}",icon.name)).color(Color32::WHITE).size(fm)).clicked() { launch(&icon.path); self.start_open = false; }
                        }
                    });
                    uj.separator();
                    uj.horizontal(|ui3| {
                        if ui3.button(RichText::new("  重启  ").color(Color32::WHITE).size(fm)).clicked() { launch("wpeutil.exe reboot"); }
                        if ui3.button(RichText::new("  关机  ").color(Color32::RED).size(fm)).clicked() { launch("wpeutil.exe shutdown"); }
                    });
                });
            });
            if ctx.input(|i| i.pointer.button_clicked(egui::PointerButton::Primary)) {
                if let Some(p) = ctx.input(|i| i.pointer.hover_pos()) { if p.y < scr.height()-th-mh { self.start_open = false; } }
            }
        }
    }
}
