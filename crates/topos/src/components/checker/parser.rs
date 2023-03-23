use serde::Deserialize;
use std::{
    fs::File,
    io::{self, Read},
    path::Path,
};

use crate::options::input_format::{InputFormat, Parser};

pub(crate) struct NodeList(pub(crate) Option<String>);

#[derive(Deserialize)]
struct FileNodes {
    nodes: Vec<String>,
}

impl Parser<NodeList> for InputFormat {
    type Result = Result<Vec<String>, io::Error>;

    fn parse(&self, NodeList(input): NodeList) -> Self::Result {
        let mut input_string = String::new();
        _ = match input {
            Some(path) if Path::new(&path).is_file() => {
                File::open(path)?.read_to_string(&mut input_string)?
            }
            Some(string) => {
                input_string = string;
                0
            }
            None => io::stdin().read_to_string(&mut input_string)?,
        };

        match self {
            InputFormat::Json => Ok(serde_json::from_str::<FileNodes>(&input_string)?.nodes),
            InputFormat::Plain => Ok(input_string
                .trim()
                .split(&[',', '\n'])
                .map(|s| s.trim().to_string())
                .collect()),
        }
    }
}
