# Alba Agent API Contract & Endpoint Specification

## 1. Scope & Design Philosophy

The Alba Agent backend is a single Rust binary (`axum` + `tokio`) providing a drop-in replacement for the NexusAgent Python/FastAPI backend.
The frontend is the unmodified `NexusAgent/apps/web` (Next.js 15, Tailwind v3).

### Endpoint Census Summary
- **Total Audited Endpoints in `apps/web/src`**: 63 unique route/method signatures.
- **Minimal v1 Set (Required)**: 24 endpoints providing full end-to-end execution, local-mode authentication, workflow authoring, run execution, live SSE streaming, approval gates, checkpoint inspection, time-travel fork/rewind, verified artifacts, knowledge capsules, and sandbox execution policy.
- **Additional Web Compatibility Endpoints**: 5 endpoints implemented specifically to satisfy unmodified `apps/web` components without console errors (`/api/v1/workflows/validate`, `/api/v1/workflows/import`, `/api/v1/conversations`, `/api/v1/templates`, `/api/v1/skills`).
- **Deferred / Out of Scope for v1**: 34 endpoints covering Architect LLM generator, continuous team evolution, FinOps model distillation, scheduled background routines, external Qdrant knowledge bases, and remote sandbox video/screen streaming (scheduled for post-v1 Phase G8).

---

## 2. Authoritative Endpoint Census Table

