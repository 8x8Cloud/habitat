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

use libc;

pub struct Child {
    pid: Pid,
    last_status: Option<i32>,
}

impl Child {
    pub fn new(child: &mut process::Child) -> Result<Child> {
        Ok(Child {
               pid: child.id() as Pid,
               last_status: None,
           })
    }

    pub fn id(&self) -> Pid {
        self.pid
    }

    pub fn status(&mut self) -> Result<HabExitStatus> {
        match self.last_status {
            Some(status) => Ok(HabExitStatus { status: Some(status as u32) }),
            None => {
                let mut exit_status: i32 = 0;

                match unsafe { libc::waitpid(self.pid as i32, &mut exit_status, libc::WNOHANG) } {
                    0 => Ok(HabExitStatus { status: None }),
                    -1 => {
                        Err(Error::WaitpidFailed(format!("Error calling waitpid on pid: {}",
                                                         self.pid)))
                    }
                    _ => {
                        self.last_status = Some(exit_status);
                        Ok(HabExitStatus { status: Some(exit_status as u32) })
                    }
                }
            }
        }
    }

    pub fn kill(&mut self) -> Result<ShutdownMethod> {
        // check the group of the process being killed
        // if it is the root process of the process group
        // we send our signals to the entire process group
        // to prevent orphaned processes.
        let pgid = unsafe { libc::getpgid(self.pid) };
        if self.pid == pgid {
            debug!("pid to kill {} is the process group root. Sending signal to process group.",
                   self.pid);
            // sending a signal to the negative pid sends it to the
            // entire process group instead just the single pid
            self.pid = self.pid.neg();
        }

        signal(self.pid, Signal::TERM)?;
        let stop_time = SteadyTime::now() + Duration::seconds(8);
        loop {
            if let Ok(status) = self.status() {
                if !status.no_status() {
                    break;
                }
            }
            if SteadyTime::now() > stop_time {
                signal(self.pid, Signal::KILL)?;
                return Ok(ShutdownMethod::Killed);
            }
        }
        Ok(ShutdownMethod::GracefulTermination)
    }
}

impl ExitStatusExt for HabExitStatus {
    fn code(&self) -> Option<u32> {
        unsafe {
            match self.status {
                None => None,
                Some(status) if libc::WIFEXITED(status as libc::c_int) => {
                    Some(libc::WEXITSTATUS(status as libc::c_int) as u32)
                }
                _ => None,
            }
        }
    }

    fn signal(&self) -> Option<u32> {
        unsafe {
            match self.status {
                None => None,
                Some(status) if !libc::WIFEXITED(status as libc::c_int) => {
                    Some(libc::WTERMSIG(status as libc::c_int) as u32)
                }
                _ => None,
            }
        }
    }
}
