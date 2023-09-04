use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DbData {
    pub block: u64,
}

const DATA_FILE_NAME: &str = "data.json";

async fn read_from_file(file_path: &Path) -> Result<DbData, std::io::Error> {
    let mut data_file = fs::File::open(file_path).await?;
    let mut buffer = Vec::new();
    data_file.read_to_end(&mut buffer).await?;
    let data: DbData = serde_json::from_slice(&buffer)?;
    Ok(data)
}

async fn write_to_file(file_path: &Path, data: &DbData) -> Result<(), std::io::Error> {
    let mut data_file = fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(file_path)
        .await?;
    let json_data = serde_json::to_string(data)?;
    data_file.write_all(json_data.as_bytes()).await?;
    Ok(())
}

pub async fn db_create_if_not_exists(db_path: &Path) -> Result<(), std::io::Error> {
    let file_path: PathBuf = Path::new(db_path).join(DATA_FILE_NAME);
    if file_path.exists() {
        Ok(())
    } else {
        // If database file does not exist create it with default values
        if let Some(prefix) = file_path.parent() {
            fs::create_dir_all(prefix).await?;
        }
        let mut data_file = fs::File::create(file_path).await?;
        let json_data = serde_json::to_string(&DbData::default())?;
        data_file.write_all(json_data.as_bytes()).await?;
        Ok(())
    }
}

pub async fn db_read_subnet_block_number(db_path: &Path) -> Result<u64, std::io::Error> {
    let file_path: PathBuf = Path::new(db_path).join(DATA_FILE_NAME);
    Ok(read_from_file(&file_path).await?.block)
}

pub async fn db_write_subnet_block_number(
    db_path: &Path,
    new_block_number: u64,
) -> Result<(), std::io::Error> {
    let file_path: PathBuf = Path::new(db_path).join(DATA_FILE_NAME);
    let mut data: DbData = read_from_file(&file_path).await?;
    data.block = new_block_number;
    write_to_file(&file_path, &data).await
}
