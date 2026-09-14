//! 音箱 HTTP 客户端（xiaoi webhook 协议）。
//!
//! 契约（plan §8）：GET /webhook/volume?did=... 带 X-Xiaoi-Token → {success:true,volume:18}；
//! POST /webhook/volume {did,volume} 带 X-Xiaoi-Token → {success:true,volume,result}。
//! 校验 HTTP 状态与 success 字段；volume 读取 0..100；超时 + 有限重试；
//! 绝不把 token / URL 写进错误信息与日志。

use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub const TOKEN_HEADER: &str = "X-Xiaoi-Token";

#[derive(Debug, Clone)]
pub struct SpeakerClient {
    pub server_url: String,
    pub token: String,
    pub max_retries: u32,
    pub retry_delay: Duration,
    http: reqwest::Client,
}

/// 服务端响应包络（宽松解析，多余字段忽略）
#[derive(Debug, Deserialize)]
struct WebhookEnvelope {
    success: bool,
    #[serde(default)]
    volume: Option<i64>,
    #[serde(default)]
    message: Option<String>,
}

impl SpeakerClient {
    pub fn new(server_url: impl Into<String>, token: impl Into<String>) -> Self {
        Self::with_options(server_url, token, Duration::from_secs(5), 2, Duration::from_millis(500))
    }

    /// 测试可注入更短超时/重试间隔。
    pub fn with_options(
        server_url: impl Into<String>,
        token: impl Into<String>,
        timeout: Duration,
        max_retries: u32,
        retry_delay: Duration,
    ) -> Self {
        SpeakerClient {
            server_url: server_url.into(),
            token: token.into(),
            max_retries,
            retry_delay,
            http: reqwest::Client::builder()
                .timeout(timeout)
                .build()
                .expect("reqwest client"),
        }
    }

    fn url(&self) -> AppResult<String> {
        let base = self.server_url.trim().trim_end_matches('/');
        if base.is_empty() {
            return Err(AppError::Config("serverUrl 未配置".into()));
        }
        let base = if base.starts_with("http://") || base.starts_with("https://") {
            base.to_string()
        } else {
            format!("http://{base}")
        };
        Ok(format!("{base}/webhook/volume"))
    }

    /// GET 读音量（0..100）。
    pub async fn get_volume(&self, did: &str) -> AppResult<u8> {
        let url = self.url()?;
        let envelope = self
            .request_with_retry(|| {
                let mut req = self.http.get(&url).query(&[("did", did)]);
                if !self.token.is_empty() {
                    req = req.header(TOKEN_HEADER, &self.token);
                }
                req
            })
            .await?;
        self.check_envelope(&envelope, "GET")
    }

    /// POST 写音量（0..100）。返回服务端确认音量。
    pub async fn set_volume(&self, did: &str, volume: u8) -> AppResult<u8> {
        if volume > 100 {
            return Err(AppError::Config(format!("音量超范围: {volume}")));
        }
        let url = self.url()?;
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Body<'a> {
            did: &'a str,
            volume: u8,
        }
        let envelope = self
            .request_with_retry(|| {
                let mut req = self.http.post(&url).json(&Body { did, volume });
                if !self.token.is_empty() {
                    req = req.header(TOKEN_HEADER, &self.token);
                }
                req
            })
            .await?;
        self.check_envelope(&envelope, "POST")
    }

    /// 带重试的请求执行。网络/超时错误有限重试；HTTP 非 2xx 与业务失败不重试。
    async fn request_with_retry<F>(&self, build: F) -> AppResult<WebhookEnvelope>
    where
        F: Fn() -> reqwest::RequestBuilder,
    {
        let mut attempt: u32 = 0;
        loop {
            match build().send().await {
                Ok(resp) => {
                    let status = resp.status();
                    let text = resp.text().await.unwrap_or_default();
                    if !status.is_success() {
                        return Err(AppError::Http(format!(
                            "HTTP {}（{}）",
                            status.as_u16(),
                            short_reason(status.as_u16())
                        )));
                    }
                    return serde_json::from_str::<WebhookEnvelope>(&text)
                        .map_err(|_| AppError::Http("响应不是有效 JSON".into()));
                }
                Err(e) => {
                    if attempt >= self.max_retries {
                        return Err(AppError::Http(crate::error::redact_url_error(&e)));
                    }
                    attempt += 1;
                    tokio::time::sleep(self.retry_delay).await;
                }
            }
        }
    }

    fn check_envelope(&self, env: &WebhookEnvelope, op: &str) -> AppResult<u8> {
        if !env.success {
            return Err(AppError::Http(format!(
                "{op} 业务失败{}",
                env.message
                    .as_deref()
                    .map(|m| format!(": {m}"))
                    .unwrap_or_default()
            )));
        }
        match env.volume {
            Some(v) if (0..=100).contains(&v) => Ok(v as u8),
            Some(v) => Err(AppError::Http(format!("{op} 响应音量越界: {v}"))),
            None => Err(AppError::Http(format!("{op} 响应缺少 volume 字段"))),
        }
    }
}

