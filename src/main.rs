#![windows_subsystem = "windows"]

use egui::{Align, Align2, Color32, ColorImage, Context, FontData, FontDefinitions, FontFamily, FontId, Id, Layout, RichText, Sense, TextureHandle, Vec2};
use std::os::windows::ffi::OsStrExt;
use std::sync::Arc;
use windows_sys::Win32::System::Time::{GetTimeZoneInformation, TIME_ZONE_INFORMATION};
use windows_sys::Win32::Graphics::Gdi::{
    CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, GetObjectW, SelectObject, BITMAP,
    BITMAPINFOHEADER,
};
use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DestroyIcon, EnumWindows, GetIconInfo, GetMessageW, GetWindow, GetWindowLongW,
    GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindowVisible, PostMessageW, ShowWindow,
    ICONINFO, MSG, SW_MINIMIZE, SW_RESTORE, SW_SHOW, WM_CLOSE,
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
        if r == 2 { tz.Bias + tz.DaylightBias } else { tz.Bias }
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
    icon: Option<TextureHandle>,
}

#[derive(Clone)]
struct WindowInfo {
    hwnd: isize,
    title: String,
}

struct Scale {
    dpi: f32,
    th: f32,
    bw: f32,
    iw: f32,
    is: f32,
    fl: f32,
    fm: f32,
    fs: f32,
    fd: f32,
    fx: f32,
    ww: f32,
}

impl Scale {
    fn new(s: f32) -> Self {
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

struct ShellApp {
    clock: String,
    start_open: bool,
    icons: Vec<DesktopIcon>,
    windows: Vec<WindowInfo>,
    last_window_refresh: std::time::Instant,
    tz_bias: i32,
    active_window: isize,
    icons_loaded: bool,
    context_menu_hwnd: isize,
    context_menu_rect: Option<egui::Rect>,
    style: Arc<egui::Style>,
    win_event_rx: std::sync::mpsc::Receiver<WinEvent>,
    force_refresh: bool,
    desktop_ctx_pos: Option<egui::Pos2>,
    shutdown_confirm: bool,
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
            icons_loaded: false,
            context_menu_hwnd: 0,
            context_menu_rect: None,
            style: {
                let mut v = egui::Visuals::dark();
                v.widgets.inactive.bg_fill = Color32::from_rgba_unmultiplied(255, 255, 255, 8);
                v.widgets.hovered.bg_fill = Color32::from_rgba_unmultiplied(255, 255, 255, 25);
                v.widgets.active.bg_fill = Color32::from_rgba_unmultiplied(255, 255, 255, 40);
                Arc::new(egui::Style { visuals: v, ..Default::default() })
            },
            win_event_rx: spawn_winevent_thread(),
            force_refresh: false,
            desktop_ctx_pos: None,
            shutdown_confirm: false,
        }
    }
}

fn load_apps_from_toml() -> Option<Vec<DesktopIcon>> {
    let path = {
        let mut p = std::env::current_exe().ok()?;
        p.set_file_name("qaq-shell.toml");
        p
    };
    let content = std::fs::read_to_string(&path).ok()?;
    let mut icons = Vec::new();
    let mut name = String::new();
    let mut exec = String::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line == "[[apps]]" {
            if !name.is_empty() && !exec.is_empty() {
                icons.push(DesktopIcon { name: std::mem::take(&mut name), path: std::mem::take(&mut exec), icon: None });
            }
            continue;
        }
        if let Some(v) = line.strip_prefix("name = \"").and_then(|s| s.strip_suffix('"')) { name = v.to_string(); }
        if let Some(v) = line.strip_prefix("exec = \"").and_then(|s| s.strip_suffix('"')) { exec = expand_env(v); }
    }
    if !name.is_empty() && !exec.is_empty() {
        icons.push(DesktopIcon { name, path: exec, icon: None });
    }
    if icons.is_empty() { None } else { Some(icons) }
}

