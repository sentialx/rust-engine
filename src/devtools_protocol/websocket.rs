//! WebSocket server for Chrome DevTools Protocol.
//!
//! Runs an async WebSocket server that Chrome DevTools can connect to.
//! Communicates with the main thread via channels since DevtoolsAgent
//! is not thread-safe (uses Rc<RefCell<>>).

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use futures_util::{SinkExt, StreamExt};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;
use winit::event_loop::EventLoopProxy;

/// A CDP request received from WebSocket, needs processing on main thread.
pub struct CdpRequest {
    pub id: u64,
    pub message: String,
    pub response_tx: mpsc::Sender<String>,
}

/// Sender for pushing CDP events to connected clients.
pub type CdpEventSender = mpsc::Sender<String>;

/// Shared waker to notify the main event loop when requests arrive.
type Waker = Arc<Mutex<Option<EventLoopProxy<()>>>>;

/// Shared event sender for pushing CDP events to connected clients.
type EventSender = Arc<Mutex<Option<mpsc::Sender<String>>>>;

/// Handle for the main thread to receive CDP requests.
pub struct DevtoolsReceiver {
    rx: mpsc::Receiver<CdpRequest>,
    waker: Waker,
    event_sender: EventSender,
}

impl DevtoolsReceiver {
    /// Try to receive a pending CDP request (non-blocking).
    pub fn try_recv(&mut self) -> Option<CdpRequest> {
        self.rx.try_recv().ok()
    }

    /// Set the event loop proxy to wake up the main thread when requests arrive.
    pub fn set_waker(&self, proxy: EventLoopProxy<()>) {
        *self.waker.lock().unwrap() = Some(proxy);
    }

    /// Send a CDP event to connected clients.
    pub fn send_event(&self, event: &str) {
        if let Some(sender) = self.event_sender.lock().unwrap().as_ref() {
            let _ = sender.blocking_send(event.to_string());
        }
    }
}

/// Starts the WebSocket server and returns a receiver for the main thread.
pub fn start_server(port: u16) -> DevtoolsReceiver {
    let (tx, rx) = mpsc::channel::<CdpRequest>(100);
    let waker: Waker = Arc::new(Mutex::new(None));
    let waker_clone = waker.clone();
    let event_sender: EventSender = Arc::new(Mutex::new(None));
    let event_sender_clone = event_sender.clone();

    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
        rt.block_on(run_server(port, tx, waker_clone, event_sender_clone));
    });

    DevtoolsReceiver { rx, waker, event_sender }
}

async fn run_server(port: u16, request_tx: mpsc::Sender<CdpRequest>, waker: Waker, event_sender: EventSender) {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = TcpListener::bind(&addr).await.expect("Failed to bind");

    println!("DevTools listening on ws://127.0.0.1:{}", port);
    println!("Open chrome://inspect or go to chrome://devtools/bundled/inspector.html?ws=127.0.0.1:{}&panel=elements", port);

    while let Ok((stream, peer)) = listener.accept().await {
        let tx = request_tx.clone();
        let waker = waker.clone();
        let event_sender = event_sender.clone();
        tokio::spawn(handle_connection(stream, peer, port, tx, waker, event_sender));
    }
}

async fn handle_connection(
    stream: TcpStream,
    peer: SocketAddr,
    port: u16,
    request_tx: mpsc::Sender<CdpRequest>,
    waker: Waker,
    event_sender: EventSender,
) {
    // Peek to check if this looks like a WebSocket upgrade (contains "Upgrade" header)
    let mut peek_buf = [0u8; 512];
    let n = match stream.peek(&mut peek_buf).await {
        Ok(n) => n,
        Err(_) => return,
    };

    let peek_str = String::from_utf8_lossy(&peek_buf[..n]);
    let is_websocket = peek_str.to_lowercase().contains("upgrade: websocket");

    if !is_websocket {
        // Plain HTTP request - read it properly
        let mut reader = BufReader::new(stream);
        let mut first_line = String::new();
        if reader.read_line(&mut first_line).await.is_err() {
            return;
        }

        let parts: Vec<&str> = first_line.trim().split_whitespace().collect();
        let path = if parts.len() >= 2 { parts[1] } else { "/" };
        println!("HTTP request: {} from {}", path, peer);

        // Consume remaining headers
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).await.is_err() {
                return;
            }
            if line.trim().is_empty() {
                break;
            }
        }

        handle_http_request(reader.into_inner(), path, port).await;
        return;
    }

    println!("WebSocket upgrade request from {}", peer);

    // WebSocket upgrade
    let ws_stream = match tokio_tungstenite::accept_async(stream).await {
        Ok(ws) => ws,
        Err(e) => {
            eprintln!("WebSocket handshake failed for {}: {}", peer, e);
            return;
        }
    };

    println!("DevTools client connected: {}", peer);

    let (mut ws_tx, mut ws_rx) = ws_stream.split();
    let (response_tx, mut response_rx) = mpsc::channel::<String>(100);

    // Store event sender so main thread can push events to this client
    *event_sender.lock().unwrap() = Some(response_tx.clone());

    // Task to send responses back to WebSocket
    let send_task = tokio::spawn(async move {
        while let Some(response) = response_rx.recv().await {
            if ws_tx.send(Message::Text(response)).await.is_err() {
                break;
            }
        }
    });

    // Receive messages from WebSocket and forward to main thread
    let mut request_id = 0u64;
    while let Some(msg) = ws_rx.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                println!("CDP request: {}", text);
                request_id += 1;
                let request = CdpRequest {
                    id: request_id,
                    message: text,
                    response_tx: response_tx.clone(),
                };
                if request_tx.send(request).await.is_err() {
                    break;
                }
                // Wake up the main event loop to process the request immediately
                if let Some(proxy) = waker.lock().unwrap().as_ref() {
                    let _ = proxy.send_event(());
                }
            }
            Ok(Message::Close(_)) => break,
            Err(e) => {
                eprintln!("WebSocket error: {}", e);
                break;
            }
            _ => {}
        }
    }

    // Clear event sender on disconnect
    *event_sender.lock().unwrap() = None;

    drop(response_tx);
    let _ = send_task.await;
    println!("DevTools client disconnected: {}", peer);
}

async fn handle_http_request(mut stream: TcpStream, path: &str, port: u16) {
    let response = match path {
        "/json" | "/json/list" => {
            let json = format!(r#"[{{
  "description": "Graviton Browser",
  "devtoolsFrontendUrl": "devtools://devtools/bundled/inspector.html?ws=127.0.0.1:{port}",
  "id": "graviton-main",
  "title": "Graviton",
  "type": "page",
  "url": "about:blank",
  "webSocketDebuggerUrl": "ws://127.0.0.1:{port}"
}}]"#);
            format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                json.len(),
                json
            )
        }
        "/json/version" => {
            let json = r#"{"Browser": "Graviton/1.0", "Protocol-Version": "1.3"}"#;
            format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                json.len(),
                json
            )
        }
        _ => "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n".to_string(),
    };

    let _ = stream.write_all(response.as_bytes()).await;
}
