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

use std::fmt;
use std::process::Child as StdChild;

use os::process as imp;

pub struct Child(imp::Child);

impl Child {
    pub fn from(raw: &mut StdChild) -> Result<Child> {
        match imp::Child::new(raw) {
            Ok(child) => Ok(Child(child)),
            Err(e) => Err(e),
        }
    }

    pub fn id(&self) -> Pid {
        self.0.id()
    }

    pub fn status(&mut self) -> Result<HabExitStatus> {
        self.0.status()
    }

    pub fn kill(&mut self) -> Result<ShutdownMethod> {
        self.0.kill()
    }
}

impl fmt::Debug for Child {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "pid: {}", self.id())
    }
}

pub struct HabExitStatus {
    status: Option<u32>,
}

impl HabExitStatus {
    pub fn no_status(&self) -> bool {
        self.status.is_none()
    }
}

pub trait ExitStatusExt {
    fn code(&self) -> Option<u32>;
    fn signal(&self) -> Option<u32>;
}
