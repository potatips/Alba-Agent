export function resolveAccountIdentity(session, fallbackLabel) {
  if (!session || !session.authenticated || !session.user) {
    return {
      authenticated: false,
      label: fallbackLabel || "Chưa đăng nhập",
      email: null,
      picture: null
    };
  }
  const user = session.user;
  const label = user.name || user.email || fallbackLabel || "Tài khoản Google";
  return {
    authenticated: true,
    label,
    email: user.email || null,
    picture: user.picture || null
  };
}

export function shouldDisplayLegacyPrompt(session) {
  return Boolean(session && session.authenticated && session.user && session.has_unclaimed_legacy_data);
}

export function handleSignOutResult(result) {
  if (!result || result.status === "signed_out") {
    return {
      success: true,
      partial: false,
      failedSteps: [],
      warning: null
    };
  }
  if (result.status === "signed_out_partial") {
    const failedSteps = Array.isArray(result.failed_steps) ? result.failed_steps : [];
    return {
      success: true,
      partial: true,
      failedSteps,
      warning: "Một số bước dọn dẹp hoặc thu hồi chưa hoàn tất: " + failedSteps.join(", ")
    };
  }
  return {
    success: false,
    partial: false,
    failedSteps: [],
    warning: result.error || "Đăng xuất thất bại"
  };
}