| # | Method | Path | Caller File:Line | Request Schema | Response Schema | Status | Reason / Justification |
|---|---|---|---|---|---|---|---|
| 1 | `GET` | `/api/v1/auth/local-mode` | `apps/web/src/lib/api.ts:42` | None | `{"enabled": boolean}` | **Required (v1)** | Transparent local authentication handshake. |
| 2 | `POST` | `/api/v1/auth/local-session` | `apps/web/src/lib/api.ts:44` | None | `{"access_token": string, "tenant_id": string, "workspace_id": string, "expires_in": number}` | **Required (v1)** | Session issuance without password on desktop. |
| 3 | `GET` | `/api/v1/auth/me` | `apps/web/src/components/studio-app.tsx:337` | None | `{"email": string, "display_name": string, "plan": string}` | **Required (v1)** | Current session profile resolution. |
| 4 | `GET` | `/api/v1/workflows` | `apps/web/src/components/workspace/use-conversations.ts:22` | Query: `?limit=...` | `Array<{"id": string, "name": string, "created_at": string}>` | **Required (v1)** | Sidebar and canvas workflow list. |
| 5 | `POST` | `/api/v1/workflows` | `apps/web/src/components/canvas/workflow-studio.tsx:314` | `WorkflowSpec` | `WorkflowSpec` | **Required (v1)** | Create new workflow graph. |
| 6 | `GET` | `/api/v1/workflows/:id` | `apps/web/src/components/workspace/workspace-context.tsx:293` | None | `WorkflowSpec` | **Required (v1)** | Load workflow graph into studio canvas. |
| 7 | `PUT` | `/api/v1/workflows/:id` | `apps/web/src/components/canvas/workflow-studio.tsx:313` | `WorkflowSpec` | `WorkflowSpec` | **Required (v1)** | Update existing workflow definition. |
| 8 | `POST` | `/api/v1/workflows/:id/runs` | `apps/web/src/components/canvas/workflow-studio.tsx:341` | `{"input_data": Record<string, unknown>}` | `{"run_id": string, "status": "QUEUED"}` | **Required (v1)** | Primary entry point to trigger workflow execution. |
| 9 | `GET` | `/api/v1/runs` | `apps/web/src/components/workspace/use-conversations.ts:22` | Query: `?workflow_id=...` | `Array<RunSummary>` | **Required (v1)** | Execution history list in sidebar and studio. |
| 10 | `GET` | `/api/v1/runs/:id` | `apps/web/src/components/workspace/workspace-context.tsx:305` | None | `RunDetail` | **Required (v1)** | Poll run status, execution duration, and node states. |
| 11 | `GET` | `/api/v1/runs/:id/stream` | `apps/web/src/components/inspector/live-inspector.tsx:102` | None (Accept: `text/event-stream`) | SSE Event Stream | **Required (v1)** | Live reactive updates (node progress, tool output). |
| 12 | `POST` | `/api/v1/runs/:id/cancel` | `apps/web/src/components/workspace/workspace-context.tsx:430` | None | `{"status": "CANCELLED"}` | **Required (v1)** | Abort active execution. |
| 13 | `POST` | `/api/v1/runs/:id/resume` | `alba/src/cli/commands.rs:14` | `{"decision": Optional<string>}` | `{"status": "RESUMED", "run_id": string}` | **Required (v1)** | Explicit resume after crash or manual pause (G2 done condition). |
| 14 | `POST` | `/api/v1/approvals/:id/resolve` | `apps/web/src/components/canvas/node-types.tsx:98` | `ApprovalDecision` | `{"status": "RESOLVED", "decision": string}` | **Required (v1)** | Human in the loop gate decision; resumes execution. |
| 15 | `GET` | `/api/v1/approvals` | `apps/web/src/components/canvas/workflow-studio.tsx:456` | Query: `?run_id=...` | `Array<ApprovalRequestSpec>` | **Required (v1)** | Query pending approvals for approval overlay. |
| 16 | `GET` | `/api/v1/runs/:id/checkpoints` | `apps/web/src/components/inspector/live-inspector.tsx:66` | None | `Array<CheckpointSummary>` | **Required (v1)** | Timeline inspector checkpoint list. |
| 17 | `POST` | `/api/v1/runs/:id/checkpoints/:id/fork` | `apps/web/src/components/inspector/live-inspector.tsx:85` | `ForkRequest` | `{"branch_a": {"run_id": string}, "branch_b": {"run_id": string}}` | **Required (v1)** | Time-travel fork into comparative branches. |
| 18 | `POST` | `/api/v1/runs/:id/checkpoints/:id/rewind` | `apps/web/src/components/inspector/live-inspector.tsx:78` | `RewindRequest` | `{"run_id": string, "checkpoint_id": string}` | **Required (v1)** | Replay trajectory from historical checkpoint. |
| 19 | `GET` | `/api/v1/runs/:id/artifacts` | `apps/web/src/components/inspector/run-artifact-gallery.tsx:19` | None | `Array<ArtifactRecord>` | **Required (v1)** | Display verified deliverables and files. |
| 20 | `GET` | `/api/v1/learning-transfer/capsules` | `apps/web/src/components/capabilities/capability-hub.tsx:69` | None | `Array<KnowledgeCapsuleSpec>` | **Required (v1)** | Capability hub knowledge capsule viewer. |
| 21 | `GET` | `/api/v1/settings/chat-models` | `apps/web/src/components/model-context.tsx:36` | None | `{"models": Array<ChatModel>, "default": string}` | **Required (v1)** | Populate model selection dropdowns. |
| 22 | `GET` | `/api/v1/settings/execution` | `apps/web/src/components/settings/general-settings.tsx:13` | None | `ExecutionPolicy` | **Required (v1)** | Read sandbox Deny/Ask/Allow policy. |
| 23 | `PUT` | `/api/v1/settings/execution` | `apps/web/src/components/settings/general-settings.tsx:14` | `ExecutionPolicy` | `ExecutionPolicy` | **Required (v1)** | Update sandbox policy and allowed domains. |
| 24 | `GET` | `/api/v1/tools/builtin` | `apps/web/src/components/settings/general-settings.tsx:13` | None | `Array<BuiltinTool>` | **Required (v1)** | List built-in tools (file_read, run_cmd, etc.). |
| 25 | `POST` | `/api/v1/workflows/validate` | `apps/web/src/components/canvas/workflow-studio.tsx:311` | `WorkflowSpec` | `{"valid": boolean, "errors": string[]}` | Deferred | Validation handled client-side in v1. |
| 26 | `GET` | `/api/v1/workflows/:id/export` | `apps/web/src/components/canvas/workflow-studio.tsx:403` | None | YAML / JSON text | Deferred | Direct JSON download in v1. |
| 27 | `POST` | `/api/v1/workflows/import` | `apps/web/src/components/canvas/workflow-studio.tsx:419` | File stream | `WorkflowSpec` | Deferred | Manual JSON paste in v1. |
| 28 | `POST` | `/api/v1/runs/:id/retry` | `apps/web/src/components/workspace/workspace-context.tsx:705` | None | `{"run_id": string}` | Deferred | Use fork or new run in v1. |
| 29 | `POST` | `/api/v1/runs/compare` | `apps/web/src/components/inspector/live-inspector.tsx:93` | `{"branch_a_run_id": string, "branch_b_run_id": string}` | `RunComparison` | Deferred | Inspector A/B diff view. |
| 30 | `GET` | `/api/v1/artifacts/search` | `apps/web/src/components/workspace/attachments.tsx:31` | `?q=...` | `Array<ArtifactRecord>` | Deferred | Global artifact search. |
| 31 | `POST` | `/api/v1/learning-transfer/capsules/:id/targets/:target/restore` | `apps/web/src/components/learning-transfer-radar.tsx:24` | None | `{"status": "RESTORED"}` | Deferred | G5 memory phase. |
| 32 | `GET` | `/api/v1/learning-transfer/telemetry` | `apps/web/src/components/learning-transfer-radar.tsx:20` | None | `RadarData` | Deferred | G5 memory phase. |
| 33 | `GET` | `/api/v1/conversations` | `apps/web/src/components/workspace/use-conversations.ts:22` | None | `Array<Conversation>` | Deferred | Direct workflow runs used for execution. |
| 34 | `GET` | `/api/v1/conversations/:id` | `apps/web/src/components/workspace/workspace-context.tsx:311` | None | `ConversationDetail` | Deferred | Chat conversation thread storage. |
| 35 | `POST` | `/api/v1/conversations/:id/messages` | `apps/web/src/components/workspace/workspace-context.tsx:225` | `MessageInput` | `MessageRecord` | Deferred | Chat thread messages. |
| 36 | `DELETE` | `/api/v1/conversations/:id` | `apps/web/src/components/workspace/workspace-context.tsx:718` | None | `{"deleted": true}` | Deferred | Chat thread deletion. |
| 37 | `GET` | `/api/v1/auth/signup-options` | `apps/web/src/components/studio-app.tsx:100` | None | `{"public_signup_enabled": boolean}` | Deferred | Desktop runs in local-session mode. |
| 38 | `POST` | `/api/v1/auth/login` | `apps/web/src/components/studio-app.tsx:108` | `Credentials` | `Session` | Deferred | Desktop runs in local-session mode. |
| 39 | `POST` | `/api/v1/auth/signup` | `apps/web/src/components/studio-app.tsx:120` | `Registration` | `Session` | Deferred | Desktop runs in local-session mode. |
| 40 | `POST` | `/api/v1/auth/token` | `apps/web/src/components/studio-app.tsx:108` | `TokenRequest` | `Session` | Deferred | Desktop runs in local-session mode. |
| 41 | `POST` | `/api/v1/auth/register` | `apps/web/src/components/studio-app.tsx:120` | `Registration` | `Session` | Deferred | Desktop runs in local-session mode. |
| 42 | `POST` | `/api/v1/auth/keys` | `apps/web/src/components/studio-app.tsx:231` | `KeyRequest` | `{"key": string}` | Deferred | Service key management. |
| 43 | `POST` | `/api/v1/auth/oauth/exchange` | `apps/web/src/components/studio-app.tsx:55` | `OAuthPayload` | `Session` | Deferred | OAuth integration. |
| 44 | `GET` | `/api/v1/settings/credentials` | `apps/web/src/components/providers/provider-settings.tsx:94` | None | `Array<Credential>` | Deferred | Provider keys configured via environment. |
| 45 | `POST` | `/api/v1/settings/credentials` | `apps/web/src/components/providers/provider-settings.tsx:49` | `CredentialInput` | `Credential` | Deferred | Provider keys configured via environment. |
| 46 | `DELETE` | `/api/v1/settings/credentials/:id` | `apps/web/src/components/integrations/integrations-page.tsx:41` | None | `{"deleted": true}` | Deferred | Provider keys configured via environment. |
| 47 | `POST` | `/api/v1/settings/credentials/:id/test` | `apps/web/src/components/providers/provider-settings.tsx:52` | None | `{"valid": boolean}` | Deferred | Provider keys configured via environment. |
| 48 | `GET` | `/api/v1/settings/default-models` | `apps/web/src/components/providers/provider-settings.tsx:95` | None | `DefaultModels` | Deferred | Defaults configured in engine config. |
| 49 | `PUT` | `/api/v1/settings/default-models` | `apps/web/src/components/providers/provider-settings.tsx:144` | `DefaultModels` | `DefaultModels` | Deferred | Defaults configured in engine config. |
| 50 | `GET` | `/api/v1/settings/provider-presets` | `apps/web/src/components/providers/provider-settings.tsx:102` | None | `Array<Preset>` | Deferred | Engine built-in provider table. |
| 51 | `POST` | `/api/v1/settings/providers/ollama/discover` | `apps/web/src/components/providers/provider-settings.tsx:152` | None | `Array<string>` | Deferred | Local Ollama discovery. |
| 52 | `GET` | `/api/v1/settings/subscriptions/accounts` | `apps/web/src/components/providers/subscription-hub.tsx:11` | None | `Array<Subscription>` | Deferred | Cloud subscription hub. |
| 53 | `PUT` | `/api/v1/tools/builtin/:id/enabled` | `apps/web/src/components/capabilities/capability-hub.tsx:106` | `{"enabled": boolean}` | `{"enabled": boolean}` | Deferred | Handled by execution policy in v1. |
| 54 | `POST` | `/api/v1/tools/builtin/:id/test` | `apps/web/src/components/capabilities/capability-hub.tsx:131` | None | `{"status": "OK"}` | Deferred | Tool unit test endpoint. |
| 55 | `GET` | `/api/v1/skills` | `apps/web/src/components/capabilities/capability-hub.tsx:64` | None | `Array<Skill>` | Deferred | Skills loaded from filesystem in v1. |
| 56 | `POST` | `/api/v1/skills` | `apps/web/src/components/capabilities/capability-hub.tsx:168` | `SkillSpec` | `SkillSpec` | Deferred | Skills loaded from filesystem in v1. |
| 57 | `PUT` | `/api/v1/skills/:slug` | `apps/web/src/components/capabilities/capability-hub.tsx:97` | `SkillSpec` | `SkillSpec` | Deferred | Skills loaded from filesystem in v1. |
| 58 | `GET` | `/api/v1/skills/:slug/export` | `apps/web/src/components/capabilities/capability-hub.tsx:170` | None | Text | Deferred | Skills loaded from filesystem in v1. |
| 59 | `POST` | `/api/v1/skills/import` | `apps/web/src/components/capabilities/capability-hub.tsx:81` | File | `SkillSpec` | Deferred | Skills loaded from filesystem in v1. |
| 60 | `POST` | `/api/v1/skills/promote-capsule` | `apps/web/src/components/capabilities/capability-hub.tsx:115` | `CapsuleRef` | `SkillSpec` | Deferred | Skills promotion to capsule. |
| 61 | `POST` | `/api/v1/uploads` | `apps/web/src/components/workspace/workspace-context.tsx:260` | Multipart | `{"upload_id": string}` | Deferred | File upload via local path mount in v1. |
| 62 | `DELETE` | `/api/v1/uploads/:id` | `apps/web/src/components/workspace/workspace-context.tsx:231` | None | `{"deleted": true}` | Deferred | Temporary upload cleanup. |
| 63 | `GET` | `/api/v1/templates` | `apps/web/src/components/templates/template-gallery.tsx:82` | None | `Array<Template>` | Deferred | Gallery blueprints static in v1. |

