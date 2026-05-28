# QAQ-Shell

A native WinPE desktop shell written in Rust + egui. Replaces winpeshl.ini and WinXShell.

## Features

- Fullscreen desktop with wallpaper
- Taskbar with start menu, pinned apps, clock, shutdown
- Desktop icons (reads .lnk shortcuts)
- DPI-aware scaling (800×600 to 4K)
- ShellExecuteW launcher (no extra cmd process)
- CJK font support (Microsoft YaHei)

## Build

```bash
cargo build --release
# output: target/release/qaq-shell.exe (~3 MB, static CRT)
```

## Usage

In WinPE, replace `winpeshl.ini`:

```ini
[LaunchApps]
X:\Windows\System32\watchdog.exe,"X:\qaq-shell\qaq-shell.exe"
```

Or test on host (close explorer first):

```bash
taskkill /f /im explorer.exe
qaq-shell.exe
# exit: Alt+Tab → Win+R → explorer.exe
```

## Architecture

```
qaq-shell
  ├── egui            → UI rendering (desktop, taskbar, start menu)
  ├── shell32 (raw)   → ShellExecuteW for launching apps
  ├── user32 (raw)    → EnumWindows, SetForegroundWindow (planned)
  └── shell-system    → Safe Win32 API wrappers (TBD)
```

## Dependencies

| Crate | Purpose |
|-------|---------|
| egui 0.31 | Immediate-mode GUI |
| eframe 0.31 | Window/OpenGL backend (glow) |
| windows-rs 0.62 | Win32 shell APIs (planned) |

## What it replaces vs WinXShell

| Feature | QAQ-Shell | WinXShell |
|---------|:---:|:---:|
| Taskbar | ✅ | ✅ |
| Start menu | ✅ | ✅ |
| Desktop icons | ✅ | ✅ |
| System tray | 🔜 | ✅ |
| Running windows | 🔜 | ✅ |
| Exe icon extract | 🔜 | ✅ |
| Global hotkeys | 🔜 | 🔜 |
| CJK font | ✅ | ✅ |
| DPI scaling | ✅ | ❌ |
| Binary size | 3 MB | 3.4 MB |
| Language | Rust | C++ / Lua |
| Config | Code | jcfg / lua |

## Why not Electron/React/Tauri?

PE has no Chromium/Node.js runtime. Native Rust keeps the binary under 3 MB with zero external dependencies.

## License

MIT
