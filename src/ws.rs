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
body{background:#1a1a1a;display:flex;justify-content:center;align-items:center;height:100vh;font-family:sans-serif}
canvas{max-width:calc(100vw - 40px);max-height:calc(100vh - 80px);border-radius:8px;box-shadow:0 4px 24px rgba(0,0,0,.5);background:#fff}
#s{position:fixed;bottom:16px;left:50%;transform:translateX(-50%);color:#666;font-size:13px}
</style></head><body>
<canvas id=c width=3440 height=1440></canvas>
<div id=s>Connected</div>
<script>
let ws=new WebSocket('ws://localhost:9876'),c=document.getElementById('c'),cx=c.getContext('2d'),st=document.getElementById('s');
let x,y,r,g,b;
cx.lineCap='round';cx.lineJoin='round';
ws.onopen=()=>console.log('WS connected');
ws.onerror=()=>st.textContent='Connection failed';
ws.onclose=()=>st.textContent='Disconnected';
ws.onmessage=e=>{let d=JSON.parse(e.data);if(d.t=='d'){x=d.x;y=d.y;r=d.r;g=d.g;b=d.b;}else if(d.t=='m'){cx.strokeStyle='rgb('+(r*255|0)+','+(g*255|0)+','+(b*255|0)+')';cx.lineWidth=Math.max(d.w,1);cx.beginPath();cx.moveTo(x,y);cx.lineTo(d.x,d.y);cx.stroke();x=d.x;y=d.y;}};
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
            eprintln!("[ws] WS server ready on port 9876");
            while let Ok((stream, peer)) = listener.accept().await {
                eprintln!("[ws] connection from {}", peer);
                let tx = tx.clone();
                tokio::spawn(async move {
                    match accept_async(stream).await {
                        Ok(ws) => { eprintln!("[ws] WS upgraded OK"); handle_ws(ws, tx).await; }
                        Err(e) => eprintln!("[ws] WS upgrade failed: {}", e),
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
