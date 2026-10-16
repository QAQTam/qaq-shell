//! iced-shell — qaq-shell 的 iced (tiny-skia 软渲染) 迁移版
//! WinPE/WinRE 桌面壳: 桌面图标 + 任务栏(开始菜单/窗口按钮/时钟/关机)
//! 零 GPU/GL 依赖, WinRE 实测可运行 (见 restore-win11/iced-probe)
#![windows_subsystem = "windows"]
#![allow(non_snake_case)]

use iced::widget::{button, column, container, image, mouse_area, row, stack, text};
use iced::{alignment, Center, Element, Fill, Task, Theme};
use std::os::windows::ffi::OsStrExt;

use windows_sys::Win32::System::Time::{GetTimeZoneInformation, TIME_ZONE_INFORMATION};
use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DestroyIcon, EnumWindows, GetIconInfo, GetWindow, GetWindowLongW, GetWindowTextW,
    GetWindowThreadProcessId, IsIconic, IsWindowVisible, SetForegroundWindow, ShowWindow,
    SW_MINIMIZE, SW_RESTORE,
};

// ---------------- Win32 平移区 (原样来自 egui 版) ----------------

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
    format!("{:02}:{:02}:{:02}", (s / 3600) % 24, (s / 60) % 60, s % 60)
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain([0]).collect()
}

fn launch(app: &str) {
    let (exe, args) = if let Some((head, tail)) = app.split_once(' ') {
        if head.contains('\\') || head.contains(':') { (app, "") } else { (head, tail) }
    } else { (app, "") };
    let we = wide(exe);
    let wo = wide("open");
    let (wa_ptr, _keep) = if args.is_empty() {
        (std::ptr::null(), Vec::<u16>::new())
    } else {
        let w = wide(args);
        (w.as_ptr(), w)
    };
    unsafe { ShellExecuteW(std::ptr::null_mut(), wo.as_ptr(), we.as_ptr(), wa_ptr, std::ptr::null(), 5); }
}

#[derive(Clone)]
pub struct WindowInfo {
    hwnd: isize,
    title: String,
}

fn enumerate_windows() -> Vec<WindowInfo> {
    struct Ctx { windows: Vec<WindowInfo>, our_pid: u32 }
    extern "system" fn callback(hwnd: *mut std::ffi::c_void, lparam: isize) -> i32 {
        unsafe {
            let ctx = &mut *(lparam as *mut Ctx);
            if IsWindowVisible(hwnd) == 0 { return 1; }
            let ex = GetWindowLongW(hwnd, -20) as u32;
            if ex & 0x80 != 0 { return 1; }               // tool window
            if ex & 0x0800_0000 != 0 { return 1; }        // no activate
            let style = GetWindowLongW(hwnd, -16) as u32;
            if style & 0x4000_0000 != 0 { return 1; }     // child
            let owner = GetWindow(hwnd, 4);
            if !owner.is_null() && ex & 0x4_0000 == 0 { return 1; }
            let mut pid: u32 = 0;
            GetWindowThreadProcessId(hwnd, &mut pid);
            if pid == ctx.our_pid { return 1; }
            let mut t = [0u16; 512];
            let len = GetWindowTextW(hwnd, t.as_mut_ptr(), 512);
            if len == 0 { return 1; }
            ctx.windows.push(WindowInfo {
                hwnd: hwnd as isize,
                title: String::from_utf16_lossy(&t[..len as usize]),
            });
            1
        }
    }
    let mut ctx = Ctx { windows: Vec::new(), our_pid: std::process::id() };
    unsafe { EnumWindows(Some(callback), &mut ctx as *mut Ctx as isize); }
    ctx.windows
}

fn focus_window(hwnd: isize) {
    unsafe {
        let h = hwnd as *mut std::ffi::c_void;
        if IsIconic(h) != 0 { ShowWindow(h, SW_RESTORE); }
        SetForegroundWindow(h);
    }
}

#[link(name = "user32")]
extern "system" {
    fn ExitWindowsEx(uFlags: u32, dwReason: u32) -> i32;
}
const EWX_POWEROFF: u32 = 0x8;
const EWX_FORCE: u32 = 0x4;

// ---------------- 图标提取 (SHGetFileInfo → RGBA) ----------------