fn expand_env(path: &str) -> String {
    let mut r = path.to_string();
    for var in &["SYSTEMDRIVE", "SYSTEMROOT", "WINDIR"] {
        if let Ok(v) = std::env::var(var) { r = r.replace(&format!("%{}%", var), &v); }
    }
    r
}

fn icons() -> Vec<DesktopIcon> {
    if let Some(apps) = load_apps_from_toml() { return apps; }

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
                        name: p.file_stem().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
                        path: p.to_string_lossy().to_string(),
                        icon: None,
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
            v.push(DesktopIcon { name: n.to_string(), path: p.to_string(), icon: None });
        }
    }
    v
}

fn launch(app: &str) {
    let (exe, args) = if let Some((head, tail)) = app.split_once(' ') {
        if head.contains('\\') || head.contains(':') { (app, "") } else { (head, tail) }
    } else { (app, "") };
    let we: Vec<u16> = std::ffi::OsStr::new(exe).encode_wide().chain(Some(0)).collect();
    let wo: Vec<u16> = std::ffi::OsStr::new("open").encode_wide().chain(Some(0)).collect();
    let wa: Vec<u16> = if args.is_empty() {
        vec![0]
    } else {
        std::ffi::OsStr::new(args).encode_wide().chain(Some(0)).collect()
    };
    let wa_ptr = if args.is_empty() { std::ptr::null() } else { wa.as_ptr() };
    unsafe { ShellExecuteW(std::ptr::null_mut(), wo.as_ptr(), we.as_ptr(), wa_ptr, std::ptr::null(), 5); }
}

#[derive(Debug)]
struct WinEvent {
    event: u32,
    hwnd: isize,
}

#[link(name = "user32")]
extern "system" {
    fn SetWinEventHook(
        eventMin: u32, eventMax: u32,
        hmodWinEventProc: *mut std::ffi::c_void,
        lpfnWinEventProc: Option<unsafe extern "system" fn(*mut std::ffi::c_void, u32, *mut std::ffi::c_void, i32, i32, u32, u32)>,
        idProcess: u32, idThread: u32, dwFlags: u32,
    ) -> *mut std::ffi::c_void;
}

static WIN_EVENT_TX: std::sync::OnceLock<std::sync::mpsc::Sender<WinEvent>> = std::sync::OnceLock::new();

unsafe extern "system" fn winevent_cb(
    _hook: *mut std::ffi::c_void, event: u32, hwnd: *mut std::ffi::c_void,
    _id_obj: i32, _id_child: i32, _thread: u32, _time: u32,
) {
    if let Some(tx) = WIN_EVENT_TX.get() {
        let _ = tx.send(WinEvent { event, hwnd: hwnd as isize });
    }
}

fn spawn_winevent_thread() -> std::sync::mpsc::Receiver<WinEvent> {
    let (tx, rx) = std::sync::mpsc::channel::<WinEvent>();
    std::thread::spawn(move || {
        WIN_EVENT_TX.set(tx).ok();
        unsafe {
            SetWinEventHook(
                0x0003, 0x0003, // EVENT_SYSTEM_FOREGROUND
                std::ptr::null_mut(), Some(winevent_cb), 0, 0,
                0x0004, // WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS
            );
            SetWinEventHook(
                0x8000, 0x8001, // EVENT_OBJECT_CREATE .. EVENT_OBJECT_DESTROY
                std::ptr::null_mut(), Some(winevent_cb), 0, 0,
                0x0004,
            );
            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {}
        }
    });
    rx
}

