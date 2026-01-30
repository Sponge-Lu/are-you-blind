# EyeGuard 👁️

> *因为你的眼睛不会说停。*

一款轻量级、跨平台的护眼工具，基于 **20-20-20 法则**：每工作 20 分钟，看向 20 英尺（约 6 米）外的物体 20 秒。

## 为什么需要这个？

- 盯着屏幕时，**眨眼频率下降 66%**
- **数字眼疲劳**是真实存在的问题
- 大多数护眼工具都是臃肿的 Electron/WebView 应用，占用 100MB+ 内存

**EyeGuard 与众不同。**

## 功能特性

- ⚡ **纯原生 Rust** - 基于 [Slint](https://slint.dev/) UI 框架
- 🪶  **极致轻量** - 内存占用 < 10MB（其他工具通常 100MB+）
- 🚀 **即时启动** - 无需加载浏览器引擎
- 🎨 **精美界面** - 现代化暗色主题，圆角设计，透明效果
- 🔒 **强制休息** - 全屏遮罩，确保你真的休息
- 🖥️ **多显示器支持** - 休息时覆盖所有屏幕
- 🔧 **开箱即用** - 无需配置

## 安装

从 [Releases](https://github.com/Sponge-Lu/are_you_blind/releases) 页面下载最新版本。

## 开发

环境要求：
- [Rust](https://www.rust-lang.org/tools/install)
- [Cargo](https://doc.rust-lang.org/cargo/)

```bash
# 开发模式运行
cargo run

# 构建发布版本
cargo build --release
```

## 架构

```
┌─────────────────┐
│  src_rust/      │  <-- 逻辑层 (计时器、状态、托盘)
│    main.rs      │
└────────┬────────┘
         │ 状态 & 回调
┌────────▼────────┐
│  ui/            │  <-- 展示层 (Slint)
│    appwindow.slint
└─────────────────┘
```

## 许可证

MIT
