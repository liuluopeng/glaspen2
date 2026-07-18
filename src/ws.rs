use std::sync::Mutex;
use tokio::sync::broadcast;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream as WsStream;
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;
use std::io::{Read, Write};

static BROADCASTER: Mutex<Option<broadcast::Sender<String>>> = Mutex::new(None);

const PAGE: &str = r#"<!DOCTYPE html>
<html><head><meta charset="utf-8"><title>你画我猜</title>
<style>
*{margin:0;padding:0;box-sizing:border-box}
body{background:#222;display:flex;justify-content:center;align-items:center;height:100vh;font-family:sans-serif;color:#fff}
#container{text-align:center}
canvas{max-width:100vw;max-height:90vh;border:1px solid #444;background:#fff}
#status{padding:8px;font-size:14px;color:#fff}
</style></head><body>
<div id=container>
<canvas id=c width=1200 height=800></canvas>
<div id=status>Connecting...</div>
</div>
<script>
let ws = new WebSocket('ws://localhost:9876');
let cv = document.getElementById('c'), cx = cv.getContext('2d');
let st = document.getElementById('status');
let curX, curY, curW, curR, curG, curB;
cx.lineCap = 'round'; cx.lineJoin = 'round';

ws.onopen = () => st.textContent = 'Connected';
ws.onmessage = (e) => {
    let d = JSON.parse(e.data);
    if (d.t === 'd') { curX=d.x; curY=d.y; curW=d.w; curR=d.r; curG=d.g; curB=d.b; }
    if (d.t === 'm') {
        cx.strokeStyle = 'rgb('+(curR*255|0)+','+(curG*255|0)+','+(curB*255|0)+')';
        cx.lineWidth = curW;
        cx.beginPath(); cx.moveTo(curX, curY); cx.lineTo(d.x, d.y); cx.stroke();
        curX = d.x; curY = d.y;
    }
};
ws.onclose = () => st.textContent = 'Disconnected';
</script></body></html>"#;

pub fn start_server() {
    // Simple HTTP server thread (serves the HTML page)
    std::thread::spawn(|| {
        let listener = std::net::TcpListener::bind("127.0.0.1:9875").unwrap();
        for stream in listener.incoming() {
            if let Ok(mut s) = stream {
                let mut buf = [0u8; 4096];
                s.read(&mut buf).ok();
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
                    PAGE.len(), PAGE
                );
                s.write_all(resp.as_bytes()).ok();
            }
        }
    });

    // WebSocket server on tokio
    std::thread::spawn(|| {
        let rt = tokio::runtime::Runtime::new().expect("WS runtime");
        rt.block_on(async {
            let (tx, _) = broadcast::channel::<String>(32);
            *BROADCASTER.lock().unwrap() = Some(tx.clone());
            let listener = TcpListener::bind("127.0.0.1:9876").await.expect("bind WS");
            while let Ok((stream, _)) = listener.accept().await {
                let tx = tx.clone();
                tokio::spawn(async move {
                    match accept_async(stream).await {
                        Ok(ws) => handle_ws(ws, tx).await,
                        Err(_) => {}
                    }
                });
            }
        });
    });

    eprintln!("[ws] open http://localhost:9875  and draw on glaspen2");
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

pub fn broadcast(msg: &str) {
    if let Some(tx) = BROADCASTER.lock().unwrap().as_ref() {
        tx.send(msg.to_string()).ok();
    }
}
