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

export interface GroupTemplate {
  id: string;
  version: string;
  label: string;
  description: string;
  roles: string[];
  data_source: "fixture";
}

export interface GroupRole {
  id: string;
  label: string;
  ordinal: number;
}

export interface GroupFact {
  kind: string;
  state: "unknown" | "observed" | "not_confirmed" | "not_run";
  evidence_ref?: string | null;
  observed_at: string;
}

export interface CollaborationGroup {
  id: string;
  data_source: "demo" | "fixture";
  goal: string;
  template_id: string;
  status: string;
  created_at: string;
  updated_at: string;
  roles: GroupRole[];
  facts: GroupFact[];
  replayed: boolean;
}

export interface ToolCatalogEntry {
  id: string;
  label: string;
  binary: string;
  description: string;
  scope: string;
}

export interface ToolDiscoveryResult {
  id: string;
  label: string;
  binary: string;
  status: "available" | "not_found" | "timed_out" | "failed";
  version?: string | null;
  version_exit_code?: number | null;
  help_status: "pass" | "not_run" | "timed_out" | "failed";
  help_exit_code?: number | null;
  advertised_flags: string[];
  output_sha256?: string | null;
  observed_at: string;
  reason?: string | null;
}

export interface ToolDiscoverySnapshot {
  contract: string;
  version: string;
  id: string;
  observed_at: string;
  data_source: "real";
  invocation: "user_triggered";
  network_requested: false;
  existing_sessions_read: false;
  external_writes: false;
  tools: ToolDiscoveryResult[];
  persisted: boolean;
}
