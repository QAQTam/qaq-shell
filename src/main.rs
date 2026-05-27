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

struct ShellApp { clock: String, taskbar_h: f32, start_open: bool, icons: Vec<DesktopIcon> }
#[derive(Clone)] struct DesktopIcon { name: String, path: String }

impl Default for ShellApp {
    fn default() -> Self {
        Self { clock: String::new(), taskbar_h: 42.0, start_open: false, icons: icons() }
    }
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
    let (exe, args) = if let Some(pos) = app.find(' ') {
        (&app[..pos], &app[pos+1..])
    } else {
        (app, "")
    };
    let w_exe: Vec<u16> = std::ffi::OsStr::new(exe).encode_wide().chain(Some(0)).collect();
    let w_args: Vec<u16> = if args.is_empty() { vec![0] }
        else { std::ffi::OsStr::new(args).encode_wide().chain(Some(0)).collect() };
    let w_op: Vec<u16> = std::ffi::OsStr::new("open").encode_wide().chain(Some(0)).collect();
    unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            w_op.as_ptr(),
            w_exe.as_ptr(),
            if args.is_empty() { std::ptr::null() } else { w_args.as_ptr() },
            std::ptr::null(),
            5,
        );
    }
}

#[link(name = "shell32")]
extern "system" {
    fn ShellExecuteW(
        hwnd: *mut std::ffi::c_void,
        operation: *const u16,
        file: *const u16,
        parameters: *const u16,
        directory: *const u16,
        show_cmd: i32,
    ) -> isize;
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
        ctx.set_visuals(egui::Visuals::dark());
        ctx.request_repaint_after(std::time::Duration::from_secs(1));

        let desk = egui::Rect::from_min_size(scr.min, Vec2::new(scr.width(), scr.height()-self.taskbar_h));
        egui::Area::new(Id::new("desk")).fixed_pos(desk.min).show(ctx, |ui| {
            ui.allocate_ui_at_rect(desk, |ui| { ui.painter().rect_filled(desk, 0.0, Color32::from_rgb(10,40,80)); });
        });

        egui::Area::new(Id::new("icons")).fixed_pos(scr.min).show(ctx, |ui| {
            ui.allocate_ui_at_rect(
                egui::Rect::from_min_size(scr.min+Vec2::new(16.0,16.0), Vec2::new(200.0, scr.height()-self.taskbar_h-32.0)),
                |ui| {
                    for i in &self.icons {
                        if ui.add_sized([180.0,40.0], egui::Button::new(
                            RichText::new(format!(" {} {}", '\u{1F4C4}', i.name)).size(13.0).color(Color32::WHITE)
                        ).fill(Color32::TRANSPARENT)).clicked() { launch(&i.path); }
                    }
                });
        });

        let tb = egui::Rect::from_min_size(scr.min+Vec2::new(0.0, scr.height()-self.taskbar_h), Vec2::new(scr.width(), self.taskbar_h));
        let style = dark_style();
        egui::Area::new(Id::new("tb")).fixed_pos(tb.min).order(egui::Order::Foreground).show(ctx, |ui| {
            ui.allocate_ui_at_rect(tb, |ui| {
                ui.painter().rect_filled(tb, 0.0, Color32::from_rgb(24,24,28));
                ui.set_style(style);
                ui.horizontal(|ui| {
                    if ui.add_sized([48.0,tb.height()], egui::Button::new(
                        RichText::new("\u{229E}").size(20.0).color(Color32::WHITE)
                    ).fill(Color32::TRANSPARENT)).clicked() { self.start_open = !self.start_open; }

                    ui.add_space(4.0);
                    for i in &self.icons {
                        let label = &i.name[..i.name.len().min(3)];
                        if ui.add_sized([38.0,tb.height()], egui::Button::new(
                            RichText::new(label).size(10.0).color(Color32::WHITE)
                        ).fill(Color32::TRANSPARENT)).clicked() { launch(&i.path); }
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(RichText::new(&self.clock).color(Color32::WHITE).size(12.0));
                        if ui.add_sized([38.0,tb.height()], egui::Button::new(
                            RichText::new("\u{23FB}").size(16.0).color(Color32::RED)
                        ).fill(Color32::TRANSPARENT)).clicked() { launch("wpeutil.exe shutdown"); }
                    });
                });
            });
        });

        if self.start_open {
            let mh = 280.0;
            let mp = egui::pos2(scr.min.x, scr.min.y+scr.height()-self.taskbar_h-mh);
            egui::Area::new(Id::new("sm")).fixed_pos(mp).order(egui::Order::Foreground).show(ctx, |ui| {
                ui.set_style(dark_style());
                ui.allocate_ui_at_rect(egui::Rect::from_min_size(mp, Vec2::new(220.0,mh)), |uj| {
                    uj.painter().rect_filled(egui::Rect::from_min_size(mp, Vec2::new(220.0,mh)), 8.0,
                        Color32::from_rgba_unmultiplied(32,32,36,250));
                    uj.add_space(8.0);
                    uj.heading(RichText::new("  QAQ-PE").color(Color32::CYAN).size(16.0));
                    uj.separator();
                    egui::ScrollArea::vertical().max_height(180.0).show(uj, |ui2| {
                        for icon in &self.icons {
                            if ui2.button(RichText::new(format!("  {}", icon.name)).color(Color32::WHITE).size(13.0)).clicked() {
                                launch(&icon.path); self.start_open = false;
                            }
                        }
                    });
                    uj.separator();
                    uj.horizontal(|ui3| {
                        if ui3.button(RichText::new("  重启  ").color(Color32::WHITE).size(13.0)).clicked() { launch("wpeutil.exe reboot"); }
                        if ui3.button(RichText::new("  关机  ").color(Color32::RED).size(13.0)).clicked() { launch("wpeutil.exe shutdown"); }
                    });
                });
            });

            if ctx.input(|i| i.pointer.button_clicked(egui::PointerButton::Primary)) {
                if let Some(p) = ctx.input(|i| i.pointer.hover_pos()) {
                    if p.y < scr.height()-self.taskbar_h-mh { self.start_open = false; }
                }
            }
        }
    }
}