fn extract_icon_rgba(path: &str) -> Option<(usize, usize, Vec<u8>)> {
    #[repr(C)]
    #[allow(non_snake_case)]
    struct SHFILEINFOW { hIcon: isize, iIcon: i32, dwAttributes: u32, szDisplayName: [u16; 260], szTypeName: [u16; 80] }
    #[link(name = "shell32")]
    extern "system" {
        fn SHGetFileInfoW(pszPath: *const u16, dwFileAttributes: u32, psfi: *mut SHFILEINFOW, cbFileInfo: u32, uFlags: u32) -> usize;
    }
    #[link(name = "user32")]
    extern "system" { fn DestroyIcon(h: *mut std::ffi::c_void) -> i32; }
    #[link(name = "gdi32")]
    extern "system" {
        fn GetIconInfo(hicon: *mut std::ffi::c_void, piconinfo: *mut ICONINFO) -> i32;
        fn GetObjectW(h: *mut std::ffi::c_void, c: i32, pv: *mut std::ffi::c_void) -> i32;
        fn GetDIBits(hdc: *mut std::ffi::c_void, hbmp: *mut std::ffi::c_void, u1: u32, u2: u32, pv: *mut std::ffi::c_void, pbmi: *mut BITMAPINFOHEADER, u3: u32) -> i32;
        fn DeleteObject(h: *mut std::ffi::c_void) -> i32;
        fn CreateCompatibleDC(hdc: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
        fn SelectObject(hdc: *mut std::ffi::c_void, h: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
        fn DeleteDC(hdc: *mut std::ffi::c_void) -> i32;
    }
    #[repr(C)]
    #[allow(non_snake_case)]
    struct ICONINFO { fIcon: i32, xHotspot: u32, yHotspot: u32, hbmMask: *mut std::ffi::c_void, hbmColor: *mut std::ffi::c_void }
    #[repr(C)]
    #[allow(non_snake_case)]
    struct BITMAP { bmType: i32, bmWidth: i32, bmHeight: i32, bmWidthBytes: i32, bmPlanes: u16, bmBitsPixel: u16, bmBits: *mut std::ffi::c_void }
    #[repr(C)]
    #[allow(non_snake_case)]
    struct BITMAPINFOHEADER { biSize: u32, biWidth: i32, biHeight: i32, biPlanes: u16, biBitCount: u16, biCompression: u32, biSizeImage: u32, biXPelsPerMeter: i32, biYPelsPerMeter: i32, biClrUsed: u32, biClrImportant: u32 }

    unsafe {
        let pw = wide(path);
        let mut sfi: SHFILEINFOW = std::mem::zeroed();
        if SHGetFileInfoW(pw.as_ptr(), 0, &mut sfi, std::mem::size_of::<SHFILEINFOW>() as u32, 0x100) == 0 || sfi.hIcon == 0 {
            return None;
        }
        let mut info: ICONINFO = std::mem::zeroed();
        if GetIconInfo(sfi.hIcon as *mut _, &mut info) == 0 || info.hbmColor.is_null() {
            DestroyIcon(sfi.hIcon as *mut _);
            return None;
        }
        let mut bmp: BITMAP = std::mem::zeroed();
        GetObjectW(info.hbmColor, std::mem::size_of::<BITMAP>() as i32, &mut bmp as *mut _ as *mut _);
        let (w, h) = (bmp.bmWidth as usize, bmp.bmHeight as usize);
        let hdc = CreateCompatibleDC(std::ptr::null_mut());
        let prev = SelectObject(hdc, info.hbmColor);
        let mut bi: BITMAPINFOHEADER = std::mem::zeroed();
        bi.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bi.biWidth = bmp.bmWidth; bi.biHeight = -bmp.bmHeight;
        bi.biPlanes = 1; bi.biBitCount = 32;
        let mut px = vec![0u8; w * h * 4];
        GetDIBits(hdc, info.hbmColor, 0, bmp.bmHeight as u32, px.as_mut_ptr() as *mut _, &mut bi, 0);
        SelectObject(hdc, prev);
        DeleteDC(hdc);
        DeleteObject(info.hbmColor); DeleteObject(info.hbmMask);
        DestroyIcon(sfi.hIcon as *mut _);
        for c in px.chunks_exact_mut(4) { c.swap(0, 2); } // BGRA→RGBA
        Some((w, h, px))
    }
}

// ---------------- 应用状态 ----------------

#[derive(Clone)]
struct DesktopIcon {
    name: String,
    path: String,
    icon_handle: Option<iced::widget::image::Handle>,
}

fn load_apps_from_toml() -> Option<Vec<(String, String)>> {
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
                icons.push((std::mem::take(&mut name), std::mem::take(&mut exec)));
            }
            continue;
        }
        if let Some(v) = line.strip_prefix("name = \"").and_then(|s| s.strip_suffix('"')) { name = v.to_string(); }
        if let Some(v) = line.strip_prefix("exec = \"").and_then(|s| s.strip_suffix('"')) {
            let mut r = v.to_string();
            for var in ["SYSTEMDRIVE", "SYSTEMROOT", "WINDIR"] {
                if let Ok(val) = std::env::var(var) { r = r.replace(&format!("%{var}%"), &val); }
            }
            exec = r;
        }
    }
    if !name.is_empty() && !exec.is_empty() { icons.push((name, exec)); }
    if icons.is_empty() { None } else { Some(icons) }
}

