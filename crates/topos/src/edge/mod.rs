use futures::stream::FuturesUnordered;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Stdio};
use std::time::Duration;
use std::{collections::HashMap, future::Future};
use tokio::time::sleep;
use tokio::{
    io::{self, AsyncBufReadExt, BufReader},
    process::{Child, Command},
};
use tracing::debug;
use tracing::{error, event, info, warn, Level};

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

    pub fn server(mut self, data_dir: &Path, genesis_path: &Path) -> Self {
        self.args.push("server".into());
        self.args.push("--data-dir".into());
        self.args.push(format!("{}", data_dir.display()));
        self.args.push("--chain".into());
        self.args.push(format!("{}", genesis_path.display()));
        self.args.push("--json".into());

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

        info!("The Edge process is terminated");
        running_out
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