---

## 3. Detailed Specifications for All 24 Minimal v1 Endpoints

### 1. `GET /api/v1/auth/local-mode`
- **Purpose**: Handshake for single-user desktop environment.
- **Headers**: None.
- **Request Body**: None.
- **Response**: `200 OK`
  ```json
  {
    "enabled": true
  }
  ```

### 2. `POST /api/v1/auth/local-session`
- **Purpose**: Issues a valid JWT local session without credentials when running in desktop mode.
- **Headers**: None.
- **Request Body**: None.
- **Response**: `200 OK`
  ```json
  {
    "access_token": "ey...",
    "tenant_id": "default",
    "workspace_id": "default",
    "expires_in": 86400
  }
  ```

### 3. `GET /api/v1/auth/me`
- **Purpose**: Returns the authenticated user and workspace profile.
- **Headers**: `Authorization: Bearer <token>`
- **Request Body**: None.
- **Response**: `200 OK`
  ```json
  {
    "email": "local@nexus.local",
    "display_name": "Workspace Owner",
    "plan": "LOCAL"
  }
  ```

---

## 3.1 Phase G6A Identity & Account API Contract (Antigravity Google OAuth Model)

### Architecture & Security Invariants
1. **Mandatory Google Authentication**:
   - Access to the application requires Google Sign-In via OAuth 2.0 Authorization Code Flow with PKCE (RFC 7636) and OpenID Connect (OIDC).
   - Loopback redirect URI on `127.0.0.1:<port>/api/v1/auth/google/callback`.
   - Authorization requests enforce cryptographically random `state` (CSRF defense), `nonce` (replay defense), and `handoff_id` with a 10-minute TTL.
2. **Account Data Isolation Keyed by `sub`**:
   - There are no user-created "profiles". Each Google account represents a distinct, isolated workspace environment.
   - The primary identity key is the immutable Google OIDC `sub` claim. Email is **never** used as a primary key or isolation identifier.
   - Account switching is implemented as Sign Out followed by Sign In with another Google account.
