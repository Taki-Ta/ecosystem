/*
写一个简单的Tcp Chat Server
    - client 连接：添加全局状态
        - 创建 peer
        - 通知所有小伙伴
    - client 断连：从全局状态删除
        - 通知所有小伙伴
    - client 发消息
        - 广播
*/

use dashmap::DashMap;
use futures::{stream::SplitStream, SinkExt, StreamExt};
use std::{fmt, net::SocketAddr, sync::Arc};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::mpsc,
};
use tokio_util::codec::{Framed, LinesCodec};
use tracing::{info, level_filters::LevelFilter, warn};
use tracing_subscriber::{fmt::Layer, layer::SubscriberExt, util::SubscriberInitExt, Layer as _};

const MAX_MESSAGES: usize = 128;

#[derive(Default, Debug)]
struct State {
    peers: DashMap<SocketAddr, mpsc::Sender<Arc<Message>>>,
}

#[derive(Debug, Clone)]
enum Message {
    UserJoined(String),
    UserLeft(String),
    Chat { sender: String, content: String },
}

#[derive(Debug)]
struct Peer {
    username: String,
    stream: SplitStream<Framed<TcpStream, LinesCodec>>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let layer = Layer::new().with_filter(LevelFilter::INFO);
    tracing_subscriber::registry().with(layer).init();

    let addr = "0.0.0.0:8082";
    let listener = TcpListener::bind(addr).await?;
    info!("Start chat server on {addr}");
    let state = Arc::new(State::default());
    loop {
        let (socket, addr) = listener.accept().await?;
        let state = state.clone();
        info!("Accepted connection from: {addr}");
        tokio::spawn(async move {
            if let Err(e) = handle_request(state, addr, socket).await {
                warn!("Can not handle client addr:{addr}:{e}");
            }
        });
    }
}

async fn handle_request(
    state: Arc<State>,
    addr: SocketAddr,
    stream: TcpStream,
) -> anyhow::Result<()> {
    let mut encoder = Framed::new(stream, LinesCodec::new());
    encoder.send("please enter your username:").await?;
    let res = match encoder.next().await {
        Some(Ok(username)) => username,
        Some(Err(e)) => return Err(e.into()),
        None => return Ok(()),
    };
    let mut peer = state.add(addr, res, encoder).await;
    //用户加入时广播
    let message = Arc::new(Message::user_joined(&peer.username));
    info!("{}", message);
    state.broadcast(addr, message).await;

    while let Some(line) = peer.stream.next().await {
        let line = match line {
            Ok(line) => line,
            Err(e) => {
                warn!("Failed to read line from {}: {}", addr, e);
                break;
            }
        };
        let message = Arc::new(Message::chat(&peer.username, line));

        state.broadcast(addr, message).await;
    }
    // when while loop exit, peer has left the chat or line reading failed
    // remove peer from state
    state.peers.remove(&addr);

    // notify others that a user has left
    let message = Arc::new(Message::user_left(&peer.username));
    info!("{}", message);

    state.broadcast(addr, message).await;
    Ok(())
}

impl State {
    async fn broadcast(&self, addr: SocketAddr, message: Arc<Message>) {
        for peer in self.peers.iter() {
            if peer.key() == &addr {
                continue;
            }
            if let Err(e) = peer.value().send(message.clone()).await {
                warn!("Fail to send message to {};{e}", peer.key());
                //发送失败，从state中移除掉
                self.peers.remove(peer.key());
            }
        }
    }

    async fn add(
        &self,
        addr: SocketAddr,
        username: String,
        stream: Framed<TcpStream, LinesCodec>,
    ) -> Peer {
        //创建channel，插入state
        let (tx, mut rx) = mpsc::channel(MAX_MESSAGES);
        self.peers.insert(addr, tx);

        let (mut stream_sender, stream_receiver) = stream.split();

        //创建异步task，从channel中接收消息，并通过stream转发
        tokio::spawn(async move {
            while let Some(message) = rx.recv().await {
                if let Err(e) = stream_sender.send(message.to_string()).await {
                    warn!("Fail to send message to {addr}:{e}");
                }
            }
        });
        Peer {
            username,
            stream: stream_receiver,
        }
    }
}

impl Message {
    fn user_joined(username: &str) -> Self {
        let content = format!("{} has joined the chat", username);
        Self::UserJoined(content)
    }

    fn user_left(username: &str) -> Self {
        let content = format!("{} has left the chat", username);
        Self::UserLeft(content)
    }

    fn chat(sender: impl Into<String>, content: impl Into<String>) -> Self {
        Self::Chat {
            sender: sender.into(),
            content: content.into(),
        }
    }
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UserJoined(content) => write!(f, "[{}]", content),
            Self::UserLeft(content) => write!(f, "[{} :(]", content),
            Self::Chat { sender, content } => write!(f, "{}: {}", sender, content),
        }
    }
}
