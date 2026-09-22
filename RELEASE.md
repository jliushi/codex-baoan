# 发布指南

本文档说明如何发布带签名应用内更新的 Windows 版本。

## GitHub Secrets

仓库 Actions Secrets 必须配置：

- `TAURI_SIGNING_PRIVATE_KEY`: 与 `src-tauri/tauri.conf.json` 中 `plugins.updater.pubkey` 配对的 Tauri updater 私钥
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`: 私钥密码；无密码时留空

私钥文件不要提交到 Git。公钥保存在 `tauri.conf.json`，用于运行时校验 Release 里的 `.sig`。

## 本地构建

```bash
pnpm install
pnpm run typecheck
pnpm run package:windows:unsigned
```

本地产物在：

```text
src-tauri/target/release/bundle/
```

未配置签名私钥时使用 unsigned 脚本验证安装包生成。正式签名构建使用 `pnpm run package:windows`，需要设置 `TAURI_SIGNING_PRIVATE_KEY`，否则 Tauri 无法生成 `.sig`。

## 发布步骤

1. 更新版本号：
   - `package.json`
   - `src-tauri/Cargo.toml`
   - `src-tauri/tauri.conf.json`

2. 提交并推送：

```bash
git add .
git commit -m "chore: bump version to 0.X.X"
git push
```

3. 创建并推送 tag：

```bash
git tag v0.X.X
git push origin v0.X.X
```

4. GitHub Actions 的 `Release` workflow 会生成并上传：
   - `Codex-Baoan-v0.X.X-Windows.msi`
   - `Codex-Baoan-v0.X.X-Windows.msi.sig`
   - `Codex-Baoan-v0.X.X-Windows-Setup.exe`
   - `Codex-Baoan-v0.X.X-Windows-Portable.zip`
   - `latest.json`

MSI 是推荐安装包，也是应用内更新使用的包。Portable 便携版会带 `portable.ini`，应用内更新会自动禁用，用户需要手动下载新版。

## 验证更新

1. 安装旧版 MSI。
2. 发布更高版本 tag。
3. 打开旧版应用，进入「设置」。
4. 「版本更新」区域应自动检查一次，也可以手动点「检查更新」。
5. 有新版本时点击「立即更新」，下载完成后应用会重启到新版本。

## 常见问题

- 检查更新失败：确认 Release 资产里存在 `latest.json`，并且 URL 为 `https://github.com/jliushi/codex-baoan/releases/latest/download/latest.json`。
- 安装更新失败：确认 `.msi.sig` 已上传，且私钥与 `tauri.conf.json` 的公钥匹配。
- 开机自启失败：应用会直接在设置页报错，不会静默吞掉失败；优先检查 Windows 安全软件或注册表写入限制。
