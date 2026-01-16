//! Development server with WebSocket live reload
//!
//! Provides a static file server with automatic reload when files change.

use axum::{
    Router,
    body::Body,
    extract::{
        State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::{Request, header},
    response::{IntoResponse, Response},
    routing::get,
};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::broadcast;
use tower_http::services::ServeDir;

/// Reload message sent to connected clients
#[derive(Debug, Clone)]
pub enum ReloadMessage {
    /// Full page reload
    Reload,
    /// CSS-only reload (hot reload)
    CssReload(String),
}

/// Server state shared across handlers
pub struct ServerState {
    /// Output directory to serve
    pub output_dir: PathBuf,
    /// Broadcast channel for reload notifications
    pub reload_tx: broadcast::Sender<ReloadMessage>,
}

/// Live reload JavaScript injected into HTML pages
const LIVE_RELOAD_SCRIPT: &str = r#"
<script>
(function() {
    var ws = new WebSocket('ws://' + location.host + '/__rs_web_live_reload');
    ws.onmessage = function(event) {
        var msg = JSON.parse(event.data);
        if (msg.type === 'reload') {
            location.reload();
        } else if (msg.type === 'css') {
            // Hot reload CSS
            var links = document.querySelectorAll('link[rel="stylesheet"]');
            links.forEach(function(link) {
                var href = link.getAttribute('href');
                if (href) {
                    var url = new URL(href, location.href);
                    url.searchParams.set('_reload', Date.now());
                    link.setAttribute('href', url.toString());
                }
            });
        }
    };
    ws.onclose = function() {
        console.log('[rs-web] Live reload disconnected. Attempting reconnect...');
        setTimeout(function() { location.reload(); }, 1000);
    };
    ws.onerror = function() {
        console.log('[rs-web] Live reload connection error');
    };
})();
</script>
"#;

/// Create the server router
pub fn create_router(state: Arc<ServerState>) -> Router {
    // Static file serving with live reload injection
    let serve_dir = ServeDir::new(&state.output_dir);

    Router::new()
        .route("/__rs_web_live_reload", get(websocket_handler))
        .fallback_service(serve_dir)
        .with_state(state)
        .layer(axum::middleware::from_fn(inject_live_reload))
}

/// WebSocket handler for live reload connections
async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<ServerState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

/// Handle WebSocket connection
async fn handle_socket(mut socket: WebSocket, state: Arc<ServerState>) {
    let mut rx = state.reload_tx.subscribe();

    loop {
        tokio::select! {
            // Receive reload notifications
            Ok(msg) = rx.recv() => {
                let json = match msg {
                    ReloadMessage::Reload => r#"{"type":"reload"}"#.to_string(),
                    ReloadMessage::CssReload(path) => {
                        format!(r#"{{"type":"css","path":"{}"}}"#, path)
                    }
                };
                if socket.send(Message::Text(json.into())).await.is_err() {
                    break;
                }
            }
            // Handle incoming messages (ping/pong)
            Some(Ok(msg)) = socket.recv() => {
                match msg {
                    Message::Ping(data) => {
                        if socket.send(Message::Pong(data)).await.is_err() {
                            break;
                        }
                    }
                    Message::Close(_) => break,
                    _ => {}
                }
            }
            else => break,
        }
    }
}

/// Middleware to inject live reload script into HTML responses
async fn inject_live_reload(request: Request<Body>, next: axum::middleware::Next) -> Response {
    let response = next.run(request).await;

    // Check if response is HTML
    let is_html = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|ct| ct.starts_with("text/html"))
        .unwrap_or(false);

    if !is_html {
        return response;
    }

    // Extract body and inject script
    let (mut parts, body) = response.into_parts();
    let bytes = match axum::body::to_bytes(body, usize::MAX).await {
        Ok(b) => b,
        Err(_) => return Response::from_parts(parts, Body::empty()),
    };

    let html = String::from_utf8_lossy(&bytes);
    let modified = if html.contains("</body>") {
        html.replace("</body>", &format!("{}</body>", LIVE_RELOAD_SCRIPT))
    } else if html.contains("</html>") {
        html.replace("</html>", &format!("{}</html>", LIVE_RELOAD_SCRIPT))
    } else {
        format!("{}{}", html, LIVE_RELOAD_SCRIPT)
    };

    // Update Content-Length header to match new body
    let new_len = modified.len();
    parts.headers.remove(header::CONTENT_LENGTH);
    parts.headers.insert(
        header::CONTENT_LENGTH,
        header::HeaderValue::from_str(&new_len.to_string()).unwrap(),
    );

    Response::from_parts(parts, Body::from(modified))
}

/// Server configuration
pub struct ServerConfig {
    pub port: u16,
    pub host: String,
    pub output_dir: PathBuf,
}

/// Run the development server
pub async fn run_server(config: ServerConfig) -> anyhow::Result<broadcast::Sender<ReloadMessage>> {
    let (reload_tx, _) = broadcast::channel::<ReloadMessage>(16);

    let state = Arc::new(ServerState {
        output_dir: config.output_dir.clone(),
        reload_tx: reload_tx.clone(),
    });

    let app = create_router(state);

    let addr: SocketAddr = format!("{}:{}", config.host, config.port).parse()?;

    println!(
        "Development server running at http://{}:{}",
        config.host, config.port
    );
    println!("Serving: {}", config.output_dir.display());
    println!("Live reload: enabled");
    println!();

    let listener = tokio::net::TcpListener::bind(addr).await?;

    tokio::spawn(async move {
        axum::serve(listener, app).await.ok();
    });

    Ok(reload_tx)
}

/// Notify clients to reload
pub fn notify_reload(tx: &broadcast::Sender<ReloadMessage>, message: ReloadMessage) {
    let _ = tx.send(message);
}
