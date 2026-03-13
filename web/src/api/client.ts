import type {
  AccessRule,
  RulesResponse,
  HealthResponse,
  SimulateRequest,
  SimulateResponse,
  SecretInfo,
  RotationStatus,
} from '../types';

const getConfig = () => ({
  url: localStorage.getItem('DAC_URL') || 'http://localhost:8080',
  apiKey: localStorage.getItem('DAC_API_KEY') || 'change-me-in-production',
});

async function request<T>(
  path: string,
  options: RequestInit = {},
  skipAuth = false
): Promise<T> {
  const { url, apiKey } = getConfig();
  const headers: Record<string, string> = {
    'Content-Type': 'application/json',
    ...(options.headers as Record<string, string>),
  };
  if (!skipAuth) {
    headers['Authorization'] = `Bearer ${apiKey}`;
  }

  const res = await fetch(`${url}${path}`, { ...options, headers });

  if (!res.ok) {
    let msg = `HTTP ${res.status}`;
    try {
      const body = await res.json();
      msg = body.message || body.error || JSON.stringify(body);
    } catch {
      msg = await res.text().catch(() => msg);
    }
    throw new Error(msg);
  }

  // 204 No Content
  if (res.status === 204) return undefined as T;

  return res.json() as Promise<T>;
}

// Health
export const getHealth = () => request<HealthResponse>('/health', {}, true);

// Rules
export const getRules = () => request<RulesResponse>('/api/rules');
export const getRule = (id: string) => request<AccessRule>(`/api/rules/${id}`);
export const createRule = (rule: Omit<AccessRule, 'id'>) =>
  request<AccessRule>('/api/rules', { method: 'POST', body: JSON.stringify(rule) });
export const updateRule = (id: string, rule: Partial<AccessRule>) =>
  request<AccessRule>(`/api/rules/${id}`, { method: 'PUT', body: JSON.stringify(rule) });
export const deleteRule = (id: string) =>
  request<void>(`/api/rules/${id}`, { method: 'DELETE' });
export const toggleRule = (id: string) =>
  request<AccessRule>(`/api/rules/${id}/toggle`, { method: 'POST' });
export const reloadRules = () =>
  request<{ message: string }>('/api/rules/reload', { method: 'POST' });
export const exportRules = () => request<unknown>('/api/rules/export');

// Simulator
export const simulate = (payload: SimulateRequest) =>
  request<SimulateResponse>('/api/audit/simulate', {
    method: 'POST',
    body: JSON.stringify(payload),
  });

// Secrets
export const getSecret = (id: string) => request<SecretInfo>(`/api/secrets/${id}`);
export const refreshSecret = (id: string) =>
  request<SecretInfo>(`/api/secrets/${id}/refresh`, { method: 'POST' });
export const getRotationStatus = (id: string) =>
  request<RotationStatus>(`/api/secrets/${id}/rotation`);
