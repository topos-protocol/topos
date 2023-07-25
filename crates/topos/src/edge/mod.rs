use std::future::Future;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::{Child, Command};
use tracing::{error, info};

pub const BINARY_NAME: &str = "polygon-edge";

pub struct CommandConfig {
    binary_path: PathBuf,
    args: Vec<String>,
}

impl CommandConfig {
    pub fn new(binary_path: PathBuf) -> Self {
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

        self
    }

    pub async fn spawn(self) -> Result<ExitStatus, std::io::Error> {
        let mut command = Command::new(self.binary_path);
        command.kill_on_drop(true);
        command.args(self.args);

        async move {
            match command
                .stderr(Stdio::inherit())
                .stdout(Stdio::inherit())
                .stdin(Stdio::inherit())
                .spawn()
            {
                Ok(mut child) => {
                    if let Some(pid) = child.id() {
                        info!("Polygon Edge child process with pid {pid} successfully started");
                    }
                    if let Err(e) = child.wait().await {
                        info!("Polygon Edge child process finished with error: {e}");
                    }
                    std::process::exit(0);
                }
                Err(e) => {
                    error!("Error executing Polygon Edge: {e}");
                    Err(ProcessError::EdgeFailure)
                }
            }
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

        debug!("The Edge process is terminated");
        running_out
    }
}
