// Copyright (c) 2017 Chef Software Inc. and/or applicable contributors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::path::Path;
use std::process::{Command, Stdio};

use ipc_channel::ipc::{IpcOneShotServer, IpcSender};
use launcher_protocol::LAUNCHER_PIPE_ENV;
use launcher_protocol::message::launcher as protocol;
use protobuf::{self, parse_from_bytes, Message};

use error::{Error, Result};

type Receiver = IpcOneShotServer<Vec<u8>>;
type Sender = IpcSender<Vec<u8>>;

pub fn run<T>(binary: T)
    where T: AsRef<Path>
{
    let mut command = Command::new(binary.as_ref());
    let (ipc_srv, ipc_name) = Receiver::new().expect("IPC NEW");
    command.arg(&ipc_name);
    command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::piped())
        .env(LAUNCHER_PIPE_ENV, ipc_name)
        .spawn()
        .expect("FAIL TO SPAWN");
    let (rx, raw) = ipc_srv.accept().expect("accept fail");
    let msg = parse_from_bytes::<protocol::Envelope>(&raw).unwrap();
    println!("ON-CONNECT: {:?}", msg);
    let mut cmd = parse_from_bytes::<protocol::Register>(msg.get_payload()).unwrap();
    println!("CONNECTING TO: {:?}", cmd.get_pipe());
    let tx = IpcSender::connect(cmd.take_pipe())
        .map_err(|e| Error::Connect(e))
        .unwrap();
    let cmd = protocol::Ok::new();
    let mut msg = protocol::Envelope::new();
    msg.set_message_id("ok".to_string());
    msg.set_payload(cmd.write_to_bytes().unwrap());
    send(&tx, &msg).unwrap();

    // * Receive messages
    // * Start supervisor if it crashes
    loop {
        match rx.recv() {
            Ok(bytes) => dispatch(&tx, &bytes),
            Err(err) => println!("ERR!! {:?}", err),
        }
    }
}

pub fn send<T>(tx: &Sender, command: &T) -> Result<()>
    where T: protobuf::MessageStatic
{
    let bytes = command
        .write_to_bytes()
        .map_err(|e| Error::Serialize(e))?;
    tx.send(bytes).map_err(|e| Error::Send(e))?;
    Ok(())
}

fn dispatch(tx: &Sender, bytes: &[u8]) {
    let msg = parse_from_bytes::<protocol::Envelope>(&bytes).unwrap();
    match msg.get_message_id() {
        "Spawn" => {
            let msg = parse_from_bytes::<protocol::Spawn>(msg.get_payload()).unwrap();
            println!("MSG!!! {:?}", msg);
            send(tx, &protocol::Ok::new()).unwrap();
        }
        unknown => {
            println!("Received unknown message, {}", unknown);
        }
    }
}
