use std::io::{Error, ErrorKind, Result};
use std::os::unix::process::ExitStatusExt;
use std::process::ExitStatus;

use libc::pid_t;
use tokio::signal::unix::{signal, SignalKind};

#[derive(Debug)]
pub struct Child {
    pid: pid_t,
    status: Option<ExitStatus>,
}

#[derive(Debug)]
pub enum ForkResult {
    Parent(Child),
    Child,
}

macro_rules! cvt {
    ($e:expr) => {
        match unsafe { $e } {
            -1 => Err(std::io::Error::last_os_error()),
            x => Ok(x),
        }
    };
}

/// # Safety
///
/// It is strongly recommended to fork before creating the tokio runtime.
///
#[allow(unused_unsafe)]
pub unsafe fn fork() -> Result<ForkResult> {
    match cvt!(libc::fork())? {
        0 => Ok(ForkResult::Child),
        pid => Ok(ForkResult::Parent(Child { pid, status: None })),
    }
}

impl Child {
    pub fn pid(&self) -> pid_t {
        self.pid
    }

    pub fn kill(&mut self) -> Result<()> {
        if self.status.is_some() {
            Err(Error::new(
                ErrorKind::InvalidInput,
                "invalid argument: can't kill an exited process",
            ))
        } else {
            cvt!(libc::kill(self.pid, libc::SIGKILL)).map(drop)
        }
    }

    pub async fn wait(&mut self) -> Result<ExitStatus> {
        if let Some(status) = self.try_wait()? {
            return Ok(status);
        }

        let mut sigchld = signal(SignalKind::child())?;
        loop {
            sigchld.recv().await;
            if let Some(x) = self.try_wait()? {
                break Ok(x);
            }
        }
    }

    pub fn block(&mut self) -> Result<ExitStatus> {
        if let Some(status) = self.status {
            return Ok(status);
        }

        let mut status = 0;
        loop {
            match cvt!(libc::waitpid(self.pid, &mut status, 0)) {
                Err(ref e) if e.kind() == ErrorKind::Interrupted => {}
                x => break x,
            }
        }?;
        self.status = Some(ExitStatus::from_raw(status));
        Ok(ExitStatus::from_raw(status))
    }

    pub fn try_wait(&mut self) -> Result<Option<ExitStatus>> {
        if let Some(status) = self.status {
            return Ok(Some(status));
        }

        let mut status = 0;
        let pid = cvt!(libc::waitpid(self.pid, &mut status, libc::WNOHANG))?;
        if pid == 0 {
            Ok(None)
        } else {
            self.status = Some(ExitStatus::from_raw(status));
            Ok(Some(ExitStatus::from_raw(status)))
        }
    }
}
