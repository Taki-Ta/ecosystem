use anyhow::Result;
use core::fmt;
use dashmap::DashMap;
use futures::{stream::SplitStream, SinkExt, StreamExt};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{self, Sender};
use tokio_util::codec::{Framed, LinesCodec};
use tracing::{info, level_filters::LevelFilter, warn};
use tracing_subscriber::{fmt::Layer, layer::SubscriberExt, util::SubscriberInitExt, Layer as _};

const MAX_MESSAGES: usize = 128;
type Reveiver = SplitStream<Framed<TcpStream, LinesCodec>>;

#[derive(Debug, Default, Clone)]
struct AppState {
    inner: DashMap<SocketAddr, Sender<String>>,
}

#[derive(Debug)]
enum Message {
    Quit(String),
    Join(String),
    Chat { username: String, content: String },
}

#[tokio::main]
async fn main() -> Result<()> {
    let layer = Layer::new().pretty().with_filter(LevelFilter::INFO);
    tracing_subscriber::registry().with(layer).init();

    let addr = "0.0.0.0:8082";
    let listener = TcpListener::bind(addr).await?;
    info!("listening on {addr}");
    let state = Arc::new(AppState::default());
    loop {
        let state = state.clone();
        let (socket, addr) = listener.accept().await?;
        tokio::spawn(async move {
            if let Err(e) = handle_request(state, socket, addr).await {
                warn!("can not handle requert:{e}");
            }
        });
    }
}

async fn handle_request(state: Arc<AppState>, socket: TcpStream, addr: SocketAddr) -> Result<()> {
    let mut framed = Framed::new(socket, LinesCodec::new());
    framed.send("enter you username:").await?;

    let username = match framed.next().await {
        Some(Ok(username)) => username,
        Some(Err(e)) => return Err(e.into()),
        None => return Ok(()),
    };

    info!("{} has joined the chat room", &username);
    let msg = Arc::new(Message::user_joined(&username));
    state.broadcast(&addr, msg).await;

    let mut receiver = state.add_user(addr, framed);

    while let Some(Ok(line)) = receiver.next().await {
        let username = username.clone();
        let msg = Arc::new(Message::Chat {
            username,
            content: line,
        });
        state.broadcast(&addr, msg).await;
    }

    state.inner.remove(&addr);
    info!("{} has quited the chat room", &username);
    let msg = Arc::new(Message::user_quited(&username));
    state.broadcast(&addr, msg).await;

    Ok(())
}

impl AppState {
    async fn broadcast(&self, addr: &SocketAddr, msg: Arc<Message>) {
        for item in self.inner.iter() {
            if item.key() != addr {
                if let Err(e) = item.value().send(msg.to_string()).await {
                    warn!("can not send message to {addr}:{e}");
                    self.inner.remove(addr);
                }
            }
        }
    }

    fn add_user(&self, addr: SocketAddr, socket: Framed<TcpStream, LinesCodec>) -> Reveiver {
        let (tx, mut rx) = mpsc::channel(MAX_MESSAGES);
        self.inner.insert(addr, tx);

        let (mut sender, receiver) = socket.split();
        tokio::spawn(async move {
            // let username=username.clone();
            while let Some(line) = rx.recv().await {
                // let msg=Arc::new(Message::Chat { username:username.clone(), content: line });
                if let Err(e) = sender.send(line).await {
                    warn!("can not send meaasge:{e}");
                }
            }
        });

        receiver
    }
}

impl Message {
    fn user_joined(username: &str) -> Message {
        Message::Join(username.to_string())
    }
    fn user_quited(username: &str) -> Message {
        Message::Quit(username.to_string())
    }
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Message::Quit(msg) => write!(f, "{} has quited the chat room :(", msg),
            Message::Join(msg) => write!(f, "{} has joined the chat room :)", msg),
            Message::Chat { username, content } => write!(f, "{}:{}", username, content),
        }
    }
}
