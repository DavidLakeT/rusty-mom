use async_broadcast::broadcast;
use futures_lite::future::block_on;
use std::{thread, time};

fn main() {
    let one_sec = time::Duration::from_millis(1000);

    let (s, r) = broadcast::<String>(100);

    let mut r1 = r.clone();
    let mut r2 = r.clone();

    // Sender
    thread::spawn(move || {
        let mut i = 0;
        loop {
            s.try_broadcast(format!("S1 = {i}")).unwrap();
            thread::sleep(one_sec);
            i += 1;
        }
    });

    thread::spawn(move || {
        block_on(async {
            thread::sleep(one_sec);
            loop {
                let msg = r1.recv().await.unwrap();
                println!("R1 = {msg}");
            }
        });
    });

    thread::spawn(move || {
        block_on(async {
            thread::sleep(one_sec);
            loop {
                let msg = r2.recv().await.unwrap();
                println!("R2 = {msg}");
            }
        });
    });

    loop {
        thread::sleep(one_sec);
    }
}
