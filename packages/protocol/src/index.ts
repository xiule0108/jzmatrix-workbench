export type ResponseStatus = "pass" | "degraded" | "blocked" | "unknown";
export type CheckStatus = ResponseStatus | "not_run";

export interface DoctorCheck {
  id: string;
  category: string;
  status: CheckStatus;
  required: boolean;
  severity: "info" | "warning" | "error";
  observed_at: string;
  evidence: Array<{ kind: string; ref: string }>;
  remediation: Array<{ action_id: string; label: string }>;
  exit_impact: number;
  reason_code?: string | null;
}

export interface DoctorData {
  doctor_version: string;
  observed_at: string;
  checks: DoctorCheck[];
  summary: DoctorSummary;
  overall_status: ResponseStatus;
}

export interface DoctorSummary {
  pass: number;
  degraded: number;
  blocked: number;
  unknown: number;
  not_run: number;
}

export interface CliResponse<T = unknown> {
  contract: "jzmatrix.cli-response";
  version: string;
  command: string;
  request_id: string;
  ok: boolean;
  status: ResponseStatus;
  outcome: "not_started" | "committed" | "not_committed" | "unknown";
  data: T;
  errors: Array<{ code: string; message: string; retryable: boolean; outcome: string; details_ref?: string | null }>;
  warnings: Array<{ code: string; message: string }>;
  evidence: Array<{ kind: string; ref: string; observed_at?: string }>;
  next_actions: Array<{ action_id: string; label: string }>;
  redactions: { profile: string; fields: string[] };
  extensions: Record<string, unknown>;
}

export interface OfflineDemoEvent {
  kind: string;
  status: string;
}

export interface OfflineDemoGroup {
  id: string;
  goal: string;
  status: string;
  roles: string[];
  events: OfflineDemoEvent[];
}

export interface OfflineDemo {
  contract: "jzmatrix.offline-demo";
  version: string;
  data_source: "demo";
  title: string;
  groups: OfflineDemoGroup[];
  optional_agent: { status: "not_run"; reason: string };
}