fn default_icons() -> Vec<(String, String)> {
    let mut v = Vec::new();
    for d in ["X:\\Users\\Default\\Desktop", "X:\\Users\\Public\\Desktop", "C:\\Users\\Public\\Desktop"] {
        if let Ok(e) = std::fs::read_dir(d) {
            for x in e.flatten() {
                let p = x.path();
                if p.extension().map(|e| e == "lnk").unwrap_or(false) {
                    v.push((
                        p.file_stem().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
                        p.to_string_lossy().to_string(),
                    ));
                }
            }
        }
    }
    if v.is_empty() {
        for (n, p) in [
            ("命令提示符", "%SYSTEMROOT%\\System32\\cmd.exe"),
            ("云下载重装", "X:\\restore\\restore-app.exe"),
            ("测试菜单", "cmd.exe /k X:\\sources\\test-menu.cmd"),
        ] {
            let mut r = p.to_string();
            for var in ["SYSTEMROOT", "WINDIR"] {
                if let Ok(val) = std::env::var(var) { r = r.replace(&format!("%{var}%"), &val); }
            }
            v.push((n.to_string(), r));
        }
    }
    v
}

fn icon_handle(path: &str) -> Option<iced::widget::image::Handle> {
    let (w, h, px) = extract_icon_rgba(path)?;
    Some(iced::widget::image::Handle::from_rgba(w as u32, h as u32, px))
}

// ---------------- iced 应用 ----------------

struct Shell {
    clock: String,
    tz_bias: i32,
    start_open: bool,
    icons: Vec<DesktopIcon>,
    windows: Vec<WindowInfo>,
    shutdown_confirm: bool,
}

#[derive(Debug, Clone)]
enum Msg {
    Tick,
    ToggleStart,
    Launch(String),
    MinimizeWindow(isize),
    FocusWindow(isize),
    ShutdownAsk(bool),
    Shutdown,
    RefreshIcons,
}

fn main() -> iced::Result {
    let tz = timezone_bias();
    let mut icons: Vec<DesktopIcon> = load_apps_from_toml()
        .unwrap_or_else(default_icons)
        .into_iter()
        .map(|(name, path)| DesktopIcon { name, path, icon_handle: None })
        .collect();
    // 图标提取 (前 24 个, 避免启动卡顿)
    for i in icons.iter_mut().take(24) {
        i.icon_handle = icon_handle(&i.path);
    }

    let mut app = iced::application("QAQ-Shell", update, view)
        .theme(|_| Theme::Dark)
        .window(iced::window::Settings {
            size: iced::Size::new(1024.0, 720.0),
            resizable: false,
            decorations: false,
            ..Default::default()
        })
        .subscription(|_shell| {
            iced::time::every(std::time::Duration::from_secs(1)).map(|_| Msg::Tick)
        });
    if let Some(bytes) = cjk_font_bytes() {
        app = app.font(bytes);
    }
    let fullscreen_task = iced::window::get_latest()
        .and_then(|id| iced::window::change_mode::<Msg>(id, iced::window::Mode::Fullscreen).map(|_| Msg::Tick));
    app.run_with(move || {
        (
            Shell {
                clock: clock(tz),
                tz_bias: tz,
                start_open: false,
                icons,
                windows: enumerate_windows(),
                shutdown_confirm: false,
            },
            fullscreen_task,
        )
    })
}

