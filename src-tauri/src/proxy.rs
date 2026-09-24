//! 在后台线程 + tokio 运行时里跑 hudsucker MITM 代理。
use crate::capture::CaptureHandler;
use crate::store::Store;
use hudsucker::rustls::crypto::aws_lc_rs;
use hudsucker::Proxy;
use std::net::SocketAddr;
use std::sync::Arc;

async fn build_and_start(
    port: u16,
    store: Arc<Store>,
    shutdown: impl std::future::Future<Output = ()> + Send + 'static,
) -> Result<(), String> {
    let ca = crate::ca::build_authority()?;
    let proxy = Proxy::builder()
        .with_addr(SocketAddr::from(([127, 0, 0, 1], port)))
        .with_ca(ca)
        .with_rustls_connector(aws_lc_rs::default_provider())
        .with_http_handler(CaptureHandler::new(store))
        .with_graceful_shutdown(shutdown)
        .build()
        .map_err(|e| e.to_string())?;
    proxy.start().await.map_err(|e| e.to_string())
}

/// 启动代理线程，返回用于优雅关闭的发送端。
pub fn spawn(port: u16, store: Arc<Store>) -> tokio::sync::oneshot::Sender<()> {
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    std::thread::spawn(move || {
        let rt = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
            Ok(rt) => rt,
            Err(e) => {
                eprintln!("tokio runtime 构建失败：{e}");
                return;
            }
        };
        rt.block_on(async move {
            let shutdown = async move {
                let _ = rx.await;
            };
            if let Err(e) = build_and_start(port, store, shutdown).await {
                eprintln!("MITM 代理退出：{e}");
            }
        });
    });
    tx
}
