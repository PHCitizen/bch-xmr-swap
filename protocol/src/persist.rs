use std::io::ErrorKind;

use fs4::tokio::AsyncFileExt;
use serde::{Deserialize, Serialize};
use tokio::{
    fs::{self, File},
    io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt},
};

use crate::protocol::SwapWrapper;

#[derive(Debug)]
pub enum Error {
    NotFound,
    Unknown(String),
}

impl<T: ToString> From<T> for Error {
    fn from(value: T) -> Self {
        Error::Unknown(value.to_string())
    }
}

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub swap: SwapWrapper,
    pub refund_private_key: bitcoincash::PrivateKey,
}

pub struct TradePersist {
    file: File,
    file_path: String,
    pub config: Config,
}

impl TradePersist {
    pub async fn restore(file_path: String) -> Result<TradePersist, Error> {
        match fs::OpenOptions::new()
            .write(true)
            .read(true)
            .open(file_path.clone())
            .await
        {
            Err(e) => match e.kind() {
                ErrorKind::NotFound => return Err(Error::NotFound),
                _ => return Err(Error::from(e.to_string())),
            },
            Ok(mut file) => {
                file.lock_exclusive()?;
                let mut buf = Vec::new();
                let _ = file.read_to_end(&mut buf).await?;

                Ok(TradePersist {
                    file,
                    config: serde_json::from_slice(&buf)?,
                    file_path,
                })
            }
        }
    }

    pub async fn delete(self) {
        if let Err(err) = fs::remove_file(&self.file_path).await {
            eprintln!("Error deleting file: {}", err);
        } else {
            println!("File deleted successfully");
        }
    }

    pub async fn save(&mut self) {
        let serialized = serde_json::to_vec_pretty(&self.config).unwrap();
        self.file.set_len(0).await.unwrap();
        self.file.rewind().await.unwrap();
        let _ = self.file.write(&serialized).await.unwrap();
    }
}
