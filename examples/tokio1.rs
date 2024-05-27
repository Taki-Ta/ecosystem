use std::{thread, time::Duration};
use tokio::{fs, runtime::Builder, time::sleep};

fn main() {
    let handler = thread::spawn(|| {
        let rt = Builder::new_current_thread().enable_all().build().unwrap();

        rt.spawn(async {
            println!("task1");
            let content = fs::read("Cargo.toml").await.unwrap();
            println!("content length: {}", content.len());
        });

        rt.spawn(async {
            println!("task2");
            let result = expensive_blocking_task("hello".to_string());
            println!("result: {}", result);
        });

        rt.block_on(async { sleep(Duration::from_secs(1)).await })
    });

    handler.join().unwrap();
}

fn expensive_blocking_task(str: String) -> String {
    std::thread::sleep(Duration::from_millis(300));
    blake3::hash(str.as_bytes()).to_string()
}
