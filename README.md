# Nexus

Nexus is a Rust-based server designed to spawn and interact with CLI agents via a REST API. It supports both non-interactive execution and interactive sessions using WebSockets.

## Features

- **Non-Interactive Execution**: Send a prompt to an agent and receive the full output.
- **Interactive Sessions**: Connect via WebSocket to interact with an agent in real-time (STDIN/STDOUT).
- **Supported Agents**:
  - `gemini`
  - `qwen`
  - `codex`

## Prerequisites

- Rust (latest stable)
- The CLI agents (`gemini`, `qwen`, `codex`) must be installed and available in the system PATH.

## Getting Started

1. **Clone the repository**:
   ```bash
   git clone <repository-url>
   cd nexus
   ```

2. **Run the server**:
   ```bash
   cargo run
   ```
   The server will start listening on `0.0.0.0:3000`.

## API Documentation

### 1. Run Agent (Non-Interactive)

Executes an agent command and returns the output.

- **Endpoint**: `POST /api/agent/run`
- **Content-Type**: `application/json`
- **Body**:
  ```json
  {
    "agent": "gemini",  // Options: "gemini", "qwen", "codex"
    "prompt": "Your prompt here"
  }
  ```

**Example**:
```bash
curl -X POST http://localhost:3000/api/agent/run \
  -H "Content-Type: application/json" \
  -d '{"agent": "gemini", "prompt": "Explain quantum computing"}'
```

### 2. Interactive Session

Starts an interactive session with an agent.

- **Endpoint**: `GET /api/agent/interactive` (WebSocket Upgrade)
- **Query Parameters**:
  - `agent`: The agent to run (`gemini`, `qwen`, `codex`)
  - `prompt`: The initial prompt to start the session with.

**Behavior**:
- **Send (Client -> Server)**: Text messages are piped to the agent's STDIN.
- **Receive (Server -> Client)**: The agent's STDOUT and STDERR are sent as text messages.

**Example** (using `wscat`):
```bash
wscat -c "ws://localhost:3000/api/agent/interactive?agent=gemini&prompt=Hello"
```
