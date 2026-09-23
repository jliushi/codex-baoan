# Codex 保安

一个**可直接运行的原生 Windows exe**（Go 单文件，无 Python、无 cgo、无运行时依赖，只需系统自带的 WebView2 运行时），作为 CC Switch 的可选本地观察层，读一天的日志：**总请求 / 异常请求 / 异常请求实际是什么模型**。

## 它解决什么

CC Switch 的日志和 Codex 的会话日志都拿不到「请求正文 ↔ 上游上报 token」的干净配对，所以无法识破「连模型名都改干净」的同名偷换。本工具在 **CC Switch 的出口处**做一次本地 MITM（用本机信任的自签根证书解密），拿到发给上游的确切请求体 + 上游自报的模型与用量：

- **路由/替换**：请求 A、上游自报 B —— 自报名即实际模型（诚实中转）。
- **分词器指纹**：不同模型家族分词器不同（o200k / cl100k / 疑似非 GPT），上游为计费必须上报 `input_tokens`，用差分回归判它到底是哪一家 —— **识破改名**。家族级判断，非权重认证；拟合不出就报「无法判定/疑似非 GPT」，绝不瞎猜。

链路：`Codex → CC Switch → 本工具(MITM) → 你的出口代理(如 Clash 7890) → 上游`。出口仍走你原来的代理，翻墙/路由不变。

## 用法（方便快捷）

1. 从 [Releases](https://github.com/jliushi/codex-baoan/releases/latest) 下载 `Codex-Baoan-*-Windows.exe`，双击打开。
2. 点「**一键信任证书**」（装入当前用户信任库，不需要管理员）。
3. 点「**一键接入 CC Switch**」，然后**重启一次 CC Switch**。
4. 新开的 Codex 会话即被观察，日报与指纹自动出。顶部「观察中」表示有流量流经。

「断开」按钮恢复 CC Switch 原出口设置。接入属于当前保安会话：关闭窗口时会自动恢复原出口；如果系统异常退出，下次启动会在确认本地代理已停止后自动清理遗留设置。全程不改 CC Switch 代码、不改 Codex 配置。

## 边界（如实说明）

- 分词器指纹只到**家族**，不精确到快照；同族偷换看不出。
- 「实际模型」= 上游自报名 或 分词器家族推断，非权重身份认证；自报名可被改写。
- MITM 需信任本机自签根证书（可一键卸载）。仅解密经本工具的流量。
- 只覆盖经 CC Switch 出口的流量。
- 接入会写入 CC Switch 的全局出口配置，并要求重启 CC Switch；运行期间请勿删除本工具的数据目录，否则无法恢复已保存的原出口。

## 命令行

```
guard            # 打开图形界面（默认）
guard install    # 装证书并打开界面
guard run        # 无界面后台运行
guard uninstall  # 移除本机根证书
guard status     # 打印证书/端口状态
```

## 构建

```
set CGO_ENABLED=0
go build -ldflags="-H windowsgui -s -w" -o codex-baoan.exe .
```

纯 Go，无需 C 编译器。第三方：goproxy(MIT)、tiktoken-go(MIT)、go-webview2(MIT)、modernc.org/sqlite(BSD-3)。