fn extract_icon(path: &str) -> Option<ColorImage> {
    struct OwnedIcon(*mut std::ffi::c_void);
    struct OwnedBitmap(*mut std::ffi::c_void);
    struct OwnedDC(*mut std::ffi::c_void);
    struct SelectGuard {
        hdc: *mut std::ffi::c_void,
        prev: *mut std::ffi::c_void,
    }
    impl Drop for OwnedIcon { fn drop(&mut self) { unsafe { DestroyIcon(self.0); } } }
    impl Drop for OwnedBitmap { fn drop(&mut self) { unsafe { DeleteObject(self.0); } } }
    impl Drop for OwnedDC { fn drop(&mut self) { unsafe { DeleteDC(self.0); } } }
    impl Drop for SelectGuard {
        fn drop(&mut self) { unsafe { SelectObject(self.hdc, self.prev); } }
    }
    impl SelectGuard {
        unsafe fn new(hdc: *mut std::ffi::c_void, obj: *mut std::ffi::c_void) -> Self {
            let prev = SelectObject(hdc, obj);
            SelectGuard { hdc, prev }
        }
    }

    #[repr(C)]
    #[allow(non_snake_case)]
    struct SHFILEINFOW { hIcon: isize, iIcon: i32, dwAttributes: u32, szDisplayName: [u16; 260], szTypeName: [u16; 80] }
    #[link(name = "shell32")]
    extern "system" {
        fn SHGetFileInfoW(pszPath: *const u16, dwFileAttributes: u32, psfi: *mut SHFILEINFOW, cbFileInfo: u32, uFlags: u32) -> usize;
    }
    unsafe {
        let pw: Vec<u16> = std::ffi::OsStr::new(path).encode_wide().chain(Some(0)).collect();
        let mut sfi: SHFILEINFOW = std::mem::zeroed();
        if SHGetFileInfoW(pw.as_ptr(), 0, &mut sfi, std::mem::size_of::<SHFILEINFOW>() as u32, 0x100) == 0 || sfi.hIcon == 0 {
            return None;
        }
        let _icon = OwnedIcon(sfi.hIcon as *mut std::ffi::c_void);
        let mut info: ICONINFO = std::mem::zeroed();
        if GetIconInfo(_icon.0, &mut info) == 0 { return None; }
        if info.hbmColor.is_null() { return None; }
        let _hbm_mask = OwnedBitmap(info.hbmMask);
        let _hbm_color = OwnedBitmap(info.hbmColor);
        let mut bmp: BITMAP = std::mem::zeroed();
        GetObjectW(_hbm_color.0, std::mem::size_of::<BITMAP>() as i32, &mut bmp as *mut BITMAP as *mut std::ffi::c_void);
        let w = bmp.bmWidth as usize;
        let h = bmp.bmHeight as usize;
        if w == 0 || h == 0 { return None; }
        let _hdc = OwnedDC(CreateCompatibleDC(std::ptr::null_mut()));
        if _hdc.0.is_null() { return None; }
        let _sel = SelectGuard::new(_hdc.0, _hbm_color.0);
        let mut bi = std::mem::zeroed::<BITMAPINFOHEADER>();
        bi.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bi.biWidth = bmp.bmWidth;
        bi.biHeight = -bmp.bmHeight;
        bi.biPlanes = 1;
        bi.biBitCount = 32;
        let mut pixels: Vec<u8> = vec![0u8; w * h * 4];
        GetDIBits(_hdc.0, _hbm_color.0, 0, bmp.bmHeight as u32, pixels.as_mut_ptr() as *mut std::ffi::c_void, &mut bi as *mut BITMAPINFOHEADER as _, 0);
        for chunk in pixels.chunks_exact_mut(4) { let b = chunk[0]; chunk[0] = chunk[2]; chunk[2] = b; }
        Some(ColorImage::from_rgba_unmultiplied([w, h], &pixels))
    }
}

