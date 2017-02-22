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

extern crate habitat_launcher_client as launcher_client;
extern crate habitat_launcher_protocol as launcher_protocol;
extern crate ipc_channel;
extern crate protobuf;

pub mod error;

use std::env;
use std::thread;
use std::time;

use launcher_client::LauncherCli;
use launcher_protocol::LAUNCHER_PIPE_ENV;

fn main() {
    let client = match env::var(LAUNCHER_PIPE_ENV) {
        Ok(pipe) => LauncherCli::connect(pipe).unwrap(),
        _ => panic!("MUST START FROM LAUNCHER"),
    };
    let wait_time = time::Duration::from_millis(100);
    loop {
        client.spawn("core/builder-router").unwrap();
        thread::sleep(wait_time);
    }
}