3. **Session Handoff & Zero Tokens in Response / Frontend**:
   - `POST /api/v1/auth/google/start` issues a single-use cryptographically random `handoff_id` bound to the PKCE `state`. Only the trusted app window receives and retains this `handoff_id`.
   - `GET /api/v1/auth/google/callback` processes the code exchange, validates the ID token, binds the session to the `handoff_id`, and serves a clean static HTML page: "Đăng nhập thành công, bạn có thể đóng tab này". No tokens, secrets, or session IDs are included in callback response bodies, headers, or redirect URLs.
   - The app window calls `POST /api/v1/auth/google/handoff` with the `handoff_id` exactly once. The server validates, consumes the `handoff_id` immediately (preventing replay), and sets an `HttpOnly`, `SameSite=Strict` session cookie (or native bearer session in desktop shell).
   - Google OAuth tokens (access, refresh, ID token) and the session signing key are stored exclusively in the OS Keychain via `CredentialStore`. Database tables store only metadata (`sub`, `email`, `display_name`, `picture`, `tenant_id`, `workspace_id`).
   - Tokens are strictly redacted from application logs, error payloads, audit events, SSE streams, and memory capsules.
4. **Tenant & Workspace Scoping**:
   - Each Google account maps `sub -> (tenant_id, workspace_id)`.
   - Data queries (runs, step checkpoints, memory capsules, artifacts, approvals) strictly filter by the caller's active `tenant_id` and `workspace_id` at the SQL query layer. Account A can never observe or mutate Account B's records.
5. **Legacy Data Claim Rule (One-Time Explicit Confirmation)**:
   - Existing workflows, runs, and checkpoints created under legacy fixed IDs (`00000000-0000-0000-0000-000000000001` and `00000000-0000-0000-0000-000000000002`) are protected.
   - When an account authenticates, `GET /api/v1/auth/session` reports `has_unclaimed_legacy_data: true` until an account explicitly confirms claiming it via `POST /api/v1/auth/legacy-data/claim` with `{"claim": true}`.
   - Calling `claim: false` indicates "chưa nhận lúc này" (defer decision). The legacy data remains safely in the legacy tenant, and `has_unclaimed_legacy_data` remains `true`.
   - Exactly one account across the entire application may claim the legacy data (`claim: true`). Once claimed, `has_unclaimed_legacy_data` becomes `false` permanently; any subsequent attempt by any account returns `409 Conflict`.
   - Behavior for second account: If Account 1 has not claimed the legacy data (`claim: false` or skipped), Account 2 can claim it upon explicit confirmation. If Account 1 has already claimed it, Account 2 is assigned its own unique UUIDv4 tenant and cannot claim legacy data (`409 Conflict`).
6. **Offline Session Resilience**:
   - When offline, a previously authenticated user remains logged in using the stored local session until the session expires or is explicitly revoked.
   - Network calls are strictly confined to OAuth authentication, token refresh, token revocation, and provider LLM operations.
7. **CSRF & DNS Rebinding Protection (Origin & Host Rules)**:
   - `Host` header must match loopback (`127.0.0.1:<port>` or `localhost:<port>`). External host values are rejected with `403 Forbidden`.
   - `GET /api/v1/auth/google/callback`: Accepts missing `Origin` header when `Host` is valid and `state` is valid (browser redirection from Google identity provider does not emit Origin).
   - Mutating `POST` endpoints (`POST /auth/google/start`, `POST /auth/google/handoff`, `POST /auth/sign-out`, `POST /auth/legacy-data/claim`): Strictly require valid `Origin` (`tauri://localhost`, `http://127.0.0.1:<port>`, `http://localhost:<port>`). External web origins are rejected with `403 Forbidden`.
8. **Dev/Test Gating for Legacy Local Endpoints**:
   - Endpoints `GET /api/v1/auth/local-mode` and `POST /api/v1/auth/local-session` exist strictly for automated test suites and development environments via an explicit build/runtime flag (`ALBA_DEV_MODE=1` or `cfg(test)`).
   - In production release builds, these endpoints are completely disabled and fail closed with `404 Not Found`.

---

### Endpoint 1: `GET /api/v1/auth/session`
- **Purpose**: Returns the current authentication status and user identity metadata.
- **Headers**: Optional `Cookie: session_id=...` or `Authorization: Bearer <session_token>`.
- **Request Body**: None.
- **Response (Unauthenticated)**: `200 OK`
  ```json
  {
    "authenticated": false
  }
  ```
- **Response (Authenticated)**: `200 OK`
  ```json
  {
    "authenticated": true,
    "user": {
      "sub": "109823471092834710293",
      "email": "developer@nexus.local",
      "display_name": "Alba Developer",
      "picture": "https://lh3.googleusercontent.com/a/default-user"
    },
    "tenant_id": "00000000-0000-0000-0000-000000000001",
    "workspace_id": "00000000-0000-0000-0000-000000000002",
    "has_unclaimed_legacy_data": false
  }
  ```
- **Error Codes**:
  - `500 Internal Server Error`: Storage error checking session status.

---

### Endpoint 2: `POST /api/v1/auth/google/start`
- **Purpose**: Initiates the Google OAuth 2.0 PKCE flow. Generates cryptographically secure `state`, `nonce`, PKCE `code_verifier`, `code_challenge` (S256), and single-use `handoff_id`. Stores parameters transiently in memory with a 10-minute expiration.
- **Headers**: `Host`, `Origin` (enforcing loopback/tauri).
- **Request Body**: None or `{}`.
- **Response**: `200 OK`
  ```json
  {
    "auth_url": "https://accounts.google.com/o/oauth2/v2/auth?client_id=CLIENT_ID&redirect_uri=http%3A%2F%2F127.0.0.1%3APORT%2Fapi%2Fv1%2Fauth%2Fgoogle%2Fcallback&response_type=code&scope=openid%20email%20profile&state=RANDOM_STATE&nonce=RANDOM_NONCE&code_challenge=CHALLENGE&code_challenge_method=S256",
    "state": "RANDOM_STATE",
    "handoff_id": "HANDOFF_UUID"
  }
  ```
