# Alba Agent

**Alba Agent** (`nexus-albaai` core engine + `alba-agent-desktop` Tauri v2 desktop application) is a local-first, zero-external-dependency AI workflow and desktop assistant written in Rust, Next.js 15, TypeScript, and SQLite. It packages as standalone native macOS binaries (`.app` and `.dmg`) with embedded frontend assets, requiring no Python runtime, Node.js daemon processes, external databases, or Docker containers.

---

## Release Artifacts (macOS aarch64)

The release build and disk image are packaged and ad-hoc code signed:

- **macOS Application Bundle (`.app`):**
  `target/release/bundle/macos/Alba Agent.app`
- **macOS Disk Image Installer (`.dmg`):**
  `target/release/bundle/dmg/Alba Agent_0.1.0_aarch64.dmg`

### Installation & Launch

1. **Via Disk Image (.dmg):**
   - Double-click `target/release/bundle/dmg/Alba Agent_0.1.0_aarch64.dmg` to mount the disk image.
   - Drag `Alba Agent.app` into `/Applications`.
   - Launch `Alba Agent` from Applications or Spotlight.
2. **Direct Execution:**
   - Double-click `target/release/bundle/macos/Alba Agent.app` or run from terminal:
     ```bash
     open "target/release/bundle/macos/Alba Agent.app"
     ```
3. **Standalone Relocation:**
   - The application bundle is completely self-contained. Static frontend assets from `web/out/` are embedded inside `Contents/Resources/out/`. The app launches and functions properly when moved anywhere on the system (e.g. `/tmp/test-alba-app/`) without depending on the source tree, Node, or development servers.

---

## Provider Configuration & Supported Formats

Alba Agent supports generic, custom, and official OpenAI-compatible LLM endpoints.

### 1. Flexible Custom Provider Support
You can connect to any third-party provider or local proxy by providing:
- **Base URL:** Any valid HTTP/HTTPS endpoint. Examples:
  - **OpenRouter:** `https://openrouter.ai/api/v1`
  - **ShopAPIKey:** `https://api.shopaikey.com/v1`
  - **Local Ollama:** `http://127.0.0.1:11434/v1`
  - **vLLM / LocalAI / LM Studio:** `http://127.0.0.1:8000/v1`
  - **Official OpenAI:** `https://api.openai.com/v1`
  - Any custom corporate proxy or gateway
- **API Key:** Stored securely in the **macOS Keychain** under service name `"alba"`. Keys are never logged in plaintext or exposed in API error messages.
- **Model IDs:** Enter any comma-separated model identifiers (e.g. `gpt-4o`, `anthropic/claude-3.5-sonnet`, `meta-llama/llama-3.3-70b`). Alba Agent preserves the exact user-specified model strings and will never overwrite them with `"default"` or dummy fallbacks.

### 2. Supported API Protocol Format
Alba Agent natively communicates via the **standard OpenAI Chat Completions protocol**:
- Endpoint: `POST {base_url}/chat/completions`
- Authentication header: `Authorization: Bearer <api_key>`
- Standard request body with `model`, `messages`, `tools`, and `tool_choice`.
- Streaming responses via Server-Sent Events (`text/event-stream`).

Services like **OpenRouter**, **ShopAPIKey**, and local inference runners (vLLM, Ollama) that adhere to this standard chat completions schema are fully supported without custom provider plugins.

### 3. Honest Connection Status & Secret Redaction
- **Initial Status:** Newly saved credentials begin in `UNCONFIGURED` state. They never display `"Đã kết nối"` (Connected) prematurely.
- **Connection Test:** Clicking "Kiểm tra kết nối" (Test Connection) performs an actual health check against the upstream provider endpoint using the configured model ID.
- **State Transition:** The status transitions to `CONNECTED` **only** if the upstream service returns HTTP 200 OK.
- **Error Handling:** If upstream returns an error (401 Unauthorized, 404 Model Not Found, 429 Rate Limit, or network timeout), the status transitions to `ERROR`. The error message is sanitized (`safe_error`) to strip API keys, Bearer tokens, and sensitive headers before displaying in the UI or writing to logs.

---

## Building & Packaging from Source

### Prerequisites
- macOS 13+ (Apple Silicon or Intel)
- Rust 1.80+ (`cargo`)
- Node.js v20+ / v24+ and `pnpm`
- Xcode Command Line Tools (`codesign`)

### 1. Build Web Frontend
```bash
export PATH="$HOME/.nvm/versions/node/v24.15.0/bin:$HOME/.cargo/bin:/opt/homebrew/bin:$PATH"
pnpm --prefix web install
pnpm --prefix web build
```
This exports the static frontend to `web/out/`.

### 2. Package macOS Release (.app & .dmg)
```bash
npx @tauri-apps/cli build --bundles app,dmg --no-sign
```

### 3. Ad-Hoc Code Signing & Verification
```bash
# Sign the macOS application bundle
codesign -s - --force --deep --options runtime "target/release/bundle/macos/Alba Agent.app"

# Sign the disk image
codesign -s - --force "target/release/bundle/dmg/Alba Agent_0.1.0_aarch64.dmg"

# Verify signature
codesign --verify --deep --strict --verbose=2 "target/release/bundle/macos/Alba Agent.app"
codesign --verify --verbose=2 "target/release/bundle/dmg/Alba Agent_0.1.0_aarch64.dmg"
```

---

## Live Provider Verification

When an active API key is available, you can verify real provider completions against any endpoint:

### Via Settings UI
1. Launch `Alba Agent.app`.
2. Navigate to **Cài đặt (Settings) -> Nhà cung cấp (Providers)**.
3. Select **Custom / OpenAI-compatible** or **ShopAPIKey**.
4. Enter your Base URL, API key, and Model ID (e.g. `gpt-4o-mini` or `anthropic/claude-3.5-sonnet`).
5. Click **Kiểm tra kết nối (Test Connection)** to verify HTTP connectivity and auth.
6. Open **Studio** and run a prompt to verify end-to-end execution.

### Via Headless CLI / Server API
To test against an upstream provider using the CLI or Axum API:
```bash
# 1. Start headless engine with your live key
OPENAI_BASE_URL="https://api.shopaikey.com/v1" \
OPENAI_API_KEY="sk-..." \
OPENAI_MODEL="gpt-4o-mini" \
cargo run --release --bin nexus-albaai -- run \
  --prompt "What is 2+2? Answer in one word." \
  --workspace-root ./workspace \
  --model gpt-4o-mini \
  --db-path test_run.db
```

*Note: In offline test environments without live provider API keys, all automated behavioral tests run against local mock servers and hermetic fixtures.*

---

## Automated Verification Suite

Run the full verification suite across Rust and Frontend:

```bash
# Verify no comments policy
./scripts/check_no_comments.sh

# Run all backend unit and integration tests
cargo test

# Run macOS release packaging test
cargo test --test g6_packaging_s5_tests -- --nocapture

# Run frontend tests
pnpm --prefix web test
```
