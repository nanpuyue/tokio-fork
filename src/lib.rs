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
pub enum Fork {
    Parent(Child),
    Child,
}

macro_rules! cvt {
    ($e:expr) => {
        match $e {
            -1 => Err(Error::last_os_error()),
            x => Ok(x),
        }
    };
}

/// # Safety
///
/// It is strongly recommended to fork before creating the tokio runtime.
///
pub unsafe fn fork() -> Result<Fork> {
    match cvt!(libc::fork())? {
        0 => Ok(Fork::Child),
        pid => Ok(Fork::Parent(Child { pid, status: None })),
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
            cvt!(unsafe { libc::kill(self.pid, libc::SIGKILL) }).map(drop)
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
            match cvt!(unsafe { libc::waitpid(self.pid, &mut status, 0) }) {
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
        let pid = cvt!(unsafe { libc::waitpid(self.pid, &mut status, libc::WNOHANG) })?;
        if pid == 0 {
            Ok(None)
        } else {
            self.status = Some(ExitStatus::from_raw(status));
            Ok(Some(ExitStatus::from_raw(status)))
        }
    }
}
