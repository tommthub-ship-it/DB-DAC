import { useEffect, useState } from 'react';
import { getHealth, getRules } from '../api/client';
import type { HealthResponse, RulesResponse } from '../types';

export default function Dashboard() {
  const [health, setHealth] = useState<HealthResponse | null>(null);
  const [healthError, setHealthError] = useState(false);
  const [rules, setRules] = useState<RulesResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const load = async () => {
      setLoading(true);
      setError(null);
      try {
        const [h, r] = await Promise.allSettled([getHealth(), getRules()]);
        if (h.status === 'fulfilled') {
          setHealth(h.value);
          setHealthError(false);
        } else {
          setHealthError(true);
        }
        if (r.status === 'fulfilled') {
          setRules(r.value);
        } else {
          setError((r.reason as Error).message);
        }
      } finally {
        setLoading(false);
      }
    };
    load();
  }, []);

  const activeCount = rules?.rules.filter((r) => r.enabled).length ?? 0;
  const inactiveCount = rules?.rules.filter((r) => !r.enabled).length ?? 0;
  const allowCount = rules?.rules.filter((r) => r.action === 'allow').length ?? 0;
  const denyCount = rules?.rules.filter((r) => r.action === 'deny').length ?? 0;
  const alertCount = rules?.rules.filter((r) => r.action === 'alert').length ?? 0;

  return (
    <div className="p-8">
      <div className="mb-8">
        <h1 className="text-2xl font-bold text-white">대시보드</h1>
        <p className="text-gray-500 text-sm mt-1">데이터베이스 접근 제어 현황</p>
      </div>

      {error && (
        <div className="mb-6 p-4 bg-red-900/30 border border-red-700 rounded-lg text-red-300 text-sm">
          ⚠️ {error}
        </div>
      )}

      {/* Server Status */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4 mb-8">
        <div className="bg-gray-800 border border-gray-700 rounded-xl p-5">
          <div className="flex items-center justify-between mb-3">
            <span className="text-xs font-medium text-gray-400 uppercase tracking-wider">서버 상태</span>
            <div className={`w-2.5 h-2.5 rounded-full ${healthError ? 'bg-red-500' : 'bg-green-500'} shadow-lg ${!healthError ? 'shadow-green-500/50' : 'shadow-red-500/50'} animate-pulse`} />
          </div>
          {loading ? (
            <div className="h-8 bg-gray-700 rounded animate-pulse" />
          ) : (
            <div className={`text-2xl font-bold ${healthError ? 'text-red-400' : 'text-green-400'}`}>
              {healthError ? 'OFFLINE' : 'ONLINE'}
            </div>
          )}
          {health && (
            <div className="text-xs text-gray-500 mt-1">{String(health.status ?? '')}</div>
          )}
        </div>

        <div className="bg-gray-800 border border-gray-700 rounded-xl p-5">
          <div className="text-xs font-medium text-gray-400 uppercase tracking-wider mb-3">총 규칙 수</div>
          {loading ? (
            <div className="h-8 bg-gray-700 rounded animate-pulse" />
          ) : (
            <div className="text-2xl font-bold text-white">{rules?.total ?? 0}</div>
          )}
        </div>

        <div className="bg-gray-800 border border-gray-700 rounded-xl p-5">
          <div className="text-xs font-medium text-gray-400 uppercase tracking-wider mb-3">활성 / 비활성</div>
          {loading ? (
            <div className="h-8 bg-gray-700 rounded animate-pulse" />
          ) : (
            <div className="flex items-baseline gap-2">
              <span className="text-2xl font-bold text-green-400">{activeCount}</span>
              <span className="text-gray-500">/</span>
              <span className="text-2xl font-bold text-gray-500">{inactiveCount}</span>
            </div>
          )}
        </div>

        <div className="bg-gray-800 border border-gray-700 rounded-xl p-5">
          <div className="text-xs font-medium text-gray-400 uppercase tracking-wider mb-3">API 엔드포인트</div>
          <div className="text-xs text-gray-400 font-mono break-all">
            {localStorage.getItem('DAC_URL') || 'http://localhost:8080'}
          </div>
        </div>
      </div>

      {/* Action Distribution */}
      <div className="bg-gray-800 border border-gray-700 rounded-xl p-6">
        <h2 className="text-sm font-semibold text-gray-300 uppercase tracking-wider mb-6">액션별 분포</h2>
        {loading ? (
          <div className="space-y-3">
            {[...Array(3)].map((_, i) => (
              <div key={i} className="h-10 bg-gray-700 rounded animate-pulse" />
            ))}
          </div>
        ) : (
          <div className="space-y-4">
            {[
              { label: 'ALLOW', count: allowCount, cls: 'bg-green-500', textCls: 'text-green-400' },
              { label: 'DENY', count: denyCount, cls: 'bg-red-500', textCls: 'text-red-400' },
              { label: 'ALERT', count: alertCount, cls: 'bg-yellow-500', textCls: 'text-yellow-400' },
            ].map(({ label, count, cls, textCls }) => {
              const total = rules?.total || 1;
              const pct = Math.round((count / total) * 100);
              return (
                <div key={label}>
                  <div className="flex justify-between items-center mb-1.5">
                    <span className={`text-sm font-bold ${textCls}`}>{label}</span>
                    <span className="text-sm text-gray-400">{count}개 ({pct}%)</span>
                  </div>
                  <div className="h-2 bg-gray-700 rounded-full overflow-hidden">
                    <div
                      className={`h-full ${cls} rounded-full transition-all duration-700`}
                      style={{ width: `${pct}%` }}
                    />
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}