- **Error Codes**:
  - `403 Forbidden`: Untrusted `Host` or `Origin` header.
  - `500 Internal Server Error`: Failure generating cryptographic randomness or binding loopback listener.

---

### Endpoint 3: `GET /api/v1/auth/google/callback`
- **Purpose**: Handles the OAuth 2.0 loopback callback from Google.
- **Headers**: `Host` required. `Origin` optional (browser redirect).
- **Query Parameters**:
  - `code`: Authorization code from Google.
  - `state`: CSRF protection state parameter matching the stored `state`.
- **Validation Sequence**:
  1. Verifies `state` exists in transient store and has not expired (fail closed: `401 Unauthorized`).
  2. Consumes `state` immediately to prevent code replay attacks.
  3. Exchanges `code` and `code_verifier` with Google's token endpoint (`https://oauth2.googleapis.com/token` or mock identity server).
  4. Parses ID token and verifies using `jsonwebtoken` with Google's JWKS:
     - Algorithm is RS256; rejects `none` and HS256 algorithm confusion forged with public keys.
     - Key ID `kid` matches an active key in Google's JWKS; refreshes JWKS cache on unknown `kid`.
     - Issuer `iss` is `https://accounts.google.com` or `accounts.google.com`.
     - Audience `aud` matches the registered Google OAuth client ID.
     - Expiration `exp` is in the future.
     - `nonce` claim matches the original nonce generated in `google/start`.
  5. Extracts `sub`, `email`, `name`, `picture`.
  6. Upserts account record in SQLite `accounts` table.
  7. Securely stores Google tokens and session signing key in OS Keychain via `CredentialStore`.
  8. Marks session as ready under `handoff_id`.
- **Response**: `200 OK` HTML document:
  ```html
  <!DOCTYPE html>
  <html>
    <head><title>Alba - Đăng nhập thành công</title></head>
    <body style="font-family:sans-serif;text-align:center;padding:40px;">
      <h2>Đăng nhập thành công</h2>
      <p>Bạn có thể đóng tab trình duyệt này và quay lại ứng dụng Alba.</p>
    </body>
  </html>
  ```
  *(Zero tokens, secrets, or session IDs in body, headers, or URL).*
- **Error Codes**:
  - `400 Bad Request`: Missing `code` or `state` query parameters.
  - `401 Unauthorized`: Invalid state, expired state, code replay, or invalid ID token signature/claims.
  - `403 Forbidden`: Untrusted `Host` header.
  - `500 Internal Server Error`: Token exchange failure or Keychain write error.

---

### Endpoint 4: `POST /api/v1/auth/google/handoff`
- **Purpose**: Exchanges the single-use `handoff_id` from the desktop application window for an authenticated session.
- **Headers**: `Content-Type: application/json`, `Host`, `Origin` (enforcing loopback/tauri).
- **Request Body**:
  ```json
  {
    "handoff_id": "HANDOFF_UUID"
  }
  ```
- **Validation Sequence**:
  1. Validates `handoff_id` exists, is associated with a verified callback session, and has not expired.
  2. Consumes `handoff_id` immediately (single-use; subsequent replay attempts return `401 Unauthorized`).
  3. Sets `Set-Cookie: session_id=...; HttpOnly; SameSite=Strict; Path=/; Max-Age=2592000` (or issues native session token).
- **Response**: `200 OK`
  ```json
  {
    "status": "authenticated",
    "user": {
      "sub": "109823471092834710293",
      "email": "developer@nexus.local",
      "display_name": "Alba Developer",
      "picture": "https://lh3.googleusercontent.com/a/default-user"
    },
    "tenant_id": "00000000-0000-0000-0000-000000000001",
    "workspace_id": "00000000-0000-0000-0000-000000000002",
    "has_unclaimed_legacy_data": false
  }
  ```
- **Error Codes**:
  - `400 Bad Request`: Missing `handoff_id`.
  - `401 Unauthorized`: Invalid, expired, unauthenticated, or replayed `handoff_id`.
  - `403 Forbidden`: Untrusted `Host` or `Origin`.

---

### Endpoint 5: `POST /api/v1/auth/sign-out`
- **Purpose**: Terminates the active Google session and revokes tokens.
- **Headers**: `Host`, `Origin` (enforcing loopback/tauri), plus session cookie or `Authorization: Bearer <token>`.
- **Authentication**: Required.
- **Request Body** (optional):
  ```json
  {
    "cancel_runs": true
  }
  ```
  - `cancel_runs`: boolean (default: `true`). If `true`, cancels all in-flight runs for this account's tenant.
- **Revocation Sequence**:
  1. Local session is invalidated immediately from active memory and cookie deletion header (`Set-Cookie: session_id=; Max-Age=0`) is issued regardless of subsequent failures.
  2. Rotates/deletes session signing key in Keychain (`account:{sub}:jwt_secret`).
  3. If stored Google tokens exist, invokes Google token revocation endpoint (`https://oauth2.googleapis.com/revoke?token=...` or mock revocation) with a 5-second timeout.
  4. Deletes stored OAuth tokens from Keychain (`account:{sub}:google_tokens`).
  5. Marks account inactive in SQLite metadata.
  6. Records revocation status and any errors in audit trail using the account's tenant and workspace (never reports false revocation success).
  7. Any subsequent request bearing the old session token fails closed with `401 Unauthorized`.
- **Response (Full Success)**: `200 OK`
  ```json
  {
    "status": "signed_out",
    "failed_steps": []
  }
  ```
