# Phase G6A Report: Google Accounts and Provider Connections

## 1. Header
- **Phase**: G6A (Google Accounts and Provider Connections)
- **Branch**: `feat/g6a-accounts`
- **Head Commit Hash**: `b5f518310b1fe9c317983676991fec5222d691bc` (`b5f5183`)
- **Date**: 2026-11-17
- **Host Environment**: macOS (Apple Silicon arm64, Darwin 24.3.0)
- **Role**: Owner Phase G6A
- **Machine Verdict**: PASS

---

## 2. Verdict
| Slice | Title | Verdict | Commit | Evidence |
| :--- | :--- | :--- | :--- | :--- |
| G6A-S1 | Provider Auth Research & Label Census | PASS | `ec02b39` | Census located hardcoded fake plan `ENTERPRISE` in `src/server/auth.rs:11` and false `Đã kết nối` in `src/server/routes/settings.rs:26`; documented API key and OAuth policies for Anthropic, OpenAI, Gemini, DeepSeek |
| G6A-S2 | Keychain Secret & Random JWT Key | PASS | `c5c4ab2`, `3061fe4` | Eliminated hardcoded `DEFAULT_JWT_SECRET`; cryptographically secure 256-bit secret stored in OS Keychain; `security-framework 3.7.0` FFI avoids `ps` argv leak; `tests/g6a_accounts_s2_tests.rs` (13 ok) |
| G6A-S3 | Google OAuth PKCE & Session Engine | PASS | `c1a56aa`, `332dc66`, `4ef3555` | Google OAuth 2.0 PKCE loopback redirect, RS256 JWKS validation, single-use `handoff_id`, HttpOnly cookie, legacy data claim, fail-closed sign-out with HTTP 207 on partial failure; `tests/g6a_accounts_s3_tests.rs` (17 ok) |
| G6A-S4 | Truthful Reactive Account UI | PASS | `c1a56aa` | Removed fake `ENTERPRISE` badge and fake `Đã kết nối` indicator; reactive session state via `GET /session`; Google login modal with handoff polling; `web/test/account-menu.test.mjs` (7 ok) |
| G6A-S5 | Native Provider Adapters | PASS | `d56fd40` | Native Anthropic Messages API (`AnthropicClient`), Gemini `generateContent` API (`GeminiClient`), OpenAI/DeepSeek (`OpenAiCompatibleClient`) behind `ModelClient`; `tests/g6a_providers_s5_tests.rs` (19 ok) |
| G6A-S6 | Provider Connections API & UI | PASS | `2c14639` | Scoped to authenticated Google `sub`; credentials in Keychain under `account:{sub}:provider:{id}`; masked keys; zero plaintext secret leakage; `tests/g6a_providers_s6_tests.rs` (5 ok), `web/test/provider-credentials.test.mjs` (3 ok) |
| G6A-S7 | Engine Provider Dispatch | PASS | `76b7ad0` | Dispatches workflow model calls via `ServerModelClient::for_account` to active account provider; fails closed on missing/revoked keys; audit rows emitted without secret leakage; `tests/g6a_engine_s7_tests.rs` (6 ok) |
| G6A-Harden | Provider Host Pinning | PASS | `ec02b39` | Official presets pin immutable hosts and reject client `base_url` with HTTP 400; tampered hosts in SQLite fail closed; redirects disabled via `reqwest::redirect::Policy::none()`; custom presets validated; `tests/g6a_pinned_hosts_tests.rs` (6 ok) |
| G6A-Deps | URL Dependency Evaluation | PASS | `b5f5183` | Recorded `url 2.5` crate evaluation in `tasks/decisions.tsv`; synchronized both `decisions.tsv` files with clean zero diff |

---

## 3. Gate
All mandatory verification gates passed cleanly with exit code 0 using `CARGO_TARGET_DIR=/tmp/alba-target-v CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0`:

1. `cargo fmt --check`:
   - Command: `cargo fmt --check`
   - Exit code: 0
   - Output: clean (zero formatting diff across all Rust sources)

2. `cargo clippy --all-targets -- -D warnings`:
   - Command: `cargo clippy --all-targets -- -D warnings`
   - Exit code: 0
   - Output: 0 warnings, 0 errors across library, binaries, and all 42 test suites