fn cjk_font_bytes() -> Option<std::borrow::Cow<'static, [u8]>> {
    for p in [
        "X:\\Windows\\Fonts\\msyh.ttc",
        "C:\\Windows\\Fonts\\msyh.ttc",
        "C:\\Windows\\Fonts\\simsun.ttc",
    ] {
        if let Ok(d) = std::fs::read(p) {
            return Some(std::borrow::Cow::Owned(d));
        }
    }
    None
}

fn update(shell: &mut Shell, msg: Msg) {
    match msg {
        Msg::Tick => {
            shell.clock = clock(shell.tz_bias);
            shell.windows = enumerate_windows();
        }
        Msg::ToggleStart => {
            shell.start_open = !shell.start_open;
            shell.shutdown_confirm = false;
        }
        Msg::Launch(path) => {
            shell.start_open = false;
            launch(&path);
        }
        Msg::MinimizeWindow(hwnd) => unsafe {
            ShowWindow(hwnd as *mut _, SW_MINIMIZE);
        },
        Msg::FocusWindow(hwnd) => focus_window(hwnd),
        Msg::ShutdownAsk(b) => {
            shell.shutdown_confirm = b;
            shell.start_open = b && shell.start_open;
        }
        Msg::Shutdown => unsafe {
            ExitWindowsEx(EWX_POWEROFF | EWX_FORCE, 0);
        },
        Msg::RefreshIcons => {
            for i in shell.icons.iter_mut().take(24) {
                if i.icon_handle.is_none() {
                    i.icon_handle = icon_handle(&i.path);
                }
            }
        }
    }
}

// ---------------- 视图 ----------------

const FONT: iced::Font = iced::Font::with_name("Microsoft YaHei");

fn f(t: impl Into<String>) -> iced::widget::Text<'static> {
    text(t.into()).font(FONT)
}

fn themed_button<'a>(
    label: iced::widget::Text<'a>,
    msg: Msg,
    selected: bool,
) -> Element<'a, Msg> {
    button(container(label).center_x(Fill))
        .width(Fill)
        .padding(6)
        .style(move |t: &Theme, st| {
            let bg = if selected {
                t.palette().primary
            } else if matches!(st, button::Status::Hovered) {
                iced::Color::from_rgba8(255, 255, 255, 0.12)
            } else {
                iced::Color::from_rgba8(255, 255, 255, 0.0)
            };
            button::Style {
                background: Some(iced::Background::Color(bg)),
                text_color: iced::Color::WHITE,
                border: iced::Border { radius: 4.0.into(), ..Default::default() },
                ..Default::default()
            }
        })
        .into()
}