- **Response (Partial Failure)**: `207 Multi-Status`
  ```json
  {
    "status": "signed_out_partial",
    "failed_steps": [
      "google_revocation_http_500"
    ]
  }
  ```
  Note: Local session invalidation and cookie clearing always succeed; `failed_steps` enumerates downstream errors (e.g. `google_revocation_http_500`, `google_revocation_timeout`, `keychain_delete_jwt_secret`, `account_scope_not_found`, `invalid_account_tenant_uuid`). When account tenant or workspace scope cannot be resolved or parsed, audit records fall back to dedicated system audit scope (`00000000-0000-0000-0000-0000000000fe`) and error is noted in `failed_steps`.
- **Error Codes**:
  - `400 Bad Request`: Malformed or invalid JSON body.
  - `401 Unauthorized`: Missing or invalid session.
  - `403 Forbidden`: Untrusted `Host` or `Origin` header.

---

### Endpoint 6: `POST /api/v1/auth/legacy-data/claim`
- **Purpose**: Enables an authenticated Google account to claim or defer claiming existing legacy data (`00000000-...-0001/0002`).
- **Headers**: `Content-Type: application/json`, `Host`, `Origin` (enforcing loopback/tauri), session auth.
- **Authentication**: Required.
- **Request Body**:
  ```json
  {
    "claim": true
  }
  ```
- **Behavior**:
  - If `claim: true`: Sets the authenticated account's `tenant_id` to `00000000-0000-0000-0000-000000000001` and records claim in `legacy_data_claim` table. Exactly one account across the entire app can claim it; subsequent attempts by any account return `409 Conflict`.
  - If `claim: false`: Indicates "chưa nhận lúc này" (defer decision). Legacy data remains intact and unclaimed (`has_unclaimed_legacy_data` remains `true`).
- **Response**: `200 OK`
  ```json
  {
    "claimed": true,
    "tenant_id": "00000000-0000-0000-0000-000000000001"
  }
  ```
- **Error Codes**:
  - `400 Bad Request`: Missing `claim` boolean field.
  - `401 Unauthorized`: Unauthenticated.
  - `403 Forbidden`: CSRF check failed (`Origin` / `Host`).
  - `409 Conflict`: Legacy data has already been claimed by an account.

---

---

### 4. `GET /api/v1/workflows`
- **Purpose**: Lists all registered workflows for the tenant.
- **Headers**: `Authorization: Bearer <token>`
- **Query Parameters**: `limit` (default: 50)
- **Response**: `200 OK`
  ```json
  [
    {
      "id": "wf-12345",
      "name": "Research & Draft",
      "description": "Multi-agent research and synthesis workflow",
      "created_at": "2026-11-12T08:00:00Z",
      "updated_at": "2026-11-12T08:00:00Z"
    }
  ]
  ```

### 5. `POST /api/v1/workflows`
- **Purpose**: Stores and registers a complete `WorkflowSpec` definition.
- **Headers**: `Authorization: Bearer <token>`, `Content-Type: application/json`
- **Request Body**: `WorkflowSpec` (matches `src/schema/workflow.rs`).
- **Response**: `201 Created`
  ```json
  {
    "id": "wf-12345",
    "name": "Research & Draft",
    "nodes": [...],
    "edges": [...]
  }
  ```

### 6. `GET /api/v1/workflows/:id`
- **Purpose**: Fetches the complete JSON definition of a workflow.
- **Headers**: `Authorization: Bearer <token>`
- **Response**: `200 OK` with full `WorkflowSpec` JSON.

### 7. `PUT /api/v1/workflows/:id`
- **Purpose**: Updates an existing workflow definition.
- **Headers**: `Authorization: Bearer <token>`, `Content-Type: application/json`
- **Request Body**: `WorkflowSpec`.
- **Response**: `200 OK` with updated `WorkflowSpec`.

### 8. `POST /api/v1/workflows/:id/runs`
- **Purpose**: Queues execution of a workflow with caller-supplied input data.
- **Headers**: `Authorization: Bearer <token>`, `Content-Type: application/json`
- **Request Body**:
  ```json
  {
    "input_data": {
      "request": "Analyze quarterly market indicators"
    }
  }
  ```
- **Response**: `202 Accepted`
  ```json
  {
    "run_id": "run-01934e8f-4b00-7c2a-912a-89a1c8b321a0",
    "workflow_id": "wf-12345",
    "status": "QUEUED"
  }
  ```

### 9. `GET /api/v1/runs`
- **Purpose**: Queries run execution records.
- **Headers**: `Authorization: Bearer <token>`
- **Query Parameters**: `workflow_id` (optional), `limit` (default: 50)
- **Additive Fields**: `verified` (boolean), `unverified_reason` (optional string).
- **Response**: `200 OK`
  ```json
  [
    {
      "id": "run-01934e8f-4b00-7c2a-912a-89a1c8b321a0",
      "workflow_id": "wf-12345",
      "status": "SUCCEEDED",
      "created_at": "2026-11-12T08:15:00Z",
      "duration_seconds": 27.22,
      "verified": true,
      "unverified_reason": null
    }
  ]
  ```

### 10. `GET /api/v1/runs/:id`
- **Purpose**: Retrieves execution status, node states, output summary, and verification verdict for a single run.
- **Headers**: `Authorization: Bearer <token>`
- **Additive Fields**: `verified` (boolean), `unverified_reason` (optional string explaining verification failure).
- **Response**: `200 OK`
  ```json
  {
    "id": "run-01934e8f-4b00-7c2a-912a-89a1c8b321a0",
    "workflow_id": "wf-12345",
    "status": "SUCCEEDED",
    "created_at": "2026-11-12T08:15:00Z",
    "completed_at": "2026-11-12T08:15:27Z",
    "output_data": {
      "summary": "Completed market analysis report."
    },
    "current_node_id": null,
    "verified": true,
    "unverified_reason": null
  }
  ```

