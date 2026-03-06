<div align="center">

# ⚡ VanceSender R

**FiveM `/me` `/do` 文本发送器 — Rust 原生重写版**

[![Rust CI](https://github.com/vancehuds/VanceSender/actions/workflows/rust-ci.yml/badge.svg)](https://github.com/vancehuds/VanceSender/actions/workflows/rust-ci.yml)
[![Release](https://img.shields.io/github/v/release/vancehuds/VanceSender?include_prereleases&label=latest)](https://github.com/vancehuds/VanceSender/releases)
![Platform](https://img.shields.io/badge/platform-Windows%20x64-blue)
![Rust](https://img.shields.io/badge/Rust-2021-orange?logo=rust)

</div>

---

VanceSenderR 是 VanceSender 的 Rust 原生重写版本，将发送引擎、GUI 和 HTTP 服务合并为一个高性能的轻量级单文件可执行程序。使用 **Win32 SendInput** 模拟键盘输入，自动将 `/me`、`/do` 等 RP 文本逐行发送至 FiveM 游戏聊天窗口，并内置 **AI 文本生成** 能力。

## ✨ 功能特性

| 功能 | 说明 |
|------|------|
| **键盘模拟发送** | 通过剪贴板粘贴（Clipboard）或逐字符输入（Typing）两种方式将文本发送至 FiveM |
| **AI 文本生成** | 多 Provider 支持（兼容 OpenAI API），可根据场景自动生成 `/me` `/do` RP 文本 |
| **AI 文本改写** | 对已有文本进行智能润色、改写 |
| **预设管理** | 创建、编辑、导入、导出文本预设，支持标签分组 |
| **快速叠加层** | 半透明快捷悬浮窗（Quick Overlay），F7 热键唤起，快速发送常用文本 |
| **系统托盘** | 最小化到托盘，右键菜单快速操作 |
| **WebUI** | 内置 HTTP 服务器，支持浏览器 / 手机 LAN 远程控制 |
| **原生 GUI** | 基于 egui/eframe 的高性能原生界面，无浏览器依赖 |
| **发送统计** | 跟踪发送次数、字符数、成功率等统计信息 |
| **自动更新检测** | 启动时检查 GitHub Release，提示新版本 |
| **YAML 配置** | 全部设置通过 `config.yaml` 管理，支持热加载 + 原子写入 |

## 🏗️ 架构概览

```
VanceSenderR/
├── src/
│   ├── main.rs            # 入口：CLI 解析、HTTP 服务器 + GUI 启动
│   ├── config.rs          # YAML 配置管理（缓存、原子写入、深度合并）
│   ├── state.rs           # 全局共享状态 (Arc<AppState>)
│   ├── error.rs           # 统一错误类型
│   ├── app_meta.rs        # 应用元数据（版本号、仓库地址）
│   ├── api/               # Axum HTTP API 路由
│   │   ├── mod.rs         #   路由注册
│   │   ├── sender.rs      #   发送 API
│   │   ├── ai.rs          #   AI 生成 / 改写 API（支持 SSE 流式）
│   │   ├── presets.rs     #   预设 CRUD API
│   │   ├── settings.rs   #   设置读写 API
│   │   └── stats.rs      #   统计 API
│   ├── core/              # 核心业务逻辑
│   │   ├── sender.rs      #   Win32 键盘模拟发送引擎
│   │   ├── ai_client.rs   #   多 Provider AI 客户端
│   │   ├── presets.rs     #   预设文件持久化
│   │   ├── stats.rs      #   发送统计
│   │   ├── history.rs    #   发送历史
│   │   ├── ai_history.rs #   AI 生成历史
│   │   ├── network.rs    #   LAN IP 检测
│   │   ├── notifications.rs  #  通知管理
│   │   ├── public_config.rs  #  远程公共配置
│   │   └── update_checker.rs #  版本更新检查
│   ├── gui/               # egui 原生 GUI
│   │   ├── mod.rs         #   主窗口 + 面板路由
│   │   ├── theme.rs       #   主题配色
│   │   ├── sidebar.rs     #   侧边导航栏
│   │   ├── titlebar.rs    #   自定义标题栏
│   │   ├── panels/        #   各功能面板
│   │   │   ├── home.rs    #     首页仪表板
│   │   │   ├── send.rs    #     发送面板
│   │   │   ├── quick_send.rs #  快速发送
│   │   │   ├── ai_generate.rs # AI 生成
│   │   │   ├── presets.rs #     预设管理
│   │   │   └── settings.rs #    设置
│   │   └── widgets/       #   可复用组件
│   │       ├── preset_card.rs   # 预设卡片
│   │       ├── progress.rs      # 进度条
│   │       ├── tag_filter.rs    # 标签筛选器
│   │       └── text_list.rs     # 文本列表
│   └── desktop/           # 桌面集成
│       ├── tray.rs        #   系统托盘
│       └── quick_overlay.rs #  快捷悬浮窗
├── .github/workflows/
│   ├── rust-ci.yml        # CI：Clippy lint + Release 构建
│   └── rust-release.yml   # CD：自动打包 + GitHub Release 发布
├── Cargo.toml
└── Cargo.lock
```

## 🚀 快速开始

### 前置条件

- **操作系统**：Windows 10/11 x64
- **Rust 工具链**：1.75+（建议使用 [rustup](https://rustup.rs/)）

### 构建 & 运行

```bash
# 克隆仓库
git clone https://github.com/vancehuds/VanceSender.git
cd VanceSender

# Debug 构建并运行
cargo run

# Release 构建（推荐，性能更优）
cargo build --release
./target/release/vancesender.exe
```

### 命令行参数

```
vancesender [OPTIONS]

Options:
  -p, --port <PORT>    指定 HTTP 服务端口（默认 8730）
      --lan            监听所有网络接口（局域网访问）
      --no-gui         无头模式（仅启动 HTTP 服务器）
  -h, --help           帮助信息
```

### 使用示例

```bash
# 默认启动（GUI + HTTP 服务 @ 127.0.0.1:8730）
vancesender.exe

# 指定端口 + 开启局域网访问
vancesender.exe --port 9090 --lan

# 无头服务模式（通过 WebUI / API 控制）
vancesender.exe --no-gui --lan
```

## ⚙️ 配置

程序启动时会读取同目录下的 `config.yaml`，若不存在则自动使用默认配置。

<details>
<summary>📄 默认配置参考</summary>

```yaml
server:
  host: "127.0.0.1"
  port: 8730
  lan_access: false
  token: ""

launch:
  open_webui_on_start: false
  enable_tray_on_start: true
  close_action: ask          # ask / minimize / exit

sender:
  method: clipboard          # clipboard / typing
  chat_open_key: t           # FiveM 聊天打开键
  delay_open_chat: 450       # 打开聊天后等待（ms）
  delay_after_paste: 160     # 粘贴后等待（ms）
  delay_after_send: 260      # 发送后等待（ms）
  delay_between_lines: 1800  # 行间延迟（ms）
  focus_timeout: 8000        # 等待 FiveM 窗口聚焦超时（ms）
  retry_count: 3             # 发送失败重试次数
  typing_char_delay: 18      # 逐字符输入模式的字符间隔（ms）

quick_overlay:
  enabled: true
  trigger_hotkey: f7
  theme:
    bg_opacity: 0.92
    accent_color: "#7c5cff"
    font_size: 12

ai:
  providers: []              # OpenAI 兼容 API 列表
  default_provider: ""
  system_prompt: ""
  custom_headers: {}
```

</details>

## 🤖 AI 文本生成

VanceSender 支持接入任何 **OpenAI API 兼容** 的大模型服务。

### 配置 Provider

在设置面板（或直接编辑 `config.yaml`）中添加 Provider：

```yaml
ai:
  providers:
    - id: "my-openai"
      name: "OpenAI"
      api_base: "https://api.openai.com/v1"
      api_key: "sk-..."
      model: "gpt-4o"
    - id: "my-local"
      name: "本地模型"
      api_base: "http://localhost:1234/v1"
      api_key: ""
      model: "qwen2.5"
  default_provider: "my-openai"
```

兼容平台包括但不限于：OpenAI、DeepSeek、智谱、通义千问、Ollama、LM Studio、vLLM 等。

## 📡 HTTP API

应用启动后会在配置端口开放 RESTful API，完整的端点列表：

| 方法 | 端点 | 说明 |
|------|------|------|
| `POST` | `/api/send` | 发送文本到 FiveM |
| `POST` | `/api/send/cancel` | 取消正在进行的发送 |
| `GET`  | `/api/send/progress` | 获取发送进度 |
| `POST` | `/api/ai/generate` | AI 生成文本 |
| `POST` | `/api/ai/generate/stream` | AI 流式生成（SSE） |
| `POST` | `/api/ai/rewrite` | AI 改写文本 |
| `GET`  | `/api/presets` | 获取预设列表 |
| `POST` | `/api/presets` | 创建预设 |
| `GET`  | `/api/settings` | 获取设置 |
| `PATCH`| `/api/settings` | 更新设置 |
| `GET`  | `/api/stats` | 获取统计数据 |

## 🔧 技术栈

| 组件 | 技术 |
|------|------|
| 语言 | Rust 2021 Edition |
| GUI | [eframe](https://github.com/emilk/egui) 0.31 / egui |
| HTTP 服务器 | [Axum](https://github.com/tokio-rs/axum) 0.8 |
| 异步运行时 | [Tokio](https://tokio.rs/) |
| HTTP 客户端 | [reqwest](https://github.com/seanmonstar/reqwest) (rustls) |
| 序列化 | serde + serde_yaml + serde_json |
| 系统托盘 | tray-icon + muda |
| 键盘模拟 | Win32 `SendInput` API |
| CI/CD | GitHub Actions |

## 📦 发布

推送 `v*` 格式的 Git tag 会自动触发 GitHub Actions 构建流水线：

```bash
git tag v2.0.0
git push origin v2.0.0
```

流水线将自动：
1. 注入版本号到源码
2. 构建 Release 二进制
3. 打包为 ZIP（含默认配置）
4. 创建 GitHub Release 并上传

## 📄 开源协议

本项目为私有项目。All rights reserved.
