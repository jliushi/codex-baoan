package main

import (
	"fmt"

	"github.com/jchv/go-webview2"
)

// launchGUI opens a native WebView2 window pointing at the local dashboard.
// Runs on the calling (main) goroutine; blocks until the window closes.
func launchGUI(dashPort int) {
	w := webview2.NewWithOptions(webview2.WebViewOptions{
		Debug: false,
		WindowOptions: webview2.WindowOptions{
			Title:  "Codex 保安 · 模型审计",
			Width:  1040,
			Height: 720,
			IconId: 1,
			Center: true,
		},
	})
	if w == nil {
		fmt.Println("无法创建窗口：缺少 WebView2 运行时。请安装 Microsoft Edge WebView2 Runtime。")
		fmt.Printf("看板地址：http://127.0.0.1:%d\n", dashPort)
		// Do not leave CC Switch pointing at a process with no visible way to
		// close it. main's deferred cleanup restores a previous attachment.
		return
	}
	defer w.Destroy()
	w.Navigate(fmt.Sprintf("http://127.0.0.1:%d", dashPort))
	w.Run()
}
