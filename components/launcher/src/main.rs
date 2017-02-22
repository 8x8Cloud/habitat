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

extern crate habitat_core as core;
extern crate habitat_launcher as launcher;

use std::str::FromStr;
use std::path::PathBuf;
use std::process;

use core::package::{PackageIdent, PackageInstall};

use launcher::server;

const SUP_CMD: &'static str = "hab-sup";
const SUP_CMD_ENVVAR: &'static str = "HAB_SUP_BINARY";
const SUP_PACKAGE_IDENT: &'static str = "core/hab-sup";

fn main() {
    let cmd = match core::env::var(SUP_CMD_ENVVAR) {
        Ok(command) => PathBuf::from(command),
        Err(_) => supervisor_cmd(),
    };
    server::run(cmd)
}

fn supervisor_cmd() -> PathBuf {
    let ident = PackageIdent::from_str(SUP_PACKAGE_IDENT).unwrap();
    match PackageInstall::load_at_least(&ident, None) {
        Ok(install) => {
            match core::fs::find_command_in_pkg(SUP_CMD, &install, "/") {
                Ok(Some(cmd)) => cmd,
                _ => {
                    println!("Supervisor package didn't contain '{}' binary", SUP_CMD);
                    process::exit(3);
                }
            }
        }
        Err(_) => {
            println!("Unable to locate Supervisor package, {}", SUP_PACKAGE_IDENT);
            process::exit(2);
        }
    }
}
