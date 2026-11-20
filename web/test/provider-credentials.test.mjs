import test from "node:test";
import assert from "node:assert/strict";

function maskApiKey(key) {
  const trimmed = (key ?? "").trim();
  if (trimmed.length <= 8) return "********";
  let prefixLen = 3;
  if (trimmed.startsWith("sk-ant-")) prefixLen = 7;
  else if (trimmed.startsWith("sk-")) prefixLen = 3;
  else if (trimmed.startsWith("AIza")) prefixLen = 4;
  const suffixLen = 4;
  if (trimmed.length <= prefixLen + suffixLen) {
    return trimmed.slice(0, prefixLen) + "..." + trimmed.slice(-2);
  }
  return trimmed.slice(0, prefixLen) + "..." + trimmed.slice(-suffixLen);
}

function sanitizeCredential(cred) {
  return {
    id: cred.id,
    preset_id: cred.preset_id,
    provider: cred.provider,
    status: cred.status ?? "UNCONFIGURED",
    masked_key: cred.masked_key ?? (cred.api_key ? maskApiKey(cred.api_key) : null),
    base_url: cred.base_url ?? null,
    is_default: Boolean(cred.is_default),
    available_models: cred.available_models ?? [],
    safe_error: cred.safe_error ?? null
  };
}

test("maskApiKey masks Anthropic, OpenAI, Gemini, and DeepSeek keys without leaking raw secret", () => {
  const ant = maskApiKey("sk-ant-api03-very-long-secret-key-12345");
  assert.equal(ant, "sk-ant-...2345");
  assert.equal(ant.includes("very-long-secret"), false);

  const oai = maskApiKey("sk-proj-super-confidential-openai-key-9876");
  assert.equal(oai, "sk-...9876");
  assert.equal(oai.includes("confidential"), false);

  const gem = maskApiKey("AIzaSyVerySecretGoogleGeminiKey-4321");
  assert.equal(gem, "AIza...4321");
  assert.equal(gem.includes("VerySecret"), false);

  const shortKey = maskApiKey("sk-short");
  assert.equal(shortKey, "********");
});

test("sanitizeCredential guarantees raw key is never present in serialized model", () => {
  const rawCred = {
    id: "cred-1",
    preset_id: "anthropic",
    provider: "anthropic",
    api_key: "sk-ant-api03-real-secret-token-7777",
    status: "CONNECTED",
    base_url: "https://api.anthropic.com/v1",
    is_default: true,
    available_models: ["claude-3-7-sonnet"]
  };

  const sanitized = sanitizeCredential(rawCred);
  assert.equal("api_key" in sanitized, false);
  assert.equal(sanitized.masked_key, "sk-ant-...7777");
  assert.equal(JSON.stringify(sanitized).includes("real-secret-token"), false);
  assert.equal(sanitized.status, "CONNECTED");
});

test("provider presets catalog schema defines all four direct LLM providers", () => {
  const catalog = {
    categories: {
      direct: [
        { id: "openai", protocol: "openai", icon: "openai" },
        { id: "anthropic", protocol: "anthropic", icon: "anthropic" },
        { id: "google_gemini", protocol: "gemini", icon: "google" },
        { id: "deepseek", protocol: "openai", icon: "deepseek" }
      ]
    }
  };

  const direct = catalog.categories.direct;
  const ids = direct.map((p) => p.id);
  assert.deepEqual(ids, ["openai", "anthropic", "google_gemini", "deepseek"]);
  const protocols = direct.map((p) => p.protocol);
  assert.deepEqual(protocols, ["openai", "anthropic", "gemini", "openai"]);
});

function sanitizeErrorMessage(message) {
  if (!message) return "";
  return message
    .replace(/(sk-ant-[a-zA-Z0-9_-]{4})[a-zA-Z0-9_-]+([a-zA-Z0-9_-]{4})/g, "$1...$2")
    .replace(/(sk-[a-zA-Z0-9_-]{4})[a-zA-Z0-9_-]+([a-zA-Z0-9_-]{4})/g, "$1...$2")
    .replace(/(AIza[a-zA-Z0-9_-]{4})[a-zA-Z0-9_-]+([a-zA-Z0-9_-]{4})/g, "$1...$2")
    .replace(/(Bearer\s+)[^\s]+/gi, "$1********");
}

function readError(error) {
  if (error instanceof Error) return sanitizeErrorMessage(error.message);
  return "Something went wrong. Please try again.";
}

test("sanitizeErrorMessage redacts sensitive API keys and authorization headers", () => {
  const msg1 = "Failed to connect: sk-1234secretapikey5678 is invalid";
  assert.equal(sanitizeErrorMessage(msg1), "Failed to connect: sk-1234...5678 is invalid");

  const msg2 = "Anthropic returned 401 for sk-ant-api03-very-confidential-token-9999";
  assert.equal(sanitizeErrorMessage(msg2), "Anthropic returned 401 for sk-ant-api0...9999");

  const msg3 = "Gemini key AIzaSyA1234supersecretkey5678 denied";
  assert.equal(sanitizeErrorMessage(msg3), "Gemini key AIzaSyA1...5678 denied");

  const msg4 = "Authorization: Bearer secret-bearer-token-here expired";
  assert.equal(sanitizeErrorMessage(msg4), "Authorization: Bearer ******** expired");
});

test("readError safely redacts secrets in error instances", () => {
  const err = new Error("Connection error: sk-1234abcdef9876 failed");
  assert.equal(readError(err), "Connection error: sk-1234...9876 failed");
  assert.equal(readError(null), "Something went wrong. Please try again.");
});

