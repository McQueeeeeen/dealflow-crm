export type Role = "admin" | "manager" | "sales_rep";
export type StageKind = "open" | "won" | "lost";

export interface Me {
  id: string;
  email: string;
  full_name: string;
  role: Role;
  organization_id: string;
  organization_name: string;
  currency: string;
}

export interface Page<T> {
  data: T[];
  page: number;
  per_page: number;
  total: number;
}

export interface User {
  id: string;
  email: string;
  full_name: string;
  role: Role;
  is_active: boolean;
  created_at: string;
}

export interface Stage {
  id: string;
  name: string;
  position: number;
  probability: number;
  kind: StageKind;
}

export interface Company {
  id: string;
  name: string;
  domain: string | null;
  industry: string | null;
  owner_id: string;
  owner_name: string;
  contacts_count: number;
  open_deals_count: number;
  open_deals_value_cents: number;
  created_at: string;
  updated_at: string;
}

export interface Contact {
  id: string;
  first_name: string;
  last_name: string;
  email: string | null;
  phone: string | null;
  job_title: string | null;
  company_id: string | null;
  company_name: string | null;
  owner_id: string;
  owner_name: string;
  created_at: string;
  updated_at: string;
}

export interface Deal {
  id: string;
  title: string;
  value_cents: number;
  stage_id: string;
  stage_name: string;
  stage_kind: StageKind;
  probability: number;
  board_position: number;
  company_id: string | null;
  company_name: string | null;
  contact_id: string | null;
  contact_name: string | null;
  owner_id: string;
  owner_name: string;
  expected_close_date: string | null;
  closed_at: string | null;
  version: number;
  created_at: string;
  updated_at: string;
}

export interface BoardColumn {
  stage: Stage;
  deals: Deal[];
  count: number;
  total_value_cents: number;
}

export interface StageChange {
  from_stage_name: string | null;
  to_stage_name: string;
  changed_by_name: string | null;
  changed_at: string;
}

export interface Task {
  id: string;
  title: string;
  details: string | null;
  due_at: string | null;
  completed_at: string | null;
  assignee_id: string;
  assignee_name: string;
  created_by: string;
  deal_id: string | null;
  deal_title: string | null;
  contact_id: string | null;
  contact_name: string | null;
  created_at: string;
  updated_at: string;
}

export interface Note {
  id: string;
  body: string;
  author_id: string;
  author_name: string;
  deal_id: string | null;
  contact_id: string | null;
  company_id: string | null;
  created_at: string;
}

export interface AuditEntry {
  id: number;
  actor_id: string | null;
  actor_name: string | null;
  entity_type: string;
  entity_id: string;
  action: "created" | "updated" | "deleted";
  changes: Record<string, unknown>;
  created_at: string;
}

export interface Summary {
  open_deals_count: number;
  open_pipeline_cents: number;
  weighted_pipeline_cents: number;
  won_this_month_cents: number;
  won_this_month_count: number;
  win_rate_90d: number | null;
  avg_days_to_win: number | null;
}

export interface FunnelStage {
  stage_id: string;
  name: string;
  kind: StageKind;
  reached: number;
  conversion_from_previous: number | null;
}

export interface Funnel {
  from: string;
  cohort_size: number;
  stages: FunnelStage[];
}

export interface ForecastMonth {
  month: string;
  deals: number;
  total_cents: number;
  weighted_cents: number;
}

export interface Forecast {
  months: ForecastMonth[];
  overdue_cents: number;
  undated_cents: number;
}

export interface LeaderboardRow {
  user_id: string;
  full_name: string;
  won_count: number;
  won_cents: number;
  lost_count: number;
  open_pipeline_cents: number;
  win_rate: number | null;
}

export interface FieldError {
  field: string;
  message: string;
}