fn enumerate_windows() -> Vec<WindowInfo> {
    struct Ctx { windows: Vec<WindowInfo>, our_pid: u32 }
    extern "system" fn callback(hwnd: *mut std::ffi::c_void, lparam: isize) -> i32 {
        unsafe {
            let ctx = &mut *(lparam as *mut Ctx);
            if IsWindowVisible(hwnd) == 0 { return 1; }
            let ex = GetWindowLongW(hwnd, -20) as u32;
            if ex & 0x80 != 0 { return 1; }
            if (ex & 0x08000000) != 0 { return 1; }
            let style = GetWindowLongW(hwnd, -16) as u32;
            if (style & 0x40000000) != 0 { return 1; }
            let owner = GetWindow(hwnd, 4);
            if !owner.is_null() && ex & 0x40000 == 0 { return 1; }
            let mut pid: u32 = 0;
            GetWindowThreadProcessId(hwnd, &mut pid);
            if pid == ctx.our_pid { return 1; }
            let mut text: [u16; 512] = [0; 512];
            let len = GetWindowTextW(hwnd, text.as_mut_ptr(), 512);
            if len == 0 { return 1; }
            ctx.windows.push(WindowInfo { hwnd: hwnd as isize, title: String::from_utf16_lossy(&text[..len as usize]) });
            1
        }
    }
    let mut ctx = Ctx { windows: Vec::new(), our_pid: std::process::id() };
    unsafe { EnumWindows(Some(callback), &mut ctx as *mut Ctx as isize); }
    ctx.windows
}

fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars { s.to_string() } else { format!("{}...", s.chars().take(max_chars).collect::<String>()) }
}

fn wrap_text(name: &str, max_chars: usize) -> Vec<String> {
    let mut lines: Vec<String> = vec![String::new()];
    for word in name.split_whitespace() {
        let cur = lines.last_mut().unwrap();
        let wc = word.chars().count();
        let cc = cur.chars().count();
        let needed = if cur.is_empty() { wc } else { cc + 1 + wc };
        if needed > max_chars && !cur.is_empty() { lines.push(word.to_string()); } else { if !cur.is_empty() { cur.push(' '); } cur.push_str(word); }
        if lines.len() >= 2 && lines.last().unwrap().chars().count() >= max_chars { break; }
    }
    if lines.len() > 2 { lines.truncate(2); }
    lines
}

fn merge_windows(old: &[WindowInfo], new: &[WindowInfo]) -> Vec<WindowInfo> {
    let new_map: std::collections::HashMap<isize, &WindowInfo> = new.iter().map(|w| (w.hwnd, w)).collect();
    let mut result: Vec<WindowInfo> = old.iter().filter_map(|w| new_map.get(&w.hwnd).map(|nw| WindowInfo { hwnd: w.hwnd, title: nw.title.clone() })).collect();
    let existing: std::collections::HashSet<isize> = result.iter().map(|w| w.hwnd).collect();
    for w in new { if !existing.contains(&w.hwnd) { result.push(w.clone()); } }
    result
}

fn focus_window(hwnd: isize) {
    #[link(name = "user32")]
    extern "system" { fn SwitchToThisWindow(hwnd: *mut std::ffi::c_void, fAltTab: i32); }
    unsafe {
        let h = hwnd as *mut std::ffi::c_void;
        ShowWindow(h, SW_SHOW);
        if IsIconic(h) != 0 { ShowWindow(h, SW_RESTORE); }
        SwitchToThisWindow(h, 1);
    }
}

