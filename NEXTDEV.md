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
