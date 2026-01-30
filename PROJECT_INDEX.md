# EyeGuard 项目索引

## 项目概览

EyeGuard 是一个轻量级跨平台护眼工具，基于纯 Rust + Slint UI 框架构建，遵循 20-20-20 规则：每工作 20 分钟，看 20 英尺外的物体 20 秒。

**项目根目录**: `are_you_blind`

**索引更新**: 2026-01-29

**技术栈**:
- **语言**: Rust 2021 Edition
- **UI 框架**: [Slint](https://slint.dev/) 1.9 (Skia 渲染器 + Winit 后端)
- **系统托盘**: tray-icon 0.19

**项目规模**:
- **源代码文件**: 2 (main.rs, dump_monitors.rs)
- **UI 文件**: 1 (appwindow.slint)
- **代码行数**: ~2300 行

## 文件夹结构

```
are_you_blind/
├── .cargo/
│   └── config.toml         # Cargo 配置 (MSVC 目标)
├── .github/
│   └── workflows/
│       └── build.yml       # CI/CD 工作流
├── src_rust/
│   ├── main.rs             # 主程序入口 (~830 行)
│   └── bin/
│       └── dump_monitors.rs # 监视器调试工具
├── ui/
│   └── appwindow.slint     # UI 定义 (~1150 行)
├── Cargo.toml              # 依赖配置
├── Cargo.lock              # 依赖锁定
├── build.rs                # Slint 编译脚本
├── build_msvc.bat          # Windows MSVC 构建脚本
├── README.md               # 项目说明
└── PROJECT_INDEX.md        # 本文件
```

## 架构视图

```
┌─────────────────────────────────────────────────────┐
│                    main.rs                          │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐ │
│  │  AppState   │  │   Timers    │  │  Callbacks  │ │
│  │  --------   │  │  --------   │  │  --------   │ │
│  │  is_paused  │  │  main_timer │  │  on_toggle  │ │
│  │  mode       │  │  tray_timer │  │  on_reset   │ │
│  │  durations  │  │             │  │  on_settings│ │
│  │  rest_type  │  │             │  │             │ │
│  └─────────────┘  └─────────────┘  └─────────────┘ │
│                                                     │
│  ┌─────────────────────────────────────────────┐   │
│  │              Windows API Layer              │   │
│  │  - DPI Awareness (Per-Monitor V2)          │   │
│  │  - Monitor Enumeration (EnumDisplayMonitors)│   │
│  │  - Virtual Screen Rect                     │   │
│  └─────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────┘
                         │
                         │ UI Callbacks & State Binding
                         ▼
┌─────────────────────────────────────────────────────┐
│                 appwindow.slint                     │
│  ┌─────────────┐  ┌─────────────┐  ┌───────────┐   │
│  │ MainWindow  │  │RestOverlay  │  │ AppPalette│   │
│  │  - 进度环   │  │  - 全屏遮罩 │  │  - 主题色 │   │
│  │  - 计时显示 │  │  - 休息提示 │  │  - 暗/亮  │   │
│  │  - 控制按钮 │  │  - 倒计时   │  │           │   │
│  │  - 设置面板 │  │             │  │           │   │
│  └─────────────┘  └─────────────┘  └───────────┘   │
└─────────────────────────────────────────────────────┘
```

## 核心模块说明

### main.rs

| 组件 | 说明 |
|------|------|
| `AppState` | 应用状态：暂停状态、工作/休息时长、当前模式、休息类型 |
| `Mode` | 枚举：`Work` / `Rest` |
| `RestType` | 枚举：`EyeRest` / `Water` / `Walk` |
| `enable_windows_per_monitor_dpi_awareness()` | Windows DPI 感知设置 |
| `monitor_rects()` | 多显示器检测 |
| `show/update/hide_rest_overlay()` | 全屏休息遮罩管理 |
| `create_tray_icon()` | 系统托盘图标与菜单 |

### appwindow.slint

| 组件 | 说明 |
|------|------|
| `AppPalette` | 全局主题配置（颜色、暗/亮模式） |
| `MainWindow` | 主窗口：无边框、置顶、可拖拽、300x400px |
| `RestOverlayWindow` | 休息遮罩窗口：全屏、黑色背景、倒计时显示 |
| `Icon*` | SVG 图标组件（Play、Pause、Reset、Settings、Sun、Moon） |

## 功能特性

- **计时器**: 可配置的工作/休息周期
- **多显示器支持**: 休息时覆盖所有显示器
- **系统托盘**: 后台运行，托盘菜单控制
- **主题切换**: 暗色/亮色主题
- **提醒类型**: 眼睛休息、喝水提醒、走动提醒
- **无边框窗口**: 现代 UI，支持拖拽

## 依赖项

| 依赖 | 版本 | 用途 |
|------|------|------|
| slint | 1.9 | UI 框架 |
| tray-icon | 0.19 | 系统托盘 |
| slint-build | 1.9 | 构建时 Slint 编译 |

## 构建与运行

```bash
# 开发模式
cargo run

# 发布构建
cargo build --release

# Windows MSVC 构建
build_msvc.bat
```

## CI/CD

GitHub Actions 工作流支持：
- Windows / macOS / Linux 三平台构建
- 代码格式检查 (`cargo fmt`)
- Clippy 代码检查
- 自动发布到 GitHub Releases

---

*最后更新: 2026-01-29*