3. `cargo test -- --test-threads=2`:
   - Command: `cargo test -- --test-threads=2`
   - Exit code: 0
   - Summary: **266 passed, 0 failed, 0 ignored** across 42 test targets:
     - `unittests src/lib.rs`: 0 passed, 0 failed
     - `unittests src/main.rs`: 0 passed, 0 failed
     - `tests/api_approvals_tests.rs`: 3 passed, 0 failed
     - `tests/api_auth_tests.rs`: 9 passed, 0 failed
     - `tests/api_contract_tests.rs`: 13 passed, 0 failed
     - `tests/api_runs_tests.rs`: 6 passed, 0 failed
     - `tests/api_sse_tests.rs`: 5 passed, 0 failed
     - `tests/g4_artifact_tests.rs`: 15 passed, 0 failed
     - `tests/g4_claims_tests.rs`: 6 passed, 0 failed
     - `tests/g4_evidence_tests.rs`: 17 passed, 0 failed
     - `tests/g4_policy_tests.rs`: 2 passed, 0 failed
     - `tests/g4_sandbox_tests.rs`: 8 passed, 0 failed
     - `tests/g5_memory_s1_tests.rs`: 5 passed, 0 failed
     - `tests/g5_memory_s2_tests.rs`: 6 passed, 0 failed
     - `tests/g5_memory_s3_tests.rs`: 3 passed, 0 failed
     - `tests/g5_memory_s4_tests.rs`: 3 passed, 0 failed
     - `tests/g5_memory_s5_tests.rs`: 3 passed, 0 failed
     - `tests/g5_memory_s6_tests.rs`: 3 passed, 0 failed
     - `tests/g5_memory_s7_tests.rs`: 6 passed, 0 failed
     - `tests/g6_packaging_s1_tests.rs`: 2 passed, 0 failed
     - `tests/g6_packaging_s2_tests.rs`: 1 passed, 0 failed
     - `tests/g6_packaging_s3_tests.rs`: 2 passed, 0 failed
     - `tests/g6_packaging_s4_tests.rs`: 3 passed, 0 failed
     - `tests/g6_packaging_s5_tests.rs`: 1 passed, 0 failed
     - `tests/g6a_accounts_s2_tests.rs`: 13 passed, 0 failed
     - `tests/g6a_accounts_s3_tests.rs`: 17 passed, 0 failed
     - `tests/g6a_engine_s7_tests.rs`: 6 passed, 0 failed
     - `tests/g6a_pinned_hosts_tests.rs`: 6 passed, 0 failed
     - `tests/g6a_providers_s5_tests.rs`: 19 passed, 0 failed
     - `tests/g6a_providers_s6_tests.rs`: 5 passed, 0 failed
     - `tests/integration_tests.rs`: 31 passed, 0 failed
     - `tests/phase1_phase2_tests.rs`: 7 passed, 0 failed
     - `tests/schema_contract_tests.rs`: 5 passed, 0 failed
     - `tests/validation_tests.rs`: 15 passed, 0 failed
     - `tests/workflow_s2_tests.rs`: 1 passed, 0 failed
     - `tests/workflow_s3_tests.rs`: 5 passed, 0 failed
     - `tests/workflow_s4_tests.rs`: 3 passed, 0 failed
     - `tests/workflow_s5_tests.rs`: 3 passed, 0 failed
     - `tests/workflow_s6_tests.rs`: 1 passed, 0 failed
     - `tests/workflow_s7_tests.rs`: 3 passed, 0 failed
     - `tests/workflow_s8_e2e_tests.rs`: 4 passed, 0 failed
     - `Doc-tests nexus_albaai`: 0 passed, 0 failed

4. `cargo deny check`:
   - Command: `cargo deny check`
   - Exit code: 0
   - Output: `advisories ok, bans ok, licenses ok, sources ok`

5. `./scripts/check_no_comments.sh`:
   - Command: `./scripts/check_no_comments.sh`
   - Exit code: 0
   - Output: `OK: 0 comments found across 163 checked file(s).`

6. `pnpm lint` (in `alba/web`):
   - Command: `pnpm lint` (`eslint src --max-warnings=0`)
   - Exit code: 0
   - Output: clean (0 errors, 0 warnings)

7. `pnpm check` (in `alba/web`):
   - Command: `pnpm check` (`tsc --noEmit`)
   - Exit code: 0
   - Output: clean (0 type errors)

8. `pnpm test` (in `alba/web`):
   - Command: `pnpm test` (`node --test`)
   - Exit code: 0
   - Output: **99 passed, 0 failed** across 99 test cases

---

## 4. Evidence
1. **Secret Isolation in Keychain**: Plaintext API keys and OAuth tokens are stored exclusively in the OS Keychain (`account:{sub}:provider:{id}` and `account:{sub}:google_tokens`). SQLite tables store only `masked_key` (for example `sk-ant-...6789`). Audit logs record usage, latency, and status without secrets.
2. **Cryptographic Identity Defense**: Google ID tokens are verified using RS256 against Google JWKS with strict signature, audience, issuer, expiry, and nonce checks. Tokens using `alg: "none"` or HS256 algorithm confusion are rejected with HTTP 401.
3. **Session Handoff Protection**: Google OAuth callback renders static HTML with zero tokens in body, headers, or URL. Single-use `handoff_id` is consumed once and bound to a 300s expiration, returning an HttpOnly SameSite=Strict session cookie.
4. **Host Pinning Defense**: Official presets (`openai`, `anthropic`, `google_gemini`, `deepseek`) pin base URLs to hardcoded constants and reject client-provided `base_url` with HTTP 400. Stored connections with tampered hosts fail closed at engine runtime before network contact. HTTP redirects are disabled via `Policy::none()`.
5. **Fail-Closed Sign-Out**: Sign-out immediately purges local session cookies, purges all profile secrets from Keychain, invokes external Google revocation with a 5s timeout, and returns HTTP 207 Multi-Status with error details if any remote step fails.

