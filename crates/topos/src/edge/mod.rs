use serde_json::Value;
use std::collections::HashMap;
use std::os::unix::prelude::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Stdio};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
};
use tracing::debug;
use tracing::{error, info, warn};

pub const BINARY_NAME: &str = "polygon-edge";

pub struct CommandConfig {
    binary_path: PathBuf,
    args: Vec<String>,
}

impl CommandConfig {
    pub fn new(binary_path: PathBuf) -> Self {
        let binary_path = if binary_path == PathBuf::from(".") {
            std::env::current_dir()
                .expect("Cannot get the current directory")
                .join(BINARY_NAME)
        } else {
            binary_path
        };

        CommandConfig {
            binary_path,
            args: Vec::new(),
        }
    }

    pub fn init(mut self, path: &Path) -> Self {
        self.args.push("secrets".into());
        self.args.push("init".into());
        self.args.push("--insecure".into());
        self.args.push("--data-dir".into());
        self.args.push(format!("{}", path.display()));
        self
    }

    pub fn server(
        mut self,
        data_dir: &Path,
        genesis_path: &Path,
        edge_args: HashMap<String, String>,
    ) -> Self {
        self.args.push("server".into());
        self.args.push("--data-dir".into());
        self.args.push(format!("{}", data_dir.display()));
        self.args.push("--chain".into());
        self.args.push(format!("{}", genesis_path.display()));
        self.args.push("--json".into());

        for (k, v) in &edge_args {
            self.args.push(format!("--{k}"));
            self.args.push(v.to_string());
        }

        self
    }

    pub async fn spawn(self) -> Result<ExitStatus, std::io::Error> {
        let mut command = Command::new(self.binary_path);
        command.kill_on_drop(true);
        command.args(self.args);

        let mut child = command
            .stderr(Stdio::piped())
            .stdout(Stdio::inherit())
            .stdin(Stdio::inherit())
            .spawn()?;

        if let Some(pid) = child.id() {
            info!("Polygon Edge child process with pid {pid} successfully started");
        }

        let stdout = child
            .stderr
            .take()
            .expect("child did not have a handle to stdout");
        let mut reader = BufReader::new(stdout).lines();

        let running = async { child.wait().await };

        let logging = async {
            while let Ok(line) = reader.next_line().await {
                match line {
                    Some(l) => match serde_json::from_str(&l) {
                        Ok(v) => EdgeLog::new(v).log(),
                        Err(_) => println!("{l}"),
                    },
                    None => break,
                }
            }
        };

        let (running_out, _) = tokio::join!(running, logging);

        let exit_status = running_out?;

        info!(
            "The Edge process is terminated with exit status {:?}; exit code: {:?}, exit signal \
             {:?}, success: {:?}, raw code: {}",
            exit_status,
            exit_status.code(),
            exit_status.signal(),
            exit_status.success(),
            exit_status.into_raw(),
        );
        Ok(exit_status)
    }
}

pub struct EdgeLog {
    v: HashMap<String, Value>,
}

impl EdgeLog {
    pub fn new(v: HashMap<String, Value>) -> Self {
        Self { v }
    }

    pub fn log(&mut self) {
        match self.v.get("@level") {
            Some(level) => match level.as_str() {
                Some(r#"info"#) => info!("{}", self.internal()),
                Some(r#"warn"#) => warn!("{}", self.internal()),
                Some(r#"debug"#) => debug!("{}", self.internal()),
                Some(r#"error"#) => error!("{}", self.internal()),
                _ => error!("log parse failure: {:?}", self.v),
            },
            None => error!("{:?}", self.v.get("error")),
        }
    }

    fn internal(&mut self) -> String {
        let module = self.v.remove("@module").unwrap();
        let message = self.v.remove("@message").unwrap();

        // FIXME: Figure out tracing features to make this nicer
        self.v.remove("@timestamp");
        self.v.remove("@level");

        let mut message = format!("{module}: {message}");

        for (k, s) in &self.v {
            message = format!("{} {}:{}", message, k, s);
        }

        message
    }
}
