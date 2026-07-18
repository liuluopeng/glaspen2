use std::sync::Mutex;
use tokio::io::AsyncWriteExt;
use tokio::sync::broadcast;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;

static BROADCASTER: Mutex<Option<broadcast::Sender<String>>> = Mutex::new(None);

const HTML: &str = r#"<!DOCTYPE html>
<html><head><meta charset="utf-8"><title>你画我猜</title>
<style>
*{margin:0;padding:0;box-sizing:border-box}
body{background:#222;display:flex;justify-content:center;align-items:center;height:100vh;font-family:sans-serif;color:#fff}
#container{text-align:center}
#canvas{max-width:100vw;max-height:90vh;border:1px solid #444;background:#fff;image-rendering:pixelated}
#status{padding:8px;font-size:14px}
</style></head><body>
<div id=container>
<img id=canvas src="" alt="waiting for strokes">
<div id=status>Connecting...</div>
</div>
<script>
let ws = new WebSocket('ws://' + location.host + '/ws');
let img = document.getElementById('canvas');
let st = document.getElementById('status');
ws.onopen = () => st.textContent = 'Connected';
ws.onmessage = (e) => { img.src = 'data:image/svg+xml;base64,' + btoa(e.data); };
ws.onclose = () => st.textContent = 'Disconnected';
</script></body></html>"#;

pub fn start_server() {
    std::thread::spawn(|| {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Failed to build WS runtime");
        rt.block_on(server_task());
    });
}

async fn server_task() {
    let (tx, _) = broadcast::channel::<String>(32);
    *BROADCASTER.lock().unwrap() = Some(tx.clone());

    let listener = TcpListener::bind("127.0.0.1:9876").await
        .expect("Failed to bind server");
    println!("[ws] draw-guess server on http://127.0.0.1:9876");

    while let Ok((stream, _)) = listener.accept().await {
        let tx = tx.clone();
        tokio::spawn(async move {
            handle_connection(stream, tx).await;
        });
    }
}

async fn handle_connection(mut raw: tokio::net::TcpStream, tx: broadcast::Sender<String>) {
    use tokio::io::AsyncReadExt;
    use sha1::{Sha1, Digest};
    use base64::Engine;

    let mut buf = Vec::new();
    let mut empty_lines = 0u32;
    loop {
        let mut byte = [0u8; 1];
        match raw.read(&mut byte).await {
            Ok(0) | Err(_) => return,
            Ok(_) => {
                buf.push(byte[0]);
                if byte[0] == b'\n' { empty_lines += 1; }
                else if byte[0] != b'\r' { empty_lines = 0; }
                if empty_lines >= 2 { break; }
            }
        }
    }

    let request = String::from_utf8_lossy(&buf);
    let is_ws = request.contains("Upgrade: websocket");

    if is_ws {
        let key = request.lines()
            .find(|l| l.to_lowercase().starts_with("sec-websocket-key:"))
            .and_then(|l| l.split(':').nth(1))
            .map(|s| s.trim())
            .unwrap_or("");
        if key.is_empty() { return; }

        let mut hasher = Sha1::new();
        hasher.update(key.as_bytes());
        hasher.update(b"258EAFA5-E914-47DA-95CA-5AB5C5D5EFB1");
        let accept = base64::engine::general_purpose::STANDARD.encode(hasher.finalize());
        let upgrade = format!(
            "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {}\r\n\r\n",
            accept
        );
        if raw.write_all(upgrade.as_bytes()).await.is_err() { return; }

        let ws = WebSocketStream::from_raw_socket(raw, tokio_tungstenite::tungstenite::protocol::Role::Server, None).await;
        handle_ws(ws, tx).await;
    } else {
        let resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
            HTML.len(), HTML
        );
        raw.write_all(resp.as_bytes()).await.ok();
    }
}

async fn handle_ws(mut ws: WebSocketStream<tokio::net::TcpStream>, tx: broadcast::Sender<String>) {
    let mut rx = tx.subscribe();
    loop {
        tokio::select! {
            msg = rx.recv() => {
                match msg {
                    Ok(svg) => { ws.send(Message::Text(svg.into())).await.ok(); }
                    Err(_) => break,
                }
            }
            _ = ws.next() => {
                break;
            }
        }
    }
}

pub fn broadcast_svg(svg: &str) {
    if let Some(tx) = BROADCASTER.lock().unwrap().as_ref() {
        tx.send(svg.to_string()).ok();
    }
}
