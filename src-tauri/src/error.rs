//! 统一错误类型：可序列化到前端（绝不包含 Token）。

use serde::{ser::Serializer, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("配置错误: {0}")]
    Config(String),
    #[error("监控器错误: {0}")]
    Monitor(String),
    #[error("网络错误: {0}")]
    Http(String),
    #[error("文件选择/IO 错误: {0}")]
    Io(String),
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Io(e.to_string())
    }
}

impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        AppError::Http(redact_url_error(&e))
    }
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

/// reqwest 错误里可能带 URL（含 query token 等），只保留不含 URL 的部分。
pub fn redact_url_error(e: &reqwest::Error) -> String {
    let s = e.to_string();
    match e.url() {
        Some(_) => s.split(" for url").next().unwrap_or(&s).trim().to_string(),
        None => s,
    }
}

pub type AppResult<T> = Result<T, AppError>;