### 11. `GET /api/v1/runs/:id/stream` (SSE)
- **Purpose**: Real-time event stream (`text/event-stream`) streaming node progress, tool execution logs, and state checkpoints.
- **Headers**: `Authorization: Bearer <token>`, `Accept: text/event-stream`, `Last-Event-ID: <event-id>` (optional)
- **Response**: `200 OK` with chunked SSE messages.
  ```text
  event: node_started
  data: {"node_id": "researcher_node", "timestamp": "2026-11-12T08:15:01Z"}

  event: tool_call
  data: {"node_id": "researcher_node", "tool": "file_read", "path": "notes.md"}

  event: run_completed
  data: {"run_id": "run-01934e8f", "status": "SUCCEEDED"}
  ```
- **Reconnection & Last-Event-ID**:
  - Clients can reconnect after network interruption by providing `Last-Event-ID: <id>`.
  - The server queries `run_events` for all events strictly after the specified event sequence, emits them in order, and then seamlessly attaches to live events from the in-process broadcast channel.
- **Backpressure & Task Cleanup Policy**:
  - Broadcast channel buffer is fixed at 1,024 events. If a slow client lags behind the broadcast channel, `RecvError::Lagged` triggers an immediate catch-up query from durable SQLite `run_events` log, guaranteeing zero event loss.
  - Flow control applies naturally via HTTP chunked transfer and TCP socket backpressure.
  - Client disconnection terminates the async stream generator immediately, dropping the broadcast receiver with no leaked tasks or resources.
  - Heartbeats (`: keep-alive\n\n`) are sent every 15 seconds during inactive intervals.

### 12. `POST /api/v1/runs/:id/cancel`
- **Purpose**: Cancels an active or paused run execution.
- **Headers**: `Authorization: Bearer <token>`
- **Request Body**: None.
- **Response**: `200 OK`
  ```json
  {
    "status": "CANCELLED",
    "run_id": "run-01934e8f-4b00-7c2a-912a-89a1c8b321a0"
  }
  ```

### 13. `POST /api/v1/runs/:id/resume`
- **Purpose**: Resumes a paused run, recovering state from its latest checkpoint (G2 crash recovery and engine resumption).
- **Headers**: `Authorization: Bearer <token>`, `Content-Type: application/json`
- **Request Body**: Optional decision override:
  ```json
  {
    "decision": "allow_once"
  }
  ```
- **Response**: `200 OK`
  ```json
  {
    "status": "RESUMED",
    "run_id": "run-01934e8f-4b00-7c2a-912a-89a1c8b321a0"
  }
  ```

### 14. `POST /api/v1/approvals/:id/resolve`
- **Purpose**: Submits a human decision (`allow_once`, `allow_always`, or `deny`) for an approval gate and triggers checkpoint resumption.
- **Headers**: `Authorization: Bearer <token>`, `Content-Type: application/json`
- **Request Body**:
  ```json
  {
    "decision": "allow_once",
    "original_ai_output": "cat /etc/passwd",
    "human_corrected_output": null,
    "feedback": "Approved for single read."
  }
  ```
- **Response**: `200 OK`
  ```json
  {
    "status": "RESOLVED",
    "approval_id": "app-01934e8f-approval-1",
    "decision": "allow_once"
  }
  ```

### 15. `GET /api/v1/approvals`
- **Purpose**: Queries pending approval requests.
- **Headers**: `Authorization: Bearer <token>`
- **Query Parameters**: `run_id` (optional)
- **Response**: `200 OK`
  ```json
  [
    {
      "id": "app-01934e8f-approval-1",
      "run_id": "run-01934e8f-4b00-7c2a-912a-89a1c8b321a0",
      "node_id": "gate_node",
      "tool": "run_cmd",
      "arguments": {"cmd": "curl https://api.external.com"},
      "author_agent_id": "agent-worker",
      "created_at": "2026-11-12T08:15:10Z"
    }
  ]
  ```

### 16. `GET /api/v1/runs/:id/checkpoints`
- **Purpose**: Returns the chronological checkpoint history for time-travel and audit.
- **Headers**: `Authorization: Bearer <token>`
- **Response**: `200 OK`
  ```json
  [
    {
      "id": "chk-001",
      "run_id": "run-01934e8f",
      "step_index": 1,
      "node_id": "researcher_node",
      "created_at": "2026-11-12T08:15:05Z",
      "state_hash": "a1b2c3d4..."
    }
  ]
  ```

### 17. `POST /api/v1/runs/:id/checkpoints/:id/fork`
- **Purpose**: Forks an existing run from a historical checkpoint into two competing branches.
- **Headers**: `Authorization: Bearer <token>`, `Content-Type: application/json`
- **Request Body**:
  ```json
  {
    "branch_a": {"prompt": "Provide detailed technical specifications"},
    "branch_b": {"prompt": "Provide a high-level executive summary", "model": "anthropic/claude-3-5-sonnet"}
  }
  ```
- **Response**: `201 Created`
  ```json
  {
    "branch_a": {"run_id": "run-branch-a-123"},
    "branch_b": {"run_id": "run-branch-b-456"}
  }
  ```

### 18. `POST /api/v1/runs/:id/checkpoints/:id/rewind`
- **Purpose**: Rewinds execution to a previous checkpoint, optionally injecting state or prompt overrides.
- **Headers**: `Authorization: Bearer <token>`, `Content-Type: application/json`
- **Request Body**:
  ```json
  {
    "node_overrides": {"prompt": "Revised prompt text"}
  }
  ```
- **Response**: `200 OK`
  ```json
  {
    "run_id": "run-01934e8f",
    "checkpoint_id": "chk-001"
  }
  ```

### 19. `GET /api/v1/runs/:id/artifacts`
- **Purpose**: Lists verified file deliverables produced during the run.
- **Headers**: `Authorization: Bearer <token>`
- **Response**: `200 OK`
  ```json
  [
    {
      "id": "art-001",
      "filename": "market_summary.pdf",
      "file_path": "artifacts/market_summary.pdf",
      "file_size": 1048576,
      "mime_type": "application/pdf",
      "verified": true,
      "sha256": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    }
  ]
  ```