impl ShellApp {
    fn render_desktop(&mut self, ctx: &Context, sc: &Scale, scr: egui::Rect) {
        let dk = egui::Rect::from_min_size(scr.min, Vec2::new(scr.width(), scr.height() - sc.th));
        egui::Area::new(Id::new("dk")).fixed_pos(dk.min).show(ctx, |ui| {
            ui.allocate_ui_at_rect(dk, |ui| { ui.painter().rect_filled(dk, 0.0, Color32::from_rgb(10, 40, 80)); });
        });

        let ih_total = sc.is + sc.fd * 1.4 * 2.0 + 10.0 * sc.dpi;
        let cols = ((scr.width() - 16.0 * sc.dpi) / sc.iw).floor() as usize;
        let cols = cols.max(1);
        egui::Area::new(Id::new("ic")).fixed_pos(scr.min).show(ctx, |ui| {
            for (idx, i) in self.icons.iter().enumerate() {
                let col = (idx % cols) as f32;
                let row = (idx / cols) as f32;
                let x = 16.0 * sc.dpi + col * sc.iw;
                let y = 16.0 * sc.dpi + row * ih_total;
                if y + ih_total > scr.height() - sc.th { break; }
                let rect = egui::Rect::from_min_size(egui::pos2(x, y), Vec2::new(sc.iw, ih_total));
                let response = ui.interact(rect, Id::new(format!("icon_{idx}")), Sense::click());
                if response.hovered() {
                    ui.painter().rect_filled(rect, 4.0, Color32::from_rgba_unmultiplied(255, 255, 255, 20));
                }
                if let Some(ref icon) = i.icon {
                    ui.painter().image(icon.id(), egui::Rect::from_center_size(
                        egui::pos2(rect.center().x, rect.min.y + sc.is / 2.0 + 4.0 * sc.dpi), Vec2::new(sc.is, sc.is),
                    ), egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
                } else {
                    ui.painter().text(egui::pos2(rect.center().x, rect.min.y + sc.is / 2.0 + 4.0 * sc.dpi), Align2::CENTER_CENTER, "📄", FontId::proportional(sc.is * 0.7), Color32::WHITE);
                }
                let text_lines = wrap_text(&i.name, 14);
                let text_y = rect.min.y + sc.is + 4.0 * sc.dpi;
                let line_h = sc.fd * 1.4;
                for (li, line) in text_lines.iter().enumerate() {
                    ui.painter().text(egui::pos2(rect.center().x, text_y + li as f32 * line_h), Align2::CENTER_TOP, line, FontId::proportional(sc.fd), Color32::WHITE);
                }
                if response.clicked() { launch(&i.path); self.force_refresh = true; }
            }
        });

        if ctx.input(|i| i.pointer.button_clicked(egui::PointerButton::Secondary)) {
            if let Some(pos) = ctx.input(|i| i.pointer.hover_pos()) {
                let on_taskbar = pos.y >= scr.height() - sc.th;
                let on_icon = (0..self.icons.len()).any(|idx| {
                    let col = (idx % cols) as f32;
                    let row = (idx / cols) as f32;
                    let x = 16.0 * sc.dpi + col * sc.iw;
                    let y = 16.0 * sc.dpi + row * ih_total;
                    if y + ih_total > scr.height() - sc.th { return false; }
                    egui::Rect::from_min_size(egui::pos2(x, y), Vec2::new(sc.iw, ih_total)).contains(pos)
                });
                if !on_taskbar && !on_icon {
                    self.desktop_ctx_pos = Some(pos);
                    self.context_menu_hwnd = 0;
                }
            }
        }

        if let Some(click_pos) = self.desktop_ctx_pos {
            let s = sc.dpi;
            let cm_w = 152.0 * s;
            let item_h = 28.0 * s;
            let pad = 4.0 * s;
            let cm_h = item_h * 3.0 + pad * 2.0 + 1.0;
            let cm_x = click_pos.x.min(scr.max.x - cm_w);
            let cm_y = click_pos.y.min(scr.max.y - sc.th - cm_h);
            let mr = egui::Rect::from_min_size(egui::pos2(cm_x, cm_y), Vec2::new(cm_w, cm_h));
            let mut close_menu = false;
            egui::Area::new(Id::new("dsk_ctx"))
                .order(egui::Order::Foreground)
                .fixed_pos(mr.min)
                .show(ctx, |ui| {
                    ui.set_style(self.style.clone());
                    ui.allocate_ui_at_rect(mr, |ui| {
                        ui.painter().rect_filled(mr, 4.0 * s, Color32::from_rgba_unmultiplied(36, 36, 44, 248));
                        ui.add_space(pad);
                        if ui.add_sized([cm_w, item_h], egui::Button::new(
                            RichText::new("    刷新图标").size(sc.fs).color(Color32::WHITE),
                        ).fill(Color32::TRANSPARENT)).clicked() {
                            self.icons = icons();
                            self.icons_loaded = false;
                            close_menu = true;
                        }
                        let sep_y = mr.min.y + pad + item_h;
                        ui.painter().line_segment(
                            [egui::pos2(mr.min.x + 8.0 * s, sep_y), egui::pos2(mr.max.x - 8.0 * s, sep_y)],
                            egui::Stroke::new(0.5, Color32::from_rgba_unmultiplied(255, 255, 255, 22)),
                        );
                        if ui.add_sized([cm_w, item_h], egui::Button::new(
                            RichText::new("    打开桌面目录").size(sc.fs).color(Color32::WHITE),
                        ).fill(Color32::TRANSPARENT)).clicked() {
                            for p in &["X:\\Users\\Default\\Desktop", "X:\\Users\\Public\\Desktop", "C:\\Users\\Public\\Desktop"] {
                                if std::path::Path::new(p).exists() { launch(p); break; }
                            }
                            close_menu = true;
                        }
                        if ui.add_sized([cm_w, item_h], egui::Button::new(
                            RichText::new("    系统属性").size(sc.fs).color(Color32::WHITE),
                        ).fill(Color32::TRANSPARENT)).clicked() {
                            launch("control.exe sysdm.cpl");
                            close_menu = true;
                        }
                    });
                });
            let clicked_outside = ctx.input(|i| {
                i.pointer.button_clicked(egui::PointerButton::Primary)
                    && i.pointer.hover_pos().map_or(true, |p| !mr.contains(p))
            });
            if close_menu || clicked_outside {
                self.desktop_ctx_pos = None;
            }
        }
    }

    fn render_taskbar(&mut self, ctx: &Context, sc: &Scale, scr: egui::Rect) {
        let s = sc.dpi;
        let tb = egui::Rect::from_min_size(scr.min + Vec2::new(0.0, scr.height() - sc.th), Vec2::new(scr.width(), sc.th));
        egui::Area::new(Id::new("tb")).fixed_pos(tb.min).order(egui::Order::Foreground).show(ctx, |ui| {
            ui.allocate_ui_at_rect(tb, |ui| {
                ui.painter().rect_filled(tb, 0.0, Color32::from_rgb(24, 24, 28));
                ui.painter().line_segment(
                    [tb.min, egui::pos2(tb.max.x, tb.min.y)],
                    egui::Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 16)),
                );
                ui.set_style(self.style.clone());
                ui.horizontal(|ui| {
                    if ui.add_sized([sc.bw, sc.th], egui::Button::new(RichText::new("\u{229E}").size(sc.fl).color(Color32::WHITE)).fill(Color32::TRANSPARENT)).clicked() {
                        self.start_open = !self.start_open;
                    }

                    let sep_color = Color32::from_rgba_unmultiplied(255, 255, 255, 40);
                    let sep_w = 6.0 * s;
                    let sep_h = sc.th - 12.0 * s;
                    let (_, sr) = ui.allocate_space(Vec2::new(sep_w, sc.th));
                    ui.painter().rect_filled(egui::Rect::from_center_size(sr.center(), Vec2::new(1.0, sep_h)), 0.0, sep_color);

                    for w in self.windows.iter().take(7) {
                        let is_active = self.active_window == w.hwnd;
                        let fill = if is_active {
                            Color32::from_rgba_unmultiplied(100, 160, 255, 55)
                        } else {
                            Color32::from_rgba_unmultiplied(255, 255, 255, 18)
                        };
                        let r = ui.add_sized([sc.ww, sc.th], egui::Button::new(
                            RichText::new(truncate(&w.title, 10)).size(sc.fs).color(Color32::WHITE),
                        ).fill(fill));
                        if is_active {
                            let ind = egui::Rect::from_min_size(
                                egui::pos2(r.rect.min.x + 4.0 * s, tb.max.y - 2.5 * s),
                                Vec2::new(r.rect.width() - 8.0 * s, 2.0 * s),
                            );
                            ui.painter().rect_filled(ind, 1.0, Color32::from_rgb(100, 180, 255));
                        }
                        if r.clicked() {
                            if self.active_window == w.hwnd {
                                unsafe { ShowWindow(w.hwnd as *mut std::ffi::c_void, SW_MINIMIZE); }
                                self.active_window = 0;
                            } else {
                                focus_window(w.hwnd);
                                self.active_window = w.hwnd;
                            }
                        }
                        if r.secondary_clicked() {
                            self.context_menu_hwnd = w.hwnd;
                            self.context_menu_rect = Some(r.rect);
                        }
                    }

                    let (_, sr2) = ui.allocate_space(Vec2::new(sep_w, sc.th));
                    ui.painter().rect_filled(egui::Rect::from_center_size(sr2.center(), Vec2::new(1.0, sep_h)), 0.0, sep_color);

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(RichText::new(&self.clock).color(Color32::WHITE).size(sc.fx));
                        if ui.add_sized([sc.bw, sc.th], egui::Button::new(RichText::new("\u{23FB}").size(sc.fl).color(Color32::RED)).fill(Color32::TRANSPARENT)).clicked() {
                            self.shutdown_confirm = true;
                        }
                    });
                });
            });
        });

        if self.context_menu_hwnd != 0 {
            let cm_w = 120.0 * s;
            let cm_h = 28.0 * s;
            let cm_x = if let Some(btn) = self.context_menu_rect {
                (btn.center().x - cm_w / 2.0).clamp(scr.min.x, scr.min.x + scr.width() - cm_w)
            } else {
                scr.min.x
            };
            let cm_y = if let Some(btn) = self.context_menu_rect {
                (btn.min.y - cm_h).max(scr.min.y)
            } else {
                scr.min.y
            };
            let r = egui::Rect::from_min_size(egui::pos2(cm_x, cm_y), Vec2::new(cm_w, cm_h));
            egui::Area::new(Id::new("ctxmenu")).order(egui::Order::Foreground).fixed_pos(r.min).show(ctx, |ui| {
                ui.allocate_ui_at_rect(r, |ui| {
                    ui.set_style(self.style.clone());
                    ui.painter().rect_filled(r, 4.0, Color32::from_rgba_unmultiplied(40, 40, 48, 240));
                    if ui.add_sized([cm_w, cm_h], egui::Button::new(
                        RichText::new("  关闭窗口").size(sc.fs).color(Color32::WHITE),
                    ).fill(Color32::TRANSPARENT)).clicked() {
                        unsafe { PostMessageW(self.context_menu_hwnd as *mut std::ffi::c_void, WM_CLOSE, 0, 0); }
                        self.context_menu_hwnd = 0;
                        self.force_refresh = true;
                    }
                });
            });
            if ctx.input(|i| i.pointer.button_clicked(egui::PointerButton::Primary)) {
                self.context_menu_hwnd = 0;
            }
        }
    }

    fn render_start_menu(&mut self, ctx: &Context, sc: &Scale, scr: egui::Rect) {
        if !self.start_open { return; }
        let s = sc.dpi;
        let mh = (280.0 * s).round();
        let mw = (240.0 * s).round();
        let mp = egui::pos2(scr.min.x, scr.min.y + scr.height() - sc.th - mh);
        egui::Area::new(Id::new("sm")).fixed_pos(mp).order(egui::Order::Foreground).show(ctx, |ui| {
            ui.set_style(self.style.clone());
            ui.allocate_ui_at_rect(egui::Rect::from_min_size(mp, Vec2::new(mw, mh)), |uj| {
                uj.painter().rect_filled(egui::Rect::from_min_size(mp, Vec2::new(mw, mh)), 8.0 * s, Color32::from_rgba_unmultiplied(32, 32, 36, 250));
                uj.add_space(8.0 * s);
                uj.heading(RichText::new("  QAQ-PE").color(Color32::CYAN).size(sc.fl));
                uj.separator();
                egui::ScrollArea::vertical().max_height((mh * 0.6).round()).show(uj, |ui2| {
                    for icon in &self.icons {
                        if ui2.button(RichText::new(format!("  {}", icon.name)).color(Color32::WHITE).size(sc.fm)).clicked() {
                            launch(&icon.path);
                            self.force_refresh = true;
                            self.start_open = false;
                        }
                    }
                });
                uj.separator();
                uj.horizontal(|ui3| {
                    if ui3.button(RichText::new("  重启  ").color(Color32::WHITE).size(sc.fm)).clicked() { launch("wpeutil.exe reboot"); }
                    if ui3.button(RichText::new("  关机  ").color(Color32::RED).size(sc.fm)).clicked() { launch("wpeutil.exe shutdown"); }
                });
            });
        });
        if ctx.input(|i| i.pointer.button_clicked(egui::PointerButton::Primary)) {
            if let Some(p) = ctx.input(|i| i.pointer.hover_pos()) {
                if p.y < scr.height() - sc.th - mh { self.start_open = false; }
            }
        }
    }
}

