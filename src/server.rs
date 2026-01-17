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
    var reconnectInterval = 1000;
    var maxReconnectInterval = 5000;
    var reconnecting = false;
    var isConnecting = false;

    function connect() {
        if (isConnecting) return;
        isConnecting = true;

        var ws;
        try {
            ws = new WebSocket('ws://' + location.host + '/__rs_web_live_reload');
        } catch (e) {
            isConnecting = false;
            scheduleReconnect();
            return;
        }

        ws.onopen = function() {
            console.log('[rs-web] Live reload connected');
            isConnecting = false;
            reconnectInterval = 1000;
            if (reconnecting) {
                // Server is back - verify page is ready then reload
                fetch(location.href, { method: 'HEAD', cache: 'no-store' })
                    .then(function(resp) {
                        if (resp.ok) {
                            location.reload();
                        } else {
                            scheduleReconnect();
                        }
                    })
                    .catch(function() {
                        scheduleReconnect();
                    });
            }
        };

        ws.onmessage = function(event) {
            console.log('[rs-web] Received:', event.data);
            var msg = JSON.parse(event.data);
            if (msg.type === 'reload') {
                console.log('[rs-web] Reloading page...');
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
            isConnecting = false;
            if (!reconnecting) {
                console.log('[rs-web] Live reload disconnected');
            }
            reconnecting = true;
            scheduleReconnect();
        };

        ws.onerror = function() {
            // Let onclose handle reconnection
        };
    }

    function scheduleReconnect() {
        setTimeout(function() {
            reconnectInterval = Math.min(reconnectInterval * 1.5, maxReconnectInterval);
            connect();
        }, reconnectInterval);
    }

    connect();
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
    log::debug!(
        "[WS] Client connected. Total receivers: {}",
        state.reload_tx.receiver_count()
    );

    loop {
        tokio::select! {
            biased;

            // Handle incoming messages (ping/pong) - check this first
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Ping(data))) => {
                        if socket.send(Message::Pong(data)).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Pong(_))) => {}
                    Some(Ok(Message::Close(_))) => break,
                    Some(Ok(_)) => {}
                    Some(Err(e)) => {
                        log::debug!("[WS] Receive error: {}", e);
                        break;
                    }
                    None => {
                        log::debug!("[WS] Connection closed by client");
                        break;
                    }
                }
            }

            // Receive reload notifications
            result = rx.recv() => {
                match result {
                    Ok(msg) => {
                        let json = match msg {
                            ReloadMessage::Reload => r#"{"type":"reload"}"#.to_string(),
                            ReloadMessage::CssReload(path) => {
                                format!(r#"{{"type":"css","path":"{}"}}"#, path)
                            }
                        };
                        log::debug!("[WS] Sending: {}", json);
                        if socket.send(Message::Text(json.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        log::debug!("[WS] Broadcast recv error: {}", e);
                    }
                }
            }
        }
    }
    log::debug!(
        "[WS] Client disconnected. Remaining receivers: {}",
        state.reload_tx.receiver_count()
    );
}

/// Middleware to inject live reload script into HTML responses
async fn inject_live_reload(request: Request<Body>, next: axum::middleware::Next) -> Response {
    // Skip for WebSocket upgrade requests
    if request.headers().contains_key(header::UPGRADE) {
        return next.run(request).await;
    }

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

/// Try to bind to a port, returns the listener and actual port used
async fn try_bind(
    host: &str,
    start_port: u16,
    max_attempts: u16,
) -> anyhow::Result<(tokio::net::TcpListener, u16)> {
    for offset in 0..max_attempts {
        let port = start_port + offset;
        let addr: SocketAddr = format!("{}:{}", host, port).parse()?;

        match tokio::net::TcpListener::bind(addr).await {
            Ok(listener) => {
                if offset > 0 {
                    rs_print!(
                        "⚠ Port {} in use, using port {} instead (another rs-web may be running)",
                        start_port,
                        port
                    );
                }
                return Ok((listener, port));
            }
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
                continue;
            }
            Err(e) => {
                return Err(e.into());
            }
        }
    }

    anyhow::bail!(
        "Could not find available port (tried {} to {})",
        start_port,
        start_port + max_attempts - 1
    )
}

/// Run the development server
pub async fn run_server(config: ServerConfig) -> anyhow::Result<broadcast::Sender<ReloadMessage>> {
    let (reload_tx, _) = broadcast::channel::<ReloadMessage>(16);

    let state = Arc::new(ServerState {
        output_dir: config.output_dir.clone(),
        reload_tx: reload_tx.clone(),
    });

    let app = create_router(state);

    // Try to find an available port (up to 10 attempts)
    let (listener, actual_port) = try_bind(&config.host, config.port, 10).await?;

    rs_print!(
        "Development server running at http://{}:{}",
        config.host,
        actual_port
    );
    rs_print!("Serving: {}", config.output_dir.display());
    rs_print!("Live reload: enabled");

    tokio::spawn(async move {
        axum::serve(listener, app).await.ok();
    });

    Ok(reload_tx)
}

/// Notify clients to reload
pub fn notify_reload(tx: &broadcast::Sender<ReloadMessage>, message: ReloadMessage) {
    let receivers = tx.receiver_count();
    log::debug!("Sending reload to {} receivers", receivers);
    match tx.send(message) {
        Ok(n) => log::debug!("Sent to {} receivers", n),
        Err(e) => log::debug!("No receivers for reload message: {}", e),
    }
}
