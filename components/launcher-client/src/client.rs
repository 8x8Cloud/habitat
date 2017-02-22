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

use launcher_protocol::message::launcher as protocol;
use ipc_channel::ipc::{IpcOneShotServer, IpcReceiver, IpcSender};
use protobuf::{self, parse_from_bytes, Message};

use error::{Error, Result};

type IpcServer = IpcOneShotServer<Vec<u8>>;

pub struct LauncherCli {
    tx: IpcSender<Vec<u8>>,
    rx: IpcReceiver<Vec<u8>>,
}

impl LauncherCli {
    ////////////////////////
    // Public Func
    //

    pub fn connect(pipe: String) -> Result<Self> {
        let tx = IpcSender::connect(pipe).map_err(|e| Error::Connect(e))?;
        let (ipc_srv, pipe) = IpcServer::new().map_err(|e| Error::BadPipe(e))?;
        let mut cmd = protocol::Register::new();
        cmd.set_pipe(pipe);
        Self::send(&tx, &cmd)?;
        let (rx, raw) = ipc_srv.accept().expect("JW TODO: Accept error here");
        Self::read::<protocol::Ok>(&raw)?;
        Ok(LauncherCli { tx: tx, rx: rx })
    }

    ////////////////////////
    // Private Func
    //

    /// Read a launcher protocol message from a byte array
    fn read<T>(bytes: &[u8]) -> Result<T>
        where T: protobuf::MessageStatic
    {
        let envelope = parse_from_bytes::<protocol::Envelope>(bytes)
            .map_err(|e| Error::Deserialize(e))?;
        let msg = parse_from_bytes::<T>(envelope.get_payload())
            .map_err(|e| Error::Deserialize(e))?;
        Ok(msg)
    }

    /// Receive and read protocol message from an IpcReceiver
    fn recv<T>(rx: &IpcReceiver<Vec<u8>>) -> Result<T>
        where T: protobuf::MessageStatic
    {
        // JW TODO: I need to be able to specify a timeout for waiting on a response. If the
        // Launcher's pipe has gone away, we need to go away. This should *never* happen so maybe
        // not? We're only ever launched by the launcher itself.
        match rx.recv() {
            Ok(bytes) => Self::read(&bytes),
            Err(err) => {
                unreachable!("FIX THIS");
                // Err(Error::Receive(*err))
            }
        }
    }

    /// Send a command to a Launcher
    fn send<T>(tx: &IpcSender<Vec<u8>>, command: &T) -> Result<()>
        where T: protobuf::MessageStatic
    {
        let mut msg = protocol::Envelope::new();
        let payload = command
            .write_to_bytes()
            .map_err(|e| Error::Serialize(e))?;
        msg.set_message_id(command.descriptor().name().to_string());
        msg.set_payload(payload);
        let bytes = msg.write_to_bytes().map_err(|e| Error::Serialize(e))?;
        tx.send(bytes).map_err(|e| Error::Send(e))?;
        Ok(())
    }

    ////////////////////////
    // Public Member Func
    //

    /// Send a process spawn command to the connected Launcher
    pub fn spawn<T>(&self, ident: T) -> Result<()>
        where T: Into<String>
    {
        let mut msg = protocol::Spawn::new();
        msg.set_ident(ident.into());
        Self::send(&self.tx, &msg)?;
        Self::recv::<protocol::Ok>(&self.rx)?;
        Ok(())
    }
}