### 19b. `GET /api/v1/runs/:id/evidence`
- **Purpose**: Lists machine verification command runs, terminal outputs, exit codes, captured output SHA-256 hashes, and verification status for run auditing.
- **Headers**: `Authorization: Bearer <token>`
- **Additive Fields**: `output_sha256` (string SHA-256 of combined stdout + stderr), `unverified_reason` (optional string explaining non-zero exit or unverified status).
- **Response**: `200 OK`
  ```json
  [
    {
      "id": "evi-001",
      "run_id": "run-01934e8f",
      "command": "cargo test --test integration_tests",
      "exit_code": 0,
      "stdout": "test result: ok. 10 passed; 0 failed",
      "stderr": "",
      "duration_ms": 320,
      "verified": true,
      "created_at": "2026-11-12T15:00:00Z",
      "output_sha256": "4b227777d4dd1fc61c6f884f48641d02b4d121d3fd328cb08b5531fcacdabf8a",
      "unverified_reason": null
    }
  ]
  ```

### 20. `GET /api/v1/learning-transfer/capsules`
- **Purpose**: Returns knowledge capsules and negative-transfer guards.
- **Headers**: `Authorization: Bearer <token>`
- **Response**: `200 OK`
  ```json
  [
    {
      "id": "capsule-001",
      "title": "SQL Join Optimization Rule",
      "trigger_pattern": "INNER JOIN on large table",
      "advice": "Use indexed foreign key or partition table",
      "scope": "workspace",
      "sensitivity": "internal"
    }
  ]
  ```

### 21. `GET /api/v1/settings/chat-models`
- **Purpose**: Lists available LLM models for agent assignment and chat selection.
- **Headers**: `Authorization: Bearer <token>`
- **Response**: `200 OK`
  ```json
  {
    "models": [
      {
        "id": "gemini-3.8-flash-high",
        "name": "Gemini 3.8 Flash (High)",
        "provider": "google",
        "context_length": 1048576
      },
      {
        "id": "claude-3-7-sonnet",
        "name": "Claude 3.7 Sonnet",
        "provider": "anthropic",
        "context_length": 200000
      }
    ],
    "default": "gemini-3.8-flash-high"
  }
  ```

### 22. `GET /api/v1/settings/execution`
- **Purpose**: Reads current execution policy (Deny/Ask/Allow rules and network domain filters).
- **Headers**: `Authorization: Bearer <token>`
- **Response**: `200 OK`
  ```json
  {
    "default_mode": "ask",
    "allowed_domains": ["api.anthropic.com", "generativelanguage.googleapis.com"],
    "denied_domains": ["169.254.169.254"]
  }
  ```

### 23. `PUT /api/v1/settings/execution`
- **Purpose**: Updates sandbox execution policy rules and domain boundaries.
- **Headers**: `Authorization: Bearer <token>`, `Content-Type: application/json`
- **Request Body**:
  ```json
  {
    "default_mode": "ask",
    "allowed_domains": ["api.anthropic.com"],
    "denied_domains": ["169.254.169.254", "*.internal"]
  }
  ```
- **Response**: `200 OK` with updated policy.

### 24. `GET /api/v1/tools/builtin`
- **Purpose**: Lists system built-in tools supported by the native Rust runner.
- **Headers**: `Authorization: Bearer <token>`
- **Response**: `200 OK`
  ```json
  [
    {
      "id": "file_read",
      "name": "Read File",
      "description": "Reads text content within workspace bounds",
      "enabled": true
    },
    {
      "id": "file_write",
      "name": "Write File",
      "description": "Writes file content requiring governance approval",
      "enabled": true
    },
    {
      "id": "run_cmd",
      "name": "Run Command",
      "description": "Executes shell commands in seatbelt sandbox",
      "enabled": true
    },
    {
      "id": "dir_list",
      "name": "List Directory",
      "description": "Lists workspace directories",
      "enabled": true
    }
  ]
  ```

---

## 4. SQLite WAL Database Schema DDL

```sql
CREATE TABLE IF NOT EXISTS runs (
    id TEXT PRIMARY KEY,
    workflow_id TEXT NOT NULL,
    status TEXT NOT NULL,
    input_data TEXT NOT NULL,
    output_data TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    completed_at TEXT
);

CREATE TABLE IF NOT EXISTS workflow_specs (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    spec_json TEXT NOT NULL,
    version INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS checkpoints (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    step_index INTEGER NOT NULL,
    node_id TEXT,
    state_blob BLOB NOT NULL,
    state_hash TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY(run_id) REFERENCES runs(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS approvals (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    node_id TEXT NOT NULL,
    tool_name TEXT NOT NULL,
    arguments_json TEXT NOT NULL,
    author_agent_id TEXT NOT NULL,
    approver_actor_id TEXT,
    decision TEXT,
    feedback TEXT,
    resolved_at TEXT,
    created_at TEXT NOT NULL,
    FOREIGN KEY(run_id) REFERENCES runs(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS audit_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    actor_id TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY(run_id) REFERENCES runs(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS artifacts (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    filename TEXT NOT NULL,
    file_path TEXT NOT NULL,
    file_size INTEGER NOT NULL,
    mime_type TEXT NOT NULL,
    sha256 TEXT NOT NULL,
    verified INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    FOREIGN KEY(run_id) REFERENCES runs(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS capsules (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    trigger_pattern TEXT NOT NULL,
    advice TEXT NOT NULL,
    scope TEXT NOT NULL,
    sensitivity TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_runs_workflow ON runs(workflow_id);
CREATE INDEX IF NOT EXISTS idx_checkpoints_run ON checkpoints(run_id, step_index);
CREATE INDEX IF NOT EXISTS idx_approvals_run ON approvals(run_id, decision);
CREATE INDEX IF NOT EXISTS idx_audit_run ON audit_events(run_id);
CREATE INDEX IF NOT EXISTS idx_artifacts_run ON artifacts(run_id);
```
