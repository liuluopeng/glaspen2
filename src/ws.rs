use std::sync::Mutex;
use tokio::sync::broadcast;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream as WsStream;
use tokio_tungstenite::tungstenite::protocol::Role;
use futures_util::{SinkExt, StreamExt};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

static BROADCASTER: Mutex<Option<broadcast::Sender<String>>> = Mutex::new(None);

const INDEX_HTML: &str = r#"<!DOCTYPE html>
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
let ws = new WebSocket('ws://' + location.host);
let img = document.getElementById('canvas');
let st = document.getElementById('status');
ws.onopen = () => st.textContent = 'Connected - draw on glaspen2';
ws.onmessage = (e) => { img.src = 'data:image/svg+xml;base64,' + btoa(e.data); };
ws.onerror = (e) => st.textContent = 'WS Error: ' + (e.message || 'unknown');
ws.onclose = () => st.textContent = 'Disconnected';
</script></body></html>"#;

pub fn start_server() {
    eprintln!("[ws] draw-guess: open http://localhost:9876 in browser");
    std::thread::spawn(|| {
        let rt = tokio::runtime::Runtime::new().expect("WS runtime");
        rt.block_on(server_task());
    });
}

async fn server_task() {
    let (tx, _) = broadcast::channel::<String>(32);
    *BROADCASTER.lock().unwrap() = Some(tx.clone());
    let listener = TcpListener::bind("127.0.0.1:9876").await.expect("bind");
    loop {
        let (stream, _) = match listener.accept().await {
            Ok(s) => s,
            Err(_) => continue,
        };
        let tx = tx.clone();
        tokio::spawn(async move { handle(stream, tx).await; });
    }
}

async fn handle(mut stream: tokio::net::TcpStream, tx: broadcast::Sender<String>) {
    use sha1::{Sha1, Digest};
    use base64::Engine;

    // Read request line by line, collect into buf
    let mut buf = [0u8; 4096];
    let mut used = 0usize;
    loop {
        if used >= buf.len() { return; }
        match stream.read(&mut buf[used..used + 1]).await {
            Ok(0) | Err(_) => return,
            Ok(_) => {
                used += 1;
                if used >= 4 && buf[used - 4..used] == [b'\r', b'\n', b'\r', b'\n'] { break; }
                if used >= 2 && buf[used - 2..used] == [b'\n', b'\n'] { break; }
            }
        }
    }

    let request = String::from_utf8_lossy(&buf[..used]);
    let is_ws = request.to_lowercase().contains("upgrade: websocket");

    if is_ws {
        let key = request.lines()
            .find(|l| l.to_lowercase().starts_with("sec-websocket-key:"))
            .and_then(|l| l.split(':').nth(1))
            .map(|s| s.trim())
            .unwrap_or("");
        if key.is_empty() { return; }

        let accept = {
            let mut h = Sha1::new();
            h.update(key.as_bytes());
            h.update(b"258EAFA5-E914-47DA-95CA-5AB5C5D5EFB1");
            base64::engine::general_purpose::STANDARD.encode(h.finalize())
        };

        let upgrade = format!(
            "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {}\r\n\r\n",
            accept
        );
        if stream.write_all(upgrade.as_bytes()).await.is_err() { return; }

        let ws = WsStream::from_raw_socket(stream, Role::Server, None).await;
        handle_ws(ws, tx).await;
    } else {
        let resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
            INDEX_HTML.len(), INDEX_HTML
        );
        stream.write_all(resp.as_bytes()).await.ok();
    }
}

async fn handle_ws(mut ws: WsStream<tokio::net::TcpStream>, tx: broadcast::Sender<String>) {
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
