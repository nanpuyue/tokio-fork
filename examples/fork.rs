use std::io::Result;
use std::process::exit;
use std::thread::sleep;
use std::time::Duration;

use tokio::runtime::Builder;
use tokio_fork::{fork, Fork};

fn main() -> Result<()> {
    match unsafe { fork()? } {
        Fork::Parent(mut child) => {
            // build the runtime with enable_io()
            let rt = Builder::new_current_thread().enable_io().build()?;

            rt.block_on(async {
                let code = child.wait().await?.code().unwrap();
                println!(
                    "This is the parent process, I see the child process (pid: {}) exit with code {code}.",
                    child.pid()
                );
                Ok(())
            })
        }
        Fork::Child => {
            println!("This is the child process, I will exit with code 1 in 3s.");
            sleep(Duration::from_secs(3));
            exit(1)
        }
    }
}