fn view(shell: &Shell) -> Element<'_, Msg> {
    let taskbar_h = 52.0;

    // ---- 桌面: 图标网格 (每行 6 个, 手动分行) ----
    let per_row = 6;
    let mut icon_rows: Vec<iced::widget::Row<'_, Msg>> = Vec::new();
    let mut cur = row![].spacing(8);
    let mut n = 0;
    for icon in &shell.icons {
        if n >= 24 { break; }
        let path = icon.path.clone();
        let name = icon.name.clone();
        let mut icon_view: Element<'_, Msg> = match &icon.icon_handle {
            Some(h) => image(h.clone()).width(32).height(32).into(),
            None => text("📄").font(FONT).size(24).into(),
        };
        icon_view = mouse_area(icon_view).on_press(Msg::Launch(path.clone())).into();
        let tile = container(
            column![
                container(icon_view).center_x(Fill),
                f(name).size(13).center(),
            ]
            .spacing(4)
            .align_x(Center),
        )
        .width(120)
        .center_x(Fill);
        cur = cur.push(tile);
        n += 1;
        if n % per_row == 0 {
            icon_rows.push(cur);
            cur = row![].spacing(8);
        }
    }
    if n % per_row != 0 || icon_rows.is_empty() {
        icon_rows.push(cur);
    }
    let mut grid = column![].spacing(8);
    for r in icon_rows {
        grid = grid.push(r);
    }

    let desktop = mouse_area(
        container(grid).padding(12).width(Fill).height(Fill).align_y(alignment::Alignment::Start),
    )
    .on_right_press(Msg::RefreshIcons);

    // ---- 任务栏 ----
    let start_btn = themed_button(
        f(if shell.start_open { " ▲ 开始 " } else { " 开始 " }),
        Msg::ToggleStart,
        shell.start_open,
    );

    let mut win_btns = row![].spacing(4);
    for w in shell.windows.iter().take(5) {
        win_btns = win_btns.push(
            mouse_area(
                container(f(truncate(&w.title, 24)).size(13)).width(160).center_x(Fill),
            )
            .on_press(Msg::FocusWindow(w.hwnd)),
        );
    }

    let taskbar = container(
        row![
            container(start_btn).width(90),
            container(win_btns).width(Fill),
            f(shell.clock.clone()).size(16),
            container(
                button(f("⏻").size(16)).on_press(Msg::ShutdownAsk(true))
                    .style(|_t: &Theme, st| button::Style {
                        background: Some(iced::Background::Color(
                            if matches!(st, button::Status::Hovered) { iced::Color::from_rgba8(200, 60, 60, 0.9) }
                            else { iced::Color::from_rgba8(255, 255, 255, 0.08) }
                        )),
                        text_color: iced::Color::WHITE,
                        border: iced::Border { radius: 4.0.into(), ..Default::default() },
                        ..Default::default()
                    })
            ).width(50).center_x(Fill),
        ]
        .height(Fill)
        .align_y(Center),
    )
    .width(Fill)
    .height(taskbar_h)
    .style(|_t| container::Style {
        background: Some(iced::Background::Color(iced::Color::from_rgba8(20, 24, 32, 0.96))),
        ..Default::default()
    });

    // ---- 开始菜单 (叠在桌面上) ----
    let start_menu = container(
        column![
            f("QAQ-Shell").size(18),
            column(
                shell
                    .icons
                    .iter()
                    .map(|i| themed_button(f(i.name.clone()), Msg::Launch(i.path.clone()), false).into())
                    .collect::<Vec<_>>(),
            )
            .spacing(2),
        ]
        .spacing(10),
    )
    .padding(14)
    .width(320)
    .style(|_t| container::Style {
        background: Some(iced::Background::Color(iced::Color::from_rgba8(28, 30, 40, 0.98))),
        border: iced::Border {
            radius: 8.0.into(),
            width: 1.0,
            color: iced::Color::from_rgba8(255, 255, 255, 0.15),
        },
        ..Default::default()
    });

    // ---- 组装: stack 叠加 (桌面 / 开始菜单), 下方任务栏 ----
    let start_overlay = if shell.start_open {
        container(start_menu)
            .padding(iced::Padding::ZERO.top(taskbar_h))
            .width(Fill).height(Fill).align_y(alignment::Alignment::Start)
    } else {
        container(text("").font(FONT)).width(Fill).height(Fill)
    };

    let shutdown_dialog = if shell.shutdown_confirm {
        Some(container(
            column![
                f("确认关机?").size(20),
                row![
                    button(f("取消").font(FONT)).on_press(Msg::ShutdownAsk(false)),
                    button(f("确认关机").font(FONT)).on_press(Msg::Shutdown),
                ].spacing(12),
            ]
            .spacing(16)
            .align_x(Center),
        )
        .padding(20)
        .style(|_t| container::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgba8(30, 30, 36, 0.99))),
            border: iced::Border { radius: 8.0.into(), ..Default::default() },
            ..Default::default()
        }))
    } else {
        None
    };

    let desktop_layer = column![
        mouse_area(
            container(desktop)
                .width(Fill)
                .height(Fill)
                .style(|_t| container::Style {
                    background: Some(iced::Background::Color(iced::Color::from_rgb8(10, 40, 80))),
                    ..Default::default()
                })
        )
        .on_right_press(Msg::RefreshIcons),
        taskbar,
    ]
    .height(Fill);

    let base = match shutdown_dialog {
        Some(d) => Element::from(stack![desktop_layer, container(d).center(Fill)].height(Fill)),
        None => Element::from(desktop_layer),
    };

    stack![base, start_overlay].into()
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max { s.to_string() } else {
        format!("{}…", s.chars().take(max).collect::<String>())
    }
}
