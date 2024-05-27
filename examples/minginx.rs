use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::{
    io,
    net::{TcpListener, TcpStream},
};
use tracing::{info, level_filters::LevelFilter, warn};
use tracing_subscriber::{fmt::Layer, layer::SubscriberExt, util::SubscriberInitExt, Layer as _};

#[derive(Serialize, Deserialize, Clone)]
struct Config {
    upstream_addr: String,
    listen_addr: String,
}

fn resolve_config() -> Config {
    Config {
        upstream_addr: "0.0.0.0:8080".to_string(),
        listen_addr: "0.0.0.0:8081".to_string(),
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // tracing_subscriber::fmt::init();
    let layer = Layer::new().with_filter(LevelFilter::INFO);
    //这里的类型推断很有意思
    //registry返回一个Registry实例，with的 trait bound显示需要接收一个Layer<type of self>
    //所以推断出了layer的类型是Layer<Registry>
    tracing_subscriber::registry().with(layer).init();

    let config = resolve_config();
    let config = Arc::new(config);

    info!("Upstream is {}", config.upstream_addr);
    info!("Listening on {}", config.listen_addr);

    let listener = TcpListener::bind(&config.listen_addr).await?;
    loop {
        //接收客户端请求
        let (client, addr) = listener.accept().await?;
        info!("Accepted connection from {}", addr);
        let clone_config = Arc::clone(&config);

        tokio::spawn(async move {
            //连接上游服务器
            let upstream = TcpStream::connect(&clone_config.upstream_addr).await?;
            //转发客户端请求到目标服务器
            proxy(client, upstream).await?;
            Ok::<(), anyhow::Error>(())
        });
    }

    #[allow(unreachable_code)]
    Ok::<(), anyhow::Error>(())
}

async fn proxy(mut client: TcpStream, mut upstream: TcpStream) -> anyhow::Result<()> {
    let (mut client_read, mut client_write) = client.split();
    let (mut upstream_read, mut upstream_write) = upstream.split();
    //将收到的客户端请求 发送到上游服务器中
    let client_to_upstream = io::copy(&mut client_read, &mut upstream_write);
    //将上游服务器的响应发送回客户端
    let upstream_to_client = io::copy(&mut upstream_read, &mut client_write);
    // 同时执行两个方向的数据传输
    match tokio::try_join!(client_to_upstream, upstream_to_client) {
        Ok((n, m)) => info!(
            "proxied {} bytes from client to upstream, {} bytes from upstream to client",
            n, m
        ),
        Err(e) => warn!("error proxying: {:?}", e),
    }
    Ok(())
}
