# vibevoice-tts-provider

A gRPC TTS (Text-to-Speech) provider service that wraps the VibeVoice API, enabling streaming speech synthesis with real-time monitoring via a terminal UI.

## Features

- **gRPC streaming API** - Returns audio chunks as they're generated for low-latency playback
- **Voice cloning support** - Use custom voice samples via VibeVoice's voice cloning
- **Terminal UI** - Real-time monitoring of active streams, request history, and statistics
- **Headless mode** - Run without TUI for containerized/daemon deployments
- **Configurable voices** - Define multiple voices with custom display names

## Prerequisites

- Rust 1.70+
- A running [VibeVoice](https://github.com/yourusername/vibevoice) instance (or compatible API)
- Voice sample files (`.wav`) for voice cloning

## Building

```bash
cargo build --release
```

## Configuration

Create a `config.toml` file:

```toml
[server]
listen_addr = "[::1]:50051"

[vibevoice]
api_url = "http://127.0.0.1:8000/v1"
model = "vibevoice/VibeVoice-7B"
ddpm_steps = 20
cfg_scale = 3.0

[voices]
default = "primavera"

[voices.primavera]
path = "primavera.wav"
display_name = "Primavera"

[voices.narrator]
path = "/path/to/narrator.wav"
display_name = "Narrator (Deep Male)"
```

### Configuration Options

| Section | Key | Description |
|---------|-----|-------------|
| `server` | `listen_addr` | gRPC server bind address |
| `vibevoice` | `api_url` | VibeVoice API base URL |
| `vibevoice` | `model` | Model identifier |
| `vibevoice` | `ddpm_steps` | Diffusion steps (higher = better quality, slower) |
| `vibevoice` | `cfg_scale` | Classifier-free guidance scale |
| `voices` | `default` | Default voice ID when none specified |
| `voices.<id>` | `path` | Path to voice sample file |
| `voices.<id>` | `display_name` | Human-readable name shown in TUI |

## Usage

### With Terminal UI (default)

```bash
./target/release/vibevoice-tts-provider config.toml
```

The TUI shows:
- Server status and uptime
- Configured voices
- Active synthesis streams
- Request history with timing
- Aggregate statistics

**Keyboard shortcuts:**
- `Q` - Quit
- `C` - Clear request log

### Headless Mode

```bash
./target/release/vibevoice-tts-provider --no-tui config.toml
```

Logs to stdout via `tracing`. Set `RUST_LOG` for log level control.

## gRPC API

The service implements `orchestrator.v1.TtsService`:

```protobuf
service TtsService {
  rpc Synthesize(SynthesizeRequest) returns (stream AudioChunk);
}

message SynthesizeRequest {
  string text = 1;
  string voice_id = 2;
  AudioConfig audio_config = 3;
}

message AudioChunk {
  repeated float samples = 1;  // PCM f32, normalized -1.0 to 1.0
  uint32 sequence = 2;
  bool is_final = 3;
}
```

### Example Client (grpcurl)

```bash
grpcurl -plaintext -d '{
  "text": "Hello, world!",
  "voice_id": "primavera"
}' localhost:50051 orchestrator.v1.TtsService/Synthesize
```

## Architecture

```
┌─────────────────┐     gRPC      ┌──────────────────┐     HTTP/SSE     ┌──────────────┐
│  gRPC Client    │──────────────▶│  TtsServiceImpl  │─────────────────▶│  VibeVoice   │
└─────────────────┘               └──────────────────┘                  │    API       │
                                          │                             └──────────────┘
                                          │ events
                                          ▼
                                  ┌──────────────────┐
                                  │    Terminal UI   │
                                  │  (ratatui/tui)   │
                                  └──────────────────┘
```

The service:
1. Receives gRPC `Synthesize` requests
2. Looks up voice configuration
3. Forwards to VibeVoice API via HTTP with SSE streaming
4. Converts audio chunks (i16 PCM → f32 normalized)
5. Streams back to client via gRPC
6. Updates TUI with real-time progress

## License

MIT
