#![allow(dead_code)]

use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[derive(Clone)]
pub struct MockResponse {
    pub status_code: u16,
    pub body: String,
    pub delay_ms: u64,
}

impl MockResponse {
    pub fn ok_json(body: impl Into<String>) -> Self {
        Self {
            status_code: 200,
            body: body.into(),
            delay_ms: 0,
        }
    }

    pub fn with_delay_ms(mut self, delay_ms: u64) -> Self {
        self.delay_ms = delay_ms;
        self
    }

    pub fn final_answer(content: &str) -> Self {
        let json = serde_json::json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": content
                }
            }],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 15,
                "total_tokens": 25
            }
        });
        Self::ok_json(json.to_string())
    }

    pub fn tool_call(call_id: &str, tool_name: &str, arguments: serde_json::Value) -> Self {
        Self::multi_tool_calls(&[(call_id, tool_name, arguments)])
    }

    pub fn multi_tool_calls(calls: &[(&str, &str, serde_json::Value)]) -> Self {
        let tool_calls: Vec<serde_json::Value> = calls
            .iter()
            .map(|(call_id, tool_name, arguments)| {
                serde_json::json!({
                    "id": call_id,
                    "type": "function",
                    "function": {
                        "name": tool_name,
                        "arguments": arguments.to_string()
                    }
                })
            })
            .collect();
        let json = serde_json::json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": tool_calls
                }
            }],
            "usage": {
                "prompt_tokens": 15,
                "completion_tokens": 20,
                "total_tokens": 35
            }
        });
        Self::ok_json(json.to_string())
    }

    pub fn error(status_code: u16, message: &str) -> Self {
        let json = serde_json::json!({
            "error": {
                "message": message
            }
        });
        Self {
            status_code,
            body: json.to_string(),
            delay_ms: 0,
        }
    }
}

pub struct TestHttpServer {
    addr: SocketAddr,
    responses: Arc<Mutex<VecDeque<MockResponse>>>,
    requests: Arc<Mutex<Vec<String>>>,
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

impl TestHttpServer {
    pub async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let responses = Arc::new(Mutex::new(VecDeque::new()));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel::<()>();

        let resp_clone = responses.clone();
        let req_clone = requests.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = &mut shutdown_rx => {
                        break;
                    }
                    accept_res = listener.accept() => {
                        let Ok((mut socket, _)) = accept_res else {
                            break;
                        };
                        let resp_queue = resp_clone.clone();
                        let req_log = req_clone.clone();
                        tokio::spawn(async move {
                            let mut buf = vec![0u8; 65536];
                            let n = socket.read(&mut buf).await.unwrap_or(0);
                            if n == 0 {
                                return;
                            }
                            let raw_req = String::from_utf8_lossy(&buf[..n]).to_string();
                            {
                                let mut lock = req_log.lock().unwrap();
                                lock.push(raw_req);
                            }

                            let mock = {
                                let mut lock = resp_queue.lock().unwrap();
                                lock.pop_front().unwrap_or_else(|| {
                                    MockResponse::final_answer("fallback default mock response")
                                })
                            };

                            if mock.delay_ms > 0 {
                                tokio::time::sleep(Duration::from_millis(mock.delay_ms)).await;
                            }

                            let status_text = match mock.status_code {
                                200 => "OK",
                                400 => "Bad Request",
                                401 => "Unauthorized",
                                403 => "Forbidden",
                                404 => "Not Found",
                                500 => "Internal Server Error",
                                _ => "Custom",
                            };

                            let body_bytes = mock.body.as_bytes();
                            let header = format!(
                                "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                mock.status_code,
                                status_text,
                                body_bytes.len()
                            );

                            let _ = socket.write_all(header.as_bytes()).await;
                            let _ = socket.write_all(body_bytes).await;
                            let _ = socket.flush().await;
                        });
                    }
                }
            }
        });

        Self {
            addr,
            responses,
            requests,
            shutdown_tx: Some(shutdown_tx),
        }
    }

    pub fn url(&self) -> String {
        format!("http://{}", self.addr)
    }

    pub fn enqueue_response(&self, response: MockResponse) {
        let mut lock = self.responses.lock().unwrap();
        lock.push_back(response);
    }

    pub fn received_requests(&self) -> Vec<String> {
        let lock = self.requests.lock().unwrap();
        lock.clone()
    }
}

impl Drop for TestHttpServer {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}
