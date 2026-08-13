use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use thiserror::Error;

#[derive(Clone, Debug)]
pub struct CommandSpec {
    pub program: String,
    pub arguments: Vec<String>,
    pub working_directory: PathBuf,
    pub timeout: Duration,
    pub environment: BTreeMap<String, String>,
}

#[derive(Clone, Debug)]
pub struct CommandOutput {
    pub success: bool,
    pub status: Option<i32>,
    pub text: String,
}

pub trait CommandRunner: Send + Sync {
    fn run(&self, command: &CommandSpec) -> Result<CommandOutput, ProcessError>;

    fn run_quiet(&self, command: &CommandSpec) -> Result<CommandOutput, ProcessError> {
        self.run(command)
    }
}

#[derive(Default)]
pub struct SystemCommandRunner;

impl CommandRunner for SystemCommandRunner {
    fn run(&self, command: &CommandSpec) -> Result<CommandOutput, ProcessError> {
        run_command(command, true)
    }

    fn run_quiet(&self, command: &CommandSpec) -> Result<CommandOutput, ProcessError> {
        run_command(command, false)
    }
}

fn run_command(command: &CommandSpec, echo: bool) -> Result<CommandOutput, ProcessError> {
    let mut child = Command::new(&command.program)
        .args(&command.arguments)
        .envs(&command.environment)
        .current_dir(&command.working_directory)
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|source| ProcessError::Start { program: command.program.clone(), source })?;
    let stdout = child.stdout.take().expect("piped stdout");
    let stderr = child.stderr.take().expect("piped stderr");
    let out_thread = thread::spawn(move || read_stream(stdout, false, echo));
    let err_thread = thread::spawn(move || read_stream(stderr, true, echo));
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().map_err(ProcessError::Wait)? {
            break status;
        }
        if started.elapsed() >= command.timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(ProcessError::Timeout {
                program: command.program.clone(),
                timeout: command.timeout,
            });
        }
        thread::sleep(Duration::from_millis(100));
    };
    let mut text = out_thread.join().unwrap_or_default();
    text.push_str(&err_thread.join().unwrap_or_default());
    Ok(CommandOutput { success: status.success(), status: status.code(), text })
}

fn read_stream<R: std::io::Read + Send + 'static>(reader: R, stderr: bool, echo: bool) -> String {
    let mut collected = String::new();
    for line in BufReader::new(reader).lines().map_while(Result::ok) {
        if echo && stderr {
            eprintln!("{line}");
        } else if echo {
            println!("{line}");
        }
        collected.push_str(&line);
        collected.push('\n');
    }
    collected
}

#[derive(Debug, Error)]
pub enum ProcessError {
    #[error("could not start external command `{program}`: {source}")]
    Start { program: String, source: std::io::Error },
    #[error("could not wait for external command: {0}")]
    Wait(std::io::Error),
    #[error("external command `{program}` exceeded timeout of {timeout:?}")]
    Timeout { program: String, timeout: Duration },
}
