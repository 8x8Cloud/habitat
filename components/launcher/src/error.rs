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

use std::error;
use std::fmt;
use std::io;
use std::result;

use ipc_channel;
use protobuf;

#[derive(Debug)]
pub enum Error {
    Connect(io::Error),
    Send(ipc_channel::Error),
    Serialize(protobuf::ProtobufError),
}

pub type Result<T> = result::Result<T, Error>;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let msg = match *self {
            Error::Connect(ref e) => format!("Unable to connect to Launcher's pipe, {}", e),
            Error::Send(ref e) => format!("Unable to send to Launcher's pipe, {}", e),
            Error::Serialize(ref e) => format!("Unable to serialize message to Launcher, {}", e),
        };
        write!(f, "{}", msg)
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        match *self {
            Error::Connect(_) => "Unable to connect to Launcher's pipe",
            Error::Send(_) => "Unable to send to Launcher's pipe",
            Error::Serialize(_) => "Unable to serialize message to Launcher",
        }
    }
}