fn short_reason(code: u16) -> &'static str {
    match code {
        401 => "鉴权失败",
        403 => "禁止访问",
        404 => "接口不存在",
        500 => "服务器内部错误",
        502 | 504 => "网关错误",
        _ => "请求失败",
    }
}

// ===========================================================================
// Mock HTTP 服务器（仅测试）：手工 TCP 实现，不依赖额外 crate，不访问真实音箱
// ===========================================================================
#[cfg(test)]
pub(crate) mod mock_server {
    use std::collections::VecDeque;
    use std::net::SocketAddr;
    use std::sync::{Arc, Mutex};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};

    /// 服务端行为脚本：每个连接弹出一条；重复使用最后一条
    #[derive(Debug, Clone)]
    pub enum MockAction {
        /// (status, content_type, body)
        Respond(u16, String),
        /// 延迟毫秒后响应（测超时）
        DelayMs(u64, u16, String),
        /// 直接断开连接不发响应
        Close,
    }

    #[derive(Debug, Clone)]
    pub struct RecordedRequest {
        pub method: String,
        pub path: String,
        pub token_header: Option<String>,
        pub body: String,
    }

    pub struct MockServer {
        pub addr: SocketAddr,
        pub requests: Arc<Mutex<Vec<RecordedRequest>>>,
        shutdown: Arc<tokio::sync::Notify>,
    }

    impl MockServer {
        pub async fn start(script: Vec<MockAction>) -> MockServer {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();
            let requests = Arc::new(Mutex::new(Vec::new()));
            let script: Arc<Mutex<VecDeque<MockAction>>> =
                Arc::new(Mutex::new(script.into()));
            let shutdown = Arc::new(tokio::sync::Notify::new());
            let shutdown2 = shutdown.clone();
            let requests2 = requests.clone();
            tokio::spawn(async move {
                loop {
                    tokio::select! {
                        _ = shutdown2.notified() => break,
                        accepted = listener.accept() => {
                            let Ok((stream, _)) = accepted else { continue };
                            let req = requests2.clone();
                            let script = script.clone();
                            tokio::spawn(handle_conn(stream, req, script));
                        }
                    }
                }
            });
            MockServer { addr, requests, shutdown }
        }

        pub fn url(&self) -> String {
            format!("http://{}", self.addr)
        }

        pub fn recorded(&self) -> Vec<RecordedRequest> {
            self.requests.lock().unwrap().clone()
        }

        pub fn stop(&self) {
            self.shutdown.notify_waiters();
        }
    }

    impl Drop for MockServer {
        fn drop(&mut self) {
            self.stop();
        }
    }

    async fn handle_conn(
        mut stream: TcpStream,
        log: Arc<Mutex<Vec<RecordedRequest>>>,
        script: Arc<Mutex<VecDeque<MockAction>>>,
    ) {
        // 读取请求头
        let mut buf = Vec::with_capacity(2048);
        let mut chunk = [0u8; 1024];
        let header_end = loop {
            let n = match stream.read(&mut chunk).await {
                Ok(0) | Err(_) => return,
                Ok(n) => n,
            };
            buf.extend_from_slice(&chunk[..n]);
            if let Some(pos) = find_subslice(&buf, b"\r\n\r\n") {
                break pos + 4;
            }
            if buf.len() > 64 * 1024 {
                return;
            }
        };
        let head = String::from_utf8_lossy(&buf[..header_end]).into_owned();
        let mut lines = head.split("\r\n");
        let request_line = lines.next().unwrap_or_default().to_string();
        let mut method = String::new();
        let mut path = String::new();
        if let Some(first) = request_line.split_whitespace().next() {
            method = first.to_string();
        }
        if let Some(p) = request_line.split_whitespace().nth(1) {
            path = p.to_string();
        }
        let mut token_header = None;
        let mut content_length = 0usize;
        for line in lines {
            if let Some((k, v)) = line.split_once(':') {
                let k = k.trim().to_ascii_lowercase();
                let v = v.trim().to_string();
                if k == "x-xiaoi-token" {
                    token_header = Some(v);
                } else if k == "content-length" {
                    content_length = v.parse().unwrap_or(0);
                }
            }
        }
        // 读取 body
        let mut body = buf[header_end..].to_vec();
        while body.len() < content_length {
            let n = match stream.read(&mut chunk).await {
                Ok(0) | Err(_) => break,
                Ok(n) => n,
            };
            body.extend_from_slice(&chunk[..n]);
        }
        log.lock().unwrap().push(RecordedRequest {
            method,
            path,
            token_header,
            body: String::from_utf8_lossy(&body).into_owned(),
        });

        // 取脚本动作
        let action = {
            let mut q = script.lock().unwrap();
            if q.len() > 1 {
                q.pop_front().unwrap()
            } else {
                q.back().cloned().unwrap_or(MockAction::Close)
            }
        };
        match action {
            MockAction::Close => {
                let _ = stream.shutdown().await;
            }
            MockAction::DelayMs(ms, status, body) => {
                tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
                respond(&mut stream, status, &body).await;
            }
            MockAction::Respond(status, body) => {
                respond(&mut stream, status, &body).await;
            }
        }
    }

    async fn respond(stream: &mut TcpStream, status: u16, body: &str) {
        let reason = match status {
            200 => "OK",
            401 => "Unauthorized",
            500 => "Internal Server Error",
            _ => "Status",
        };
        let resp = format!(
            "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = stream.write_all(resp.as_bytes()).await;
        let _ = stream.shutdown().await;
    }

    fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        haystack
            .windows(needle.len())
            .position(|w| w == needle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mock_server::{MockAction, MockServer};

    const TOKEN: &str = "test-token-DoNotLog";

    fn client(url: &str) -> SpeakerClient {
        SpeakerClient::with_options(
            url,
            TOKEN,
            Duration::from_millis(400),
            2,
            Duration::from_millis(30),
        )
    }

    #[test]
    fn envelope_parse_variants() {
        let ok: WebhookEnvelope = serde_json::from_str(r#"{"success":true,"volume":18}"#).unwrap();
        assert!(ok.success);
        assert_eq!(ok.volume, Some(18));
        let no_vol: WebhookEnvelope = serde_json::from_str(r#"{"success":true}"#).unwrap();
        assert_eq!(no_vol.volume, None);
        let biz: WebhookEnvelope =
            serde_json::from_str(r#"{"success":false,"message":"offline"}"#).unwrap();
        assert!(!biz.success);
    }

    #[test]
    fn url_building() {
        assert_eq!(
            SpeakerClient::new("https://x.example/", "t").url().unwrap(),
            "https://x.example/webhook/volume"
        );
        assert_eq!(
            SpeakerClient::new("x.example", "t").url().unwrap(),
            "http://x.example/webhook/volume"
        );
        assert!(SpeakerClient::new("", "t").url().is_err());
    }

    #[tokio::test]
    async fn set_volume_rejects_out_of_range_locally() {
        let c = SpeakerClient::new("https://x.example", "t");
        let err = c.set_volume("d", 101).await.unwrap_err();
        assert!(err.to_string().contains("音量超范围"));
    }

    #[tokio::test]
    async fn get_volume_ok() {
        let s = MockServer::start(vec![MockAction::Respond(
            200,
            r#"{"success":true,"volume":18}"#.into(),
        )])
        .await;
        let v = client(&s.url()).get_volume("did-1").await.unwrap();
        assert_eq!(v, 18);
        let reqs = s.recorded();
        assert_eq!(reqs.len(), 1);
        assert_eq!(reqs[0].method, "GET");
        assert_eq!(reqs[0].path, "/webhook/volume?did=did-1");
        assert_eq!(reqs[0].token_header.as_deref(), Some(TOKEN));
    }

    #[tokio::test]
    async fn set_volume_ok_posts_camel_case_body() {
        let s = MockServer::start(vec![MockAction::Respond(
            200,
            r#"{"success":true,"volume":20,"result":"ok"}"#.into(),
        )])
        .await;
        let v = client(&s.url()).set_volume("did-2", 20).await.unwrap();
        assert_eq!(v, 20);
        let reqs = s.recorded();
        assert_eq!(reqs.len(), 1);
        assert_eq!(reqs[0].method, "POST");
        assert!(reqs[0].body.contains(r#""did":"did-2""#), "body: {}", reqs[0].body);
        assert!(reqs[0].body.contains(r#""volume":20"#));
        assert_eq!(reqs[0].token_header.as_deref(), Some(TOKEN));
    }

    #[tokio::test]
    async fn http_401_is_error_no_retry() {
        let s = MockServer::start(vec![MockAction::Respond(
            401,
            r#"{"success":false}"#.into(),
        )])
        .await;
        let err = client(&s.url()).get_volume("d").await.unwrap_err();
        assert!(err.to_string().contains("401"), "{err}");
        assert_eq!(s.recorded().len(), 1, "HTTP 错误不重试");
    }

    #[tokio::test]
    async fn http_500_is_error_no_retry() {
        let s = MockServer::start(vec![MockAction::Respond(
            500,
            "internal".into(),
        )])
        .await;
        let err = client(&s.url()).set_volume("d", 10).await.unwrap_err();
        assert!(err.to_string().contains("500"), "{err}");
        assert_eq!(s.recorded().len(), 1, "HTTP 错误不重试");
    }

    #[tokio::test]
    async fn business_failure_reported() {
        let s = MockServer::start(vec![MockAction::Respond(
            200,
            r#"{"success":false,"message":"device offline"}"#.into(),
        )])
        .await;
        let err = client(&s.url()).set_volume("d", 10).await.unwrap_err();
        assert!(err.to_string().contains("业务失败"), "{err}");
        assert!(err.to_string().contains("device offline"));
    }

    #[tokio::test]
    async fn out_of_range_volume_in_response_rejected() {
        for bad in [r#"{"success":true,"volume":150}"#, r#"{"success":true,"volume":-3}"#] {
            let s = MockServer::start(vec![MockAction::Respond(200, bad.into())]).await;
            let err = client(&s.url()).get_volume("d").await.unwrap_err();
            assert!(err.to_string().contains("越界"), "{bad} -> {err}");
        }
    }

    #[tokio::test]
    async fn invalid_json_rejected() {
        let s = MockServer::start(vec![MockAction::Respond(200, "not json".into())]).await;
        let err = client(&s.url()).get_volume("d").await.unwrap_err();
        assert!(err.to_string().contains("JSON"), "{err}");
    }

    #[tokio::test]
    async fn network_error_retries_then_succeeds() {
        // 第一次连接被断开（网络错误→重试），第二次成功
        let s = MockServer::start(vec![
            MockAction::Close,
            MockAction::Respond(200, r#"{"success":true,"volume":7}"#.into()),
        ])
        .await;
        let v = client(&s.url()).get_volume("d").await.unwrap();
        assert_eq!(v, 7);
        assert_eq!(s.recorded().len(), 2, "断连后应重试一次");
    }

    #[tokio::test]
    async fn network_error_exhausts_retries() {
        let s = MockServer::start(vec![MockAction::Close]).await;
        let err = client(&s.url()).get_volume("d").await.unwrap_err();
        assert!(matches!(err, AppError::Http(_)), "{err}");
        // max_retries=2 → 3 次尝试
        assert_eq!(s.recorded().len(), 3);
    }

    #[tokio::test]
    async fn timeout_errors_and_redacts_url() {
        // 服务端延迟 2s，客户端超时 400ms → 超时错误；错误信息不得含 URL/token
        let s = MockServer::start(vec![MockAction::DelayMs(
            2000,
            200,
            r#"{"success":true,"volume":1}"#.into(),
        )])
        .await;
        let err = client(&s.url()).get_volume("d").await.unwrap_err();
        assert!(matches!(err, AppError::Http(_)), "{err}");
        let msg = err.to_string();
        assert!(!msg.contains("127.0.0.1"), "错误不得泄露 URL: {msg}");
        assert!(!msg.contains(TOKEN), "错误不得泄露 token: {msg}");
    }

    #[tokio::test]
    async fn no_token_header_when_empty() {
        let s = MockServer::start(vec![MockAction::Respond(
            200,
            r#"{"success":true,"volume":5}"#.into(),
        )])
        .await;
        let c = SpeakerClient::with_options(
            s.url(),
            "",
            Duration::from_millis(400),
            0,
            Duration::from_millis(10),
        );
        assert_eq!(c.get_volume("d").await.unwrap(), 5);
        assert_eq!(s.recorded()[0].token_header, None);
    }
}
