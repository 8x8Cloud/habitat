extern crate ipc_channel;

use std::env;
use std::thread;
use std::time;

use ipc_channel::ipc::IpcSender;

const LAUNCHER_PIPE_ENV: &'static str = "HAB_LAUNCHER_PIPE";

fn main() {
    let pipe = match env::var(LAUNCHER_PIPE_ENV) {
        Ok(pipe) => pipe,
        _ => panic!("MUST START FROM LAUNCHER"),
    };
    let wait_time = time::Duration::from_millis(100);
    println!("CONNECTING TO {:?}", pipe);
    let tx: IpcSender<String> = IpcSender::connect(pipe).expect("NO IPC CONNECT");
    println!("entering loop");
    loop {
        tx.send("message-from-sup".to_string())
            .expect("NO MSG SEND");
        thread::sleep(wait_time);
    }
}
