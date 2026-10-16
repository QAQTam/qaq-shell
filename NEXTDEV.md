# QAQ-Shell — Next Development

## 当前状态

- egui fullscreen 桌面壳，支持 DPI 自适应
- 任务栏（开始菜单、快捷启动、时钟、关机）
- 桌面图标（读取 .lnk 快捷方式）
- ShellExecuteW 启动外部程序

## 下一步：shell-system crate

### 目标

将 Win32 shell API 封装为独立 safe Rust crate `shell-system`，供 qaq-shell 调用。

### windows-rs 覆盖度：20/20 全齐

| 功能 | windows-rs API | Feature |
|------|---------------|---------|
| 窗口枚举 | `EnumWindows` | `Win32_UI_WindowsAndMessaging` |
| 窗口前置 | `SetForegroundWindow` | 同上 |
| 窗口可见/风格 | `IsWindowVisible`, `GetWindowLongW` | 同上 |
| 窗口定位/大小 | `SetWindowPos`, `ShowWindow`, `MoveWindow` | 同上 |
| 壁纸 | `SystemParametersInfoW` | 同上 |
| 系统托盘 | `Shell_NotifyIconW`, `NOTIFYICONDATAW` | `Win32_UI_Shell` |
| 提取 exe 图标 | `SHGetFileInfoW`, `ExtractIconExW` | 同上 |
| 快捷方式解析 | `IShellLinkW` | 同上 |
| 任务栏按钮 | `ITaskbarList3` | 同上 |
| 任务栏 AppBar | `SHAppBarMessage` | 同上 |
| 全局热键 | `RegisterHotKey` | `Win32_UI_Input_KeyboardAndMouse` |
| 拖放 | `IDropTarget` | `Win32_System_Ole` |
| 窗口边框/圆角 | `DwmSetWindowAttribute` | `Win32_Graphics_Dwm` |

### shell-system API 设计（草案）

```
shell-system/
  src/
    window.rs    → enum_running(), focus_window()
    tray.rs      → add_tray_icon(), remove_tray_icon()
    icon.rs      → extract_icon(), resolve_lnk()
    hotkey.rs    → register_hotkey(), unregister_hotkey()
    desktop.rs   → set_wallpaper(), get_wallpaper()
    taskbar.rs   → appbar_register(), appbar_set_pos()
    lib.rs       → pub mod
```

### 优先级

1. `window.rs` — 窗口管理（EnumWindows + SetForegroundWindow）
2. `icon.rs`   — 真实图标替代 📄 emoji
3. `hotkey.rs` — Win+E 等全局热键
4. `desktop.rs`— 壁纸设置
5. `tray.rs`   — 系统托盘
6. `taskbar.rs`— 任务栏窗口列表按钮

## 不碰的（内核专属）

- 输入法候选窗 (TSF UI Host)
- Alt+Tab 窗口切换
- 右键菜单 COM 扩展 (IContextMenu)
- 文件复制进度条 (IFileOperation)

## 依赖

```toml
[dependencies]
windows = { version = "0.62", features = [
    "Win32_UI_WindowsAndMessaging",
    "Win32_UI_Shell",
    "Win32_UI_Input_KeyboardAndMouse",
    "Win32_System_Ole",
    "Win32_Graphics_Dwm",
    "Win32_Foundation",
]}
```

## iced 迁移支线 (iced-shell/, 2026-10-17)

egui/eframe-glow 在 WinRE/PE 里**不可用**: WinRE 无 opengl32.dll (连 GL 库都不存在),
且无 api-ms-win-crt-* 转发器/vcruntime140.dll。对策已验证:

- `iced-shell/` = egui 版的 iced (tiny-skia 软渲染) 迁移版, 零 GPU/GL 依赖,
  静态 CRT, 3.9MB, 宿主机全屏运行验证通过 (桌面/图标/任务栏/开始菜单/时钟/关机)
- CRT 必须静态: `.cargo/config.toml` → `+crt-static` (WinRE 只有 ucrtbase.dll)
- 中文字体: 运行时读 msyh.ttc 字节 → `app.font(bytes)` → `text().font(Font::with_name("Microsoft YaHei"))`

### iced 0.13 API 笔记 (egui→iced 迁移踩坑)
- feature 体系: 无 `winit` feature; 用 `default-features=false, features=["tiny-skia","image","smol","system"]`
- 常量: `DEFAULT_CHARSET`/`CLEARTYPE_QUALITY` 在 `wingdi`
- `Border::with_radius` 不存在 → `Border { radius: R.into(), ..Default::default() }`
- `button::Status` 无 `is_hovered()` → `matches!(st, Status::Hovered)`
- 全屏: `Settings` 无 `fullscreen` 字段 → 启动 Task:
  `window::get_latest().and_then(|id| window::change_mode::<Msg>(id, Mode::Fullscreen))`
- `HINTERNET`/Win32 句柄非 Sync → 多线程下载每线程独立会话
- `Padding` 无 `From<[f32;4]>` → `Padding::ZERO.top(f32)`
- `text()` 无 `center_x` → `container(text).center_x(Fill)`
- Stack 叠加层: `stack![base, overlay]`; 任务栏高度用 `Padding::ZERO.bottom(h)` 预留

### 待办
- [ ] WinRE 实机验证 iced 渲染 (VBox: X:\qaq-shell\iced-shell\iced-shell.exe)
- [ ] winevent hook 精细化 (现为 1s 轮询枚举, 够用)
- [ ] perfmon 迁移为 iced 窗口
- [ ] 与 restore-win11 联动: 桌面图标「云下载重装」→ restore-app.exe (TOML 已配)