impl eframe::App for ShellApp {
    fn update(&mut self, ctx: &Context, _: &mut eframe::Frame) {
        self.clock = clock(self.tz_bias);
        let scr = ctx.screen_rect();
        let sc = Scale::new(dpi(ctx));
        ctx.set_visuals(egui::Visuals::dark());
        ctx.request_repaint_after(std::time::Duration::from_secs(1));

        if !self.icons_loaded {
            for i in &mut self.icons {
                if i.icon.is_none() {
                    if let Some(img) = extract_icon(&i.path) {
                        i.icon = Some(ctx.load_texture(&i.path, img, Default::default()));
                    }
                }
            }
            self.icons_loaded = true;
        }

        // WinEvent-driven window tracking + 3s polling fallback
        while let Ok(ev) = self.win_event_rx.try_recv() {
            match ev.event {
                0x8001 | 0x8003 => self.force_refresh = true,
                0x0003 => { self.active_window = ev.hwnd; self.force_refresh = true; }
                _ => {}
            }
        }
        let now = std::time::Instant::now();
        if self.force_refresh || now.duration_since(self.last_window_refresh).as_secs() >= 3 {
            self.windows = merge_windows(&self.windows, &enumerate_windows());
            self.last_window_refresh = now;
            self.force_refresh = false;
            if self.active_window != 0 && !self.windows.iter().any(|w| w.hwnd == self.active_window) {
                self.active_window = 0;
            }
        }

        self.render_desktop(ctx, &sc, scr);
        self.render_taskbar(ctx, &sc, scr);
        self.render_start_menu(ctx, &sc, scr);

        if self.shutdown_confirm {
            let s = sc.dpi;
            let mw = 220.0 * s;
            let mh = 100.0 * s;
            let mp = egui::pos2((scr.width() - mw) / 2.0, (scr.height() - mh) / 2.0);
            egui::Area::new(Id::new("shutdown_confirm"))
                .order(egui::Order::Foreground)
                .fixed_pos(mp)
                .show(ctx, |ui| {
                    let r = egui::Rect::from_min_size(mp, Vec2::new(mw, mh));
                    ui.allocate_ui_at_rect(r, |ui| {
                        ui.set_style(self.style.clone());
                        ui.painter().rect_filled(r, 8.0 * s, Color32::from_rgba_unmultiplied(30, 30, 36, 252));
                        ui.painter().rect_stroke(r, 8.0 * s, egui::Stroke::new(0.5, Color32::from_rgba_unmultiplied(255, 255, 255, 30)), egui::StrokeKind::Inside);
                        ui.add_space(16.0 * s);
                        ui.label(RichText::new("  确认关机？").color(Color32::WHITE).size(sc.fm));
                        ui.add_space(12.0 * s);
                        ui.horizontal(|ui| {
                            ui.add_space(20.0 * s);
                            if ui.button(RichText::new("  关机  ").color(Color32::RED).size(sc.fm)).clicked() {
                                launch("wpeutil.exe shutdown");
                            }
                            ui.add_space(8.0 * s);
                            if ui.button(RichText::new("  取消  ").color(Color32::WHITE).size(sc.fm)).clicked() {
                                self.shutdown_confirm = false;
                            }
                        });
                    });
                });
            if ctx.input(|i| i.pointer.button_clicked(egui::PointerButton::Primary)) {
                self.shutdown_confirm = false;
            }
        }
    }
}
