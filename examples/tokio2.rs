use std::{thread, time::Duration};
use tokio::sync::mpsc::{self, Receiver};

//将阻塞代码移到同步线程中执行，避免阻塞异步运行时
//通过blocking_recv 阻塞同步线程，等待异步tx传递信息
//随后创建同步channel，新建线程执行会阻塞的任务，通过同步channel，将结果发送给同步channel receiver

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let (tx, rx) = mpsc::channel(32);
    let handle = worker(rx);

    tokio::spawn(async move {
        let mut i = 0;
        loop {
            i += 1;
            println!("sending task {}", i);
            tx.send(format!("task {i}")).await.unwrap();
        }
    });

    handle.join().unwrap();
    Ok(())
}

fn worker(mut rx: Receiver<String>) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let (sender, receiver) = std::sync::mpsc::channel();
        while let Some(v) = rx.blocking_recv() {
            let sender_clone = sender.clone();
            thread::spawn(move || {
                let ret = expensive_blocking_task(v);
                sender_clone.send(ret).unwrap();
            });
            let result = receiver.recv().unwrap();
            println!("result: {}", result);
        }
    })
}

fn expensive_blocking_task(str: String) -> String {
    std::thread::sleep(Duration::from_millis(300));
    blake3::hash(str.as_bytes()).to_string()
}
