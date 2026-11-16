import test from "node:test";
import assert from "node:assert/strict";
import {
  resolveAccountIdentity,
  shouldDisplayLegacyPrompt,
  handleSignOutResult
} from "../src/lib/account-state.mjs";

test("unauthenticated state resolves to signed-out label without fake badges or profile data", () => {
  const nullSession = resolveAccountIdentity(null, "Chưa đăng nhập");
  assert.equal(nullSession.authenticated, false);
  assert.equal(nullSession.label, "Chưa đăng nhập");
  assert.equal(nullSession.email, null);
  assert.equal(nullSession.picture, null);

  const unauthenticatedSession = resolveAccountIdentity({
    authenticated: false,
    user: null,
    has_unclaimed_legacy_data: false
  }, "Signed out");
  assert.equal(unauthenticatedSession.authenticated, false);
  assert.equal(unauthenticatedSession.label, "Signed out");
  assert.equal(unauthenticatedSession.email, null);
  assert.equal(unauthenticatedSession.picture, null);
});

test("authenticated state displays Google name, email, and avatar picture", () => {
  const session = {
    authenticated: true,
    user: {
      id: "google-123",
      email: "engineer@example.com",
      name: "Alba Engineer",
      picture: "https://lh3.googleusercontent.com/avatar.png",
      tenant_id: "tenant-1",
      workspace_id: "workspace-1"
    },
    has_unclaimed_legacy_data: false
  };

  const identity = resolveAccountIdentity(session, "Chưa đăng nhập");
  assert.equal(identity.authenticated, true);
  assert.equal(identity.label, "Alba Engineer");
  assert.equal(identity.email, "engineer@example.com");
  assert.equal(identity.picture, "https://lh3.googleusercontent.com/avatar.png");
});

test("authenticated state falls back to email or generic label when name is absent", () => {
  const emailOnlySession = {
    authenticated: true,
    user: {
      id: "google-456",
      email: "engineer@example.com",
      name: null,
      picture: null,
      tenant_id: "tenant-1",
      workspace_id: "workspace-1"
    },
    has_unclaimed_legacy_data: false
  };

  const emailIdentity = resolveAccountIdentity(emailOnlySession, "Chưa đăng nhập");
  assert.equal(emailIdentity.authenticated, true);
  assert.equal(emailIdentity.label, "engineer@example.com");
  assert.equal(emailIdentity.email, "engineer@example.com");
  assert.equal(emailIdentity.picture, null);

  const noNameOrEmailSession = {
    authenticated: true,
    user: {
      id: "google-789",
      email: null,
      name: null,
      picture: null,
      tenant_id: "tenant-1",
      workspace_id: "workspace-1"
    },
    has_unclaimed_legacy_data: false
  };

  const genericIdentity = resolveAccountIdentity(noNameOrEmailSession, "Tài khoản Google");
  assert.equal(genericIdentity.authenticated, true);
  assert.equal(genericIdentity.label, "Tài khoản Google");
});

test("legacy claim banner display requires authenticated session and active legacy data", () => {
  assert.equal(shouldDisplayLegacyPrompt(null), false);

  assert.equal(shouldDisplayLegacyPrompt({
    authenticated: false,
    user: null,
    has_unclaimed_legacy_data: true
  }), false);

  assert.equal(shouldDisplayLegacyPrompt({
    authenticated: true,
    user: {
      id: "google-123",
      email: "eng@example.com",
      name: "Eng",
      picture: null,
      tenant_id: "t1",
      workspace_id: "w1"
    },
    has_unclaimed_legacy_data: false
  }), false);

  assert.equal(shouldDisplayLegacyPrompt({
    authenticated: true,
    user: {
      id: "google-123",
      email: "eng@example.com",
      name: "Eng",
      picture: null,
      tenant_id: "t1",
      workspace_id: "w1"
    },
    has_unclaimed_legacy_data: true
  }), true);
});

test("clean sign-out returns success with empty failed steps and no warnings", () => {
  const res = handleSignOutResult({ status: "signed_out", failed_steps: [] });
  assert.equal(res.success, true);
  assert.equal(res.partial, false);
  assert.deepEqual(res.failedSteps, []);
  assert.equal(res.warning, null);

  const nullRes = handleSignOutResult(null);
  assert.equal(nullRes.success, true);
  assert.equal(nullRes.partial, false);
  assert.deepEqual(nullRes.failedSteps, []);
  assert.equal(nullRes.warning, null);
});

test("partial sign-out returns success true with partial true, failed steps, and warning message", () => {
  const res = handleSignOutResult({
    status: "signed_out_partial",
    failed_steps: ["google_revocation_timeout", "keychain_delete_jwt_secret"]
  });
  assert.equal(res.success, true);
  assert.equal(res.partial, true);
  assert.deepEqual(res.failedSteps, [
    "google_revocation_timeout",
    "keychain_delete_jwt_secret"
  ]);
  assert.match(res.warning, /Một số bước dọn dẹp hoặc thu hồi chưa hoàn tất/);
  assert.match(res.warning, /google_revocation_timeout/);
  assert.match(res.warning, /keychain_delete_jwt_secret/);
});

test("failed sign-out returns success false and error message", () => {
  const res = handleSignOutResult({
    status: "error",
    error: "Connection refused"
  });
  assert.equal(res.success, false);
  assert.equal(res.partial, false);
  assert.deepEqual(res.failedSteps, []);
  assert.equal(res.warning, "Connection refused");
});
