export type Action = 'allow' | 'deny' | 'alert';

export type Condition =
  | { type: 'ip_range'; cidr: string }
  | { type: 'db_user'; pattern: string }
  | { type: 'db_type'; db_type: string }
  | { type: 'target_db'; pattern: string }
  | { type: 'query_pattern'; regex: string }
  | { type: 'time_range'; start_hour: number; end_hour: number; days: string[] }
  | { type: 'iam_arn'; pattern: string }
  | { type: 'block_dangerous_query' };

export interface AccessRule {
  id: string;
  name: string;
  description?: string;
  priority: number;
  action: Action;
  enabled: boolean;
  conditions: Condition[];
}

export interface RulesResponse {
  total: number;
  rules: AccessRule[];
}

export interface HealthResponse {
  status: string;
  [key: string]: unknown;
}

export interface SimulateRequest {
  client_ip: string;
  db_user: string;
  db_type: string;
  target_db: string;
  query?: string;
}

export interface SimulateResponse {
  decision: 'allow' | 'deny';
  reason: string;
  matched_rule?: {
    id: string;
    name: string;
    action: Action;
  };
}

export interface SecretInfo {
  username: string;
  password: string;
  engine: string;
  host: string;
  port: number;
  db_name: string;
  [key: string]: unknown;
}

export interface RotationStatus {
  rotation_enabled: boolean;
  last_rotated_date?: string;
  next_rotation_date?: string;
  rotation_days?: number;
}

export interface ApiConfig {
  url: string;
  apiKey: string;
}
