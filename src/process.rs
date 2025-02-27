use crate::Config;
use failure::{bail, format_err, Fallible};
use log::{debug, error, info};
use nix::{
    sys::signal::{kill, Signal},
    unistd::Pid,
};
use std::{
    fs::{self, create_dir_all, metadata, set_permissions, File},
    io::{BufRead, BufReader},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc::{channel, Sender},
    thread::{spawn, JoinHandle},
    time::Instant,
};

/// A general process abstraction
pub struct Process {
    command: String,
    kill: Sender<()>,
    log_file: PathBuf,
    pid: u32,
    watch: Option<JoinHandle<Fallible<()>>>,
    readyness_timeout: u64,
}

/// The trait to stop something
pub trait Stoppable {
    /// Stop the process
    fn stop(&mut self) -> Fallible<()>;
}

/// Startable process type
pub type Startable = Box<dyn Stoppable + Send>;

impl Process {
    /// Creates a new `Process` instance by spawning the provided command `cmd`.
    /// If the process creation fails, an `Error` will be returned.
    pub fn start(
        config: &Config,
        dir: &Path,
        command: &'static str,
        args: &[&str],
    ) -> Fallible<Process> {
        // Prepare the commands
        if command.is_empty() {
            bail!("No valid command provided")
        }

        // Prepare the log dir and file
        let log_dir = config.root().join("log");
        create_dir_all(&log_dir)?;
        let mut log_file = log_dir.join(command);
        log_file.set_extension("log");

        let out_file = File::create(&log_file)?;
        let err_file = out_file.try_clone()?;

        // Spawn the process child
        let mut child = Command::new(command)
            .args(args)
            .stderr(Stdio::from(err_file))
            .stdout(Stdio::from(out_file))
            .spawn()?;

        let (kill_tx, kill_rx) = channel();
        let c = command.to_owned();
        let pid = child.id();
        let watch = spawn(move || {
            // Wait for the process to exit
            let status = child.wait()?;

            // No kill send, we assume that the process died
            if kill_rx.try_recv().is_err() {
                error!("Process '{}' died unexpectedly", c);
            } else {
                info!("Process '{}' exited", c);
            }
            debug!("{} {}", c, status);
            Ok(())
        });

        // Write the executed command into the dir
        create_dir_all(dir)?;
        let run_file = dir.join("run.sh");
        let sep = format!(" \\\n{}", " ".repeat(4));
        let full_command = format!(r#"{}{}{}"#, command, sep, args.join(&sep));
        fs::write(
            &run_file,
            format!(include_str!("assets/run.sh"), full_command),
        )
        .map_err(|e| format_err!("Unable to create '{}': {}", run_file.display(), e))?;
        let mut perms = metadata(&run_file)?.permissions();
        perms.set_mode(0o755);
        set_permissions(run_file, perms)?;

        Ok(Process {
            command: command.to_owned(),
            kill: kill_tx,
            log_file,
            pid,
            watch: Some(watch),
            readyness_timeout: 30,
        })
    }

    // Wait for the process to become ready, by searching for the pattern in
    // every line of its output.
    pub fn wait_ready(&mut self, pattern: &str) -> Fallible<()> {
        debug!(
            "Waiting for process '{}' to become ready with pattern: '{}'",
            self.command, pattern
        );
        let now = Instant::now();
        let file = File::open(&self.log_file)?;
        let mut reader = BufReader::new(file);

        while now.elapsed().as_secs() < self.readyness_timeout {
            let mut line = String::new();
            reader.read_line(&mut line)?;

            if line.contains(pattern) {
                debug!("Found pattern '{}' in line '{}'", pattern, line.trim());
                return Ok(());
            }
        }

        // Cleanup since process is not ready
        self.stop()?;
        bail!("Timed out waiting for process to become ready")
    }

    /// Retrieve a pseudo state for stopped processes
    pub fn stopped() -> Fallible<Startable> {
        Err(format_err!("Stopped"))
    }
}

impl Stoppable for Process {
    /// Stopping the process by killing it
    fn stop(&mut self) -> Fallible<()> {
        debug!("Stopping process '{}'", self.command);

        // Indicate that this shutdown is intended
        self.kill
            .send(())
            .map_err(|e| format_err!("Unable to send kill signal to process: {}", e))?;

        // Send SIGTERM to the process
        kill(Pid::from_raw(self.pid as i32), Signal::SIGTERM)?;

        // Join the waiting thread
        if let Some(handle) = self.watch.take() {
            if handle.join().is_err() {
                bail!("Unable to stop process '{}'", self.command);
            }
        }
        debug!("Process '{}' stopped", self.command);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::tests::{test_config, test_config_wrong_root};
    use tempfile::tempdir;

    #[test]
    fn stopped() {
        assert!(Process::stopped().is_err())
    }

    #[test]
    fn start_success() -> Fallible<()> {
        let c = test_config()?;
        let d = tempdir()?;
        Process::start(&c, d.path(), "echo", &[])?;
        Ok(())
    }

    #[test]
    fn start_failure_wrong_root() -> Fallible<()> {
        let c = test_config_wrong_root()?;
        let d = tempdir()?;
        assert!(Process::start(&c, d.path(), "echo", &[]).is_err());
        Ok(())
    }

    #[test]
    fn start_failure_no_command() -> Fallible<()> {
        let c = test_config()?;
        let d = tempdir()?;
        assert!(Process::start(&c, d.path(), "", &[]).is_err());
        Ok(())
    }

    #[test]
    fn start_failure_invalid_command() -> Fallible<()> {
        let c = test_config()?;
        let d = tempdir()?;
        assert!(Process::start(&c, d.path(), "invalid_command", &[]).is_err());
        Ok(())
    }

    #[test]
    fn wait_ready_success() -> Fallible<()> {
        let c = test_config()?;
        let d = tempdir()?;
        let mut p = Process::start(&c, d.path(), "echo", &["test"])?;
        p.wait_ready("test")?;
        Ok(())
    }

    #[test]
    fn wait_ready_failure() -> Fallible<()> {
        let c = test_config()?;
        let d = tempdir()?;
        let mut p = Process::start(&c, d.path(), "echo", &["test"])?;
        p.readyness_timeout = 1;
        assert!(p.wait_ready("invalid").is_err());
        Ok(())
    }

    #[test]
    fn stop_success() -> Fallible<()> {
        let c = test_config()?;
        let d = tempdir()?;
        let mut p = Process::start(&c, d.path(), "sleep", &["500"])?;
        p.stop()?;
        Ok(())
    }
}
