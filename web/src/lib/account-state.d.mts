export interface AccountUser {
  sub?: string;
  id?: string;
  email: string | null;
  name: string | null;
  picture: string | null;
  tenant_id?: string;
  workspace_id?: string;
}

export interface AccountSession {
  authenticated: boolean;
  user: AccountUser | null;
  has_unclaimed_legacy_data: boolean;
}

export interface AccountIdentity {
  authenticated: boolean;
  label: string;
  email: string | null;
  picture: string | null;
}

export interface SignOutOutcome {
  success: boolean;
  partial: boolean;
  failedSteps: string[];
  warning: string | null;
}

export function resolveAccountIdentity(
  session: AccountSession | null | undefined,
  fallbackLabel?: string
): AccountIdentity;

export function shouldDisplayLegacyPrompt(
  session: AccountSession | null | undefined
): boolean;

export function handleSignOutResult(
  result: { status?: string; failed_steps?: string[]; error?: string } | null | undefined
): SignOutOutcome;
