use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query,
    },
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    let app = Router::new()
        .route("/api/agent/run", post(run_agent))
        .route("/api/agent/interactive", get(interactive_agent));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

#[derive(Deserialize)]
struct RunAgentRequest {
    agent: String,
    prompt: String,
}

#[derive(Deserialize)]
struct InteractiveAgentParams {
    agent: String,
    prompt: String,
}

async fn run_agent(Json(payload): Json<RunAgentRequest>) -> impl IntoResponse {
    let (cmd_name, args) = match build_command(&payload.agent, &payload.prompt, false) {
        Ok(res) => res,
        Err(e) => return (StatusCode::BAD_REQUEST, e).into_response(),
    };

    println!("Running non-interactive: {} {:?}", cmd_name, args);

    let output = Command::new(&cmd_name)
        .args(&args)
        .output()
        .await;

    match output {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            // Combine stdout and stderr or just return stdout depending on preference.
            // For now, let's return combined if stderr exists.
            let result = if !stderr.is_empty() {
                format!("{}\nSTDERR:\n{}", stdout, stderr)
            } else {
                stdout
            };
            (StatusCode::OK, result).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn interactive_agent(
    ws: WebSocketUpgrade,
    Query(params): Query<InteractiveAgentParams>,
) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, params))
}

async fn handle_socket(mut socket: WebSocket, params: InteractiveAgentParams) {
    let (cmd_name, args) = match build_command(&params.agent, &params.prompt, true) {
        Ok(res) => res,
        Err(e) => {
            let _ = socket.send(Message::Text(format!("Error: {}", e))).await;
            return;
        }
    };

    println!("Running interactive: {} {:?}", cmd_name, args);

    let mut child = Command::new(&cmd_name)
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped()) // Capture stderr too
        .spawn()
        .expect("failed to spawn command");

    let mut stdin = child.stdin.take().expect("failed to open stdin");
    let stdout = child.stdout.take().expect("failed to open stdout");
    let stderr = child.stderr.take().expect("failed to open stderr");

    let mut stdout_reader = BufReader::new(stdout).lines();
    let mut stderr_reader = BufReader::new(stderr).lines();

    loop {
        tokio::select! {
            // Read from WebSocket and write to Process Stdin
            Some(msg) = socket.recv() => {
                match msg {
                    Ok(Message::Text(text)) => {
                        if let Err(e) = stdin.write_all(text.as_bytes()).await {
                            eprintln!("Failed to write to stdin: {}", e);
                            break;
                        }
                        // Add newline if not present, as CLI usually expects it
                        if !text.ends_with('\n') {
                             if let Err(e) = stdin.write_all(b"\n").await {
                                eprintln!("Failed to write newline to stdin: {}", e);
                                break;
                            }
                        }
                        if let Err(e) = stdin.flush().await {
                             eprintln!("Failed to flush stdin: {}", e);
                             break;
                        }
                    }
                    Ok(Message::Close(_)) => break,
                    Err(_) => break,
                    _ => {}
                }
            }
            // Read from Process Stdout and write to WebSocket
            Ok(Some(line)) = stdout_reader.next_line() => {
                 if let Err(e) = socket.send(Message::Text(format!("{}\n", line))).await {
                     eprintln!("Failed to send stdout to websocket: {}", e);
                     break;
                 }
            }
            // Read from Process Stderr and write to WebSocket (maybe prefix?)
            Ok(Some(line)) = stderr_reader.next_line() => {
                 if let Err(e) = socket.send(Message::Text(format!("ERR: {}\n", line))).await {
                     eprintln!("Failed to send stderr to websocket: {}", e);
                     break;
                 }
            }
            // Check if child process has exited
            status = child.wait() => {
                println!("Child process exited: {:?}", status);
                break;
            }
        }
    }
}

fn build_command(agent: &str, prompt: &str, interactive: bool) -> Result<(String, Vec<String>), String> {
    match (agent, interactive) {
        // Non-interactive
        ("gemini", false) => Ok(("gemini".to_string(), vec![prompt.to_string(), "-y".to_string()])),
        ("qwen", false) => Ok(("qwen".to_string(), vec![prompt.to_string(), "-y".to_string()])),
        ("codex", false) => Ok(("codex".to_string(), vec!["exec".to_string(), prompt.to_string(), "--full-auto".to_string()])),

        // Interactive
        ("gemini", true) => Ok(("gemini".to_string(), vec!["-i".to_string(), prompt.to_string(), "-y".to_string()])),
        ("qwen", true) => Ok(("qwen".to_string(), vec![prompt.to_string(), "-y".to_string()])),
        ("codex", true) => Ok(("codex".to_string(), vec![prompt.to_string(), "--full-auto".to_string()])),

        _ => Err(format!("Unknown agent: {}", agent)),
    }
}