---

## 5. Reviewer
Independent review was conducted and recorded in `tasks/reviews/G6A.review.md`:
- **Secret Hunt**: Verified 0 secret leaks across SQLite databases, logs, audit tables, SSE streams, and endpoint responses.
- **Break Attempts**: 28 security break vectors tested (ID token tampering, OAuth callback tampering, sign-out failures, multi-account isolation, provider host tampering, release-mode local session refusal). All 28 failed closed and resisted attack.
- **Mutation Spot Checks**: Tested 4 distinct mutations across host pinning, nonce validation, single-use handoff, and Keychain purge. All 4 mutations were detected by test failures and restored cleanly.
- **Silent Failures Audit**: Verified 0 instances of `let _ =` in production code. All `.ok()` usages evaluated and confirmed safe.

---

## 6. Dependencies
Three new dependencies were introduced during Phase G6A:

| Dependency Crate | Version | License | Incremental Size | `cargo deny` Status | `tasks/decisions.tsv` Alignment |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `security-framework` | `3.7.0` | MIT OR Apache-2.0 | ~0 KB (links system framework) | Green (Clean) | Recorded in `tasks/decisions.tsv` (line 54) |
| `jsonwebtoken` | `9.3.0` | MIT | ~40 KB (uses existing ring 0.17) | Green (Clean) | Recorded in `tasks/decisions.tsv` (line 61) |
| `url` | `2.5` | MIT OR Apache-2.0 | ~140 KB | Green (Clean) | Recorded in `tasks/decisions.tsv` (line 72) |

Both `tasks/decisions.tsv` files (`/Users/nguyenvokhang/Downloads/v3/tasks/decisions.tsv` and `/Users/nguyenvokhang/Downloads/v3/alba/tasks/decisions.tsv`) are identical with zero diff.

---

## 7. Deviation
Known limitations and design deviations:
1. **Custom Preset DNS Rebinding**:
   - `validate_custom_base_url` validates URLs statically (verifies `is_loopback`, `v4.is_private()`, `v6.is_unique_local()`, and blacklists internal domain suffixes `.local`, `.internal`, `.lan`).
   - Hostnames are not resolved via DNS at registration time to prevent offline test failures and network dependency during validation. Consequently, a public domain name configured to resolve to a private IP (DNS rebinding) is not detected at URL validation time.
   - Mitigation: Custom presets require an explicit confirmation flag `confirm_custom_host: true`, enforce HTTPS unless loopback, disable HTTP redirect following (`reqwest::redirect::Policy::none()`), and display a clear destination warning in the UI before API keys are dispatched.
2. **IPv4-Mapped IPv6 Addresses**:
   - `validate_custom_base_url` inspects native IPv6 address properties but does not unwrap dual-stack IPv4-mapped IPv6 addresses (`::ffff:192.168.x.x`).
   - Mitigation: Loopback and public HTTPS policies remain enforced; custom hosts display destination warnings.

---

## 8. Not verified
The following items are intentionally mocked, blocked, or verified at alternative boundaries:
1. **S8 (Provider OAuth)**: BLOCKED: cần client ID đã đăng ký. No registered client IDs exist for OpenAI (Sign in with ChatGPT) or Google (Gemini consumer subscription usage). No synthetic or fake client IDs were invented. Connecting providers via BYOK API keys stored in Keychain remains the primary verified path.
2. **Live Google Production Identity Servers**: Verified against an RFC-compliant loopback mock server generating RS256 JWKS keypairs and OAuth tokens. Live sign-in through `accounts.google.com` requires an interactive end-user browser and registered Google Client ID.
3. **Live External LLM Provider Calls**: Tested against local HTTP mock servers mimicking OpenAI, Anthropic, Gemini, and DeepSeek wire formats and error schemas.
4. **Tauri Packaged Window Lifecycle**: Verified at the Axum loopback HTTP server and TypeScript/React component level; native macOS window events were not exercised interactively.

---

## 9. Open questions
1. **S8 Provider OAuth Registration**: When will production OAuth client IDs be provisioned for OpenAI and Google provider connections? (Currently blocked: cần client ID đã đăng ký).
2. **DNS Rebinding Mitigation Strategy**: In future phases, should DNS resolution be performed at request time via a custom `reqwest` connector that validates the socket IP immediately prior to TCP handshake, avoiding registration-time DNS side effects?
3. **Canary Authoring & Pinning**: Authoring of `tasks/verify/canaries/G6A.json`, registration of G6A in `tasks/verify/phases.json`, and updating `tasks/reviews/VERIFIER.sha256` are reserved for human maintainers.
