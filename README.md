# Codex 保安

Codex 保安是一个开箱即用的本机桌面小工具，用来帮 Codex 用户快速看清当前连接的上游、审计本机会话记录里的高风险命令，并在不打断日常使用的前提下保持后台提醒。

它的目标不是做复杂网关或大型安全平台，而是保持轻量、方便、低负担：

- 开箱即用：安装后自动发现本机 Codex / ccswitch / Codex++ 配置，不要求用户手动填写 Base URL 或 API Key。
- 即插即用：启动后自动进入本地审计状态，托盘常驻，需要时点开查看。
- 系统占用低：只读取本机配置和 Codex 会话日志，不做代理常驻、不接管网络链路。
- 卸载无残留负担：安装版走系统卸载入口；便携版解压即用，不写入开机自启动；开发运行也不会写入系统启动项。
- 方便快捷：主界面只保留上游状态、审计状态、风险记录、更新和卸载入口。

技术上它是 Tauri 2 桌面应用：Rust 后端负责本机配置发现、本地审计状态和命令风险提示，React/TypeScript 前端负责桌面界面展示。它是标准桌面应用，不是浏览器页面、脚本壳或插件包装。

## 普通用户安装

从 GitHub Releases 下载最新安装包：

https://github.com/jliushi/codex-baoan/releases/latest

Tauri 会生成标准桌面安装产物：

- Windows MSI 安装版：推荐使用，安装到用户目录，支持开始菜单、卸载项、开机自启动和应用内更新
- Windows NSIS 安装器：备用安装入口
- Windows Portable 便携版：解压即用，不写入开机自启动，不参与应用内自动更新

卸载走系统应用管理。Windows 用户可以在应用右侧设置抽屉里点击「卸载入口」，或打开「设置 > 应用 > 已安装的应用」。

开机自启动仅安装版可开启。便携版和开发运行不会新写入系统启动项，避免目录移动、删除或卸载后留下无效启动项。

### 自动更新

应用内置了自动更新功能：

1. 打开应用，点击右上角「设置」按钮
2. 在「版本更新」区域点击「检查更新」
3. 如果有新版本，点击「立即更新」按钮
4. 更新下载完成后，应用会自动重启到新版本

所有更新包都经过签名验证，确保安全性。开发模式和 Portable 便携版不会执行应用内更新，可通过下载页手动安装正式版本。

### 维护者发布

正式自动更新依赖 Tauri updater 签名。仓库 Secrets 需要配置与 `src-tauri/tauri.conf.json` 中 `plugins.updater.pubkey` 配对的私钥：

- `TAURI_SIGNING_PRIVATE_KEY`
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`

推送 `v*` tag 后，GitHub Actions 会构建 Windows MSI/NSIS/Portable 产物、签名 MSI 更新包并上传 `latest.json` 到本次 Release。已安装的 MSI 版本会从 Releases latest endpoint 检查并安装新版本。

## 开发运行

本项目使用 pnpm + Tauri。开发启动也先构建本地 `dist/`，再启动 Tauri 桌面窗口；不会暴露浏览器网页入口：

```bash
pnpm install
pnpm run dev
```

类型检查：

```bash
pnpm run typecheck
```

构建安装包：

```bash
pnpm run package:windows:unsigned
```

正式发布使用 `pnpm run package:windows`，需要设置 `TAURI_SIGNING_PRIVATE_KEY` 以生成应用内更新签名。

本地构建产物会出现在：

```text
src-tauri/target/release/bundle/
```

## 自动发现

启动后会自动扫描本机已有配置，只展示当前实际启用的 Codex 上游来源；不会把历史供应商列表当成可切换管理面板。

- ccswitch: `%APPDATA%\cc-switch\cc-switch.db`、`~/.cc-switch/cc-switch.db`
- Codex++: `~/.codex-session-delete/settings.json` 及常见 AppData/config 目录
- Codex 桌面端 / CLI: `$CODEX_HOME/config.toml`、`$CODEX_HOME/auth.json`；未设置 `CODEX_HOME` 时使用 `~/.codex`。

支持默认 OpenAI / ChatGPT 登录、当前配置 profile、自定义供应商的 `env_key` 和本地模型。系统凭据库、临时登录态及外部认证命令会标为待确认，不会因无法读取 API Key 而停止本地审计。上游选择优先以 Codex 自身配置为准。

## Codex 版本兼容

`0.2.3` 已按 Codex CLI `0.153.4` 和 Windows 桌面端 `26.901.5280.0`（内置核心 `0.153.4`）适配。兼容旧版工具调用日志及新版分页会话的 `CommandExecution` / `FileChange` 记录，包括 Code Mode 内部执行的命令。

桌面端、CLI 和子会话同时运行时分别跟踪读取位置，记录中显示来源和核心版本。启动时加载最近 8 个会话各自末尾最多 1 MiB，随后跟踪所有会话的新增记录；支持旧会话恢复、日志截断和分段写入。

协议依据、验证命令和监控边界见 [Codex 兼容性维护说明](docs/CODEX_COMPATIBILITY.md)。

读取到的 API Key 只在 Rust 后端用于判断可用性，前端只显示脱敏结果。执行记录中的命令会在保存和展示前脱敏常见密钥、令牌、认证头和 URL 查询凭据。

## 当前能力

- 本地审计优先的桌面窗口：左侧按活动类型筛选，右侧展示当前上游与执行记录。
- 自动发现并只展示当前实际启用的 ccswitch / Codex++ / Codex 配置来源，不做逐个供应商的启用/禁用管理。
- 一键开启/关闭本地审计，同时跟踪 Codex 桌面端与 CLI 的本机会话，不依赖上游认证状态。
- 后台轻量常驻：支持托盘运行、静默自启动和单实例保护，避免重复进程占用。
- 执行记录展示层：命令、读取、新建、修改、删除、网络请求、高危事件分类齐全；当前自动采集主要通过读取本机 Codex 会话日志完成，活动数据也可通过 `record_command` / `record_file_event` 接口写入。
- 命令风险规则：识别密钥目录、环境文件、网络传输、删除命令等高危行为，并按等级标记告警。当前版本提供风险提示和事后记录，不承诺对任意外部进程做到执行前阻断。
- 应用管理：打开安装包页面、检查升级、打开安装目录、打开系统卸载入口。
- Tauri 打包：标准安装 / 卸载由系统安装器负责；便携版保持解压即用。

## 设计风格

项目的界面风格沉淀在 [DESIGN_STYLE.md](./DESIGN_STYLE.md)，后续扩展页面、组件或宣传物料时优先按这份规范保持一致。

## 后续路线

- 增加 CLI wrapper / shell shim，让通过可控入口启动的 Codex 或 shell 命令可以执行前检查并按策略阻断。
- 增加本地代理，让 Codex API 流量显式走本地代理，在模型响应 / 工具调用返回前做策略判断。
- 对 Codex App 做更深的进程 / 网络层监控；进程检测、可疑进程终止和响应阻断均属于后续路线，当前版本尚未实现。
- 引入持久化会话日志，展示清晰的告警时间线。

## 开源

MIT License。仓库地址：https://github.com/jliushi/codex-baoan
