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

pub struct Child {
    handle: Option<winapi::HANDLE>,
    last_status: Option<u32>,
    pid: u32,
}

impl Child {
    // On windows we need the process handle to capture status
    // Here we will attempt to get the handle from the pid but if the
    // process dies before we can get it, we will just wait() on the
    // std::process::Child and cache the exit_status which we will return
    // when status is called.
    pub fn new(child: &mut process::Child) -> Result<Child> {
        let (win_handle, status) = match handle_from_pid(child.id()) {
            Some(handle) => (Some(handle), Ok(None)),
            _ => {
                (None,
                 {
                     match child.wait() {
                         Ok(exit) => Ok(Some(exit.code().unwrap() as u32)),
                         Err(e) => {
                             Err(format!("Failed to retrieve exit code for pid {} : {}",
                                         child.id(),
                                         e))
                         }
                     }
                 })
            }
        };

        match status {
            Ok(status) => {
                Ok(Child {
                       handle: win_handle,
                       last_status: status,
                       pid: child.id(),
                   })
            }
            Err(e) => Err(Error::GetHabChildFailed(e)),
        }
    }

    pub fn id(&self) -> u32 {
        self.pid
    }

    pub fn status(&mut self) -> Result<HabExitStatus> {
        if self.last_status.is_some() {
            return Ok(HabExitStatus { status: Some(self.last_status.unwrap()) });
        }

        let exit_status = exit_status(self.handle.unwrap())?;

        if exit_status == STILL_ACTIVE {
            return Ok(HabExitStatus { status: None });
        };

        Ok(HabExitStatus { status: Some(exit_status) })
    }

    pub fn kill(&mut self) -> Result<ShutdownMethod> {
        if self.last_status.is_some() {
            return Ok(ShutdownMethod::AlreadyExited);
        }

        let mut ret;
        unsafe {
            // Turn off ctrl-C handling for current process
            ret = kernel32::SetConsoleCtrlHandler(None, winapi::TRUE);
            if ret == 0 {
                debug!("Failed to call SetConsoleCtrlHandler on pid {}: {}",
                       self.pid,
                       io::Error::last_os_error());
            }

            if ret != 0 {
                // Send a ctrl-C
                ret = kernel32::GenerateConsoleCtrlEvent(0, 0);
                if ret == 0 {
                    debug!("Failed to send ctrl-c to pid {}: {}",
                           self.pid,
                           io::Error::last_os_error());
                }
            }
        }

        let stop_time = SteadyTime::now() + Duration::seconds(8);

        let result;
        loop {
            if ret == 0 || SteadyTime::now() > stop_time {
                unsafe {
                    ret = kernel32::TerminateProcess(self.handle.unwrap(), 1);
                    if ret == 0 {
                        result = Err(Error::TerminateProcessFailed(format!("Failed to call \
                                                                       terminate pid {}: {}",
                                                                      self.pid,
                                                                      io::Error::last_os_error())));
                    } else {
                        result = Ok(ShutdownMethod::Killed);
                    }
                    break;
                }
            }

            match self.status() {
                Ok(status) => {
                    if !status.no_status() {
                        result = Ok(ShutdownMethod::GracefulTermination);
                        break;
                    }
                }
                _ => {}
            }
        }

        // turn Ctrl-C handling back on for current process
        ret = unsafe { kernel32::SetConsoleCtrlHandler(None, winapi::FALSE) };
        if ret == 0 {
            debug!("Failed to call SetConsoleCtrlHandler on pid {}: {}",
                   self.pid,
                   io::Error::last_os_error());
        }

        result
    }
}

// Have to implement these due to our HANDLE field
unsafe impl Send for Child {}
unsafe impl Sync for Child {}

impl Drop for Child {
    fn drop(&mut self) {
        match self.handle {
            None => {}
            Some(handle) => unsafe {
                let _ = kernel32::CloseHandle(handle);
            },
        }
    }
}

impl ExitStatusExt for HabExitStatus {
    fn code(&self) -> Option<u32> {
        self.status
    }

    fn signal(&self) -> Option<u32> {
        None
    }
}
