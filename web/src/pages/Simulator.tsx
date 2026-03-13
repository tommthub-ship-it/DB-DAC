import { useState } from 'react';
import { simulate } from '../api/client';
import type { SimulateResponse } from '../types';

const DB_TYPES = ['mysql', 'postgres', 'mongodb', 'redis', 'elasticsearch', 'oracle', 'mssql'];

export default function Simulator() {
  const [form, setForm] = useState({
    client_ip: '',
    db_user: '',
    db_type: 'postgres',
    target_db: '',
    query: '',
  });
  const [result, setResult] = useState<SimulateResponse | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const set = (key: string) => (e: React.ChangeEvent<HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement>) =>
    setForm((f) => ({ ...f, [key]: e.target.value }));

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    setResult(null);
    setLoading(true);
    try {
      const payload = {
        ...form,
        query: form.query || undefined,
      };
      const res = await simulate(payload);
      setResult(res);
    } catch (err) {
      setError((err as Error).message);
    } finally {
      setLoading(false);
    }
  };

  const inputCls = "w-full bg-gray-900 border border-gray-600 rounded-lg px-3 py-2.5 text-sm text-white placeholder-gray-500 focus:outline-none focus:border-indigo-500 focus:ring-1 focus:ring-indigo-500 transition-colors";
  const labelCls = "block text-sm font-medium text-gray-300 mb-1.5";

  return (
    <div className="p-8 max-w-3xl">
      <div className="mb-8">
        <h1 className="text-2xl font-bold text-white">정책 시뮬레이터</h1>
        <p className="text-gray-500 text-sm mt-1">접속 시나리오를 입력하여 정책 결과를 미리 확인합니다</p>
      </div>

      <div className="bg-gray-800 border border-gray-700 rounded-xl p-6 mb-6">
        <form onSubmit={handleSubmit} className="space-y-5">
          <div className="grid grid-cols-1 md:grid-cols-2 gap-5">
            <div>
              <label className={labelCls}>클라이언트 IP</label>
              <input
                type="text"
                placeholder="예: 192.168.1.100"
                value={form.client_ip}
                onChange={set('client_ip')}
                required
                className={inputCls}
              />
            </div>
            <div>
              <label className={labelCls}>DB 유저</label>
              <input
                type="text"
                placeholder="예: app_user"
                value={form.db_user}
                onChange={set('db_user')}
                required
                className={inputCls}
              />
            </div>
            <div>
              <label className={labelCls}>DB 종류</label>
              <select value={form.db_type} onChange={set('db_type')} className={inputCls}>
                {DB_TYPES.map((t) => (
                  <option key={t} value={t}>{t}</option>
                ))}
              </select>
            </div>
            <div>
              <label className={labelCls}>대상 DB</label>
              <input
                type="text"
                placeholder="예: production-db"
                value={form.target_db}
                onChange={set('target_db')}
                required
                className={inputCls}
              />
            </div>
          </div>

          <div>
            <label className={labelCls}>
              쿼리 <span className="text-gray-500 font-normal">(선택)</span>
            </label>
            <textarea
              placeholder="SELECT * FROM users WHERE id = 1"
              value={form.query}
              onChange={set('query')}
              rows={3}
              className={`${inputCls} resize-none font-mono`}
            />
          </div>

          <button
            type="submit"
            disabled={loading}
            className="w-full flex items-center justify-center gap-2 px-6 py-3 bg-indigo-600 hover:bg-indigo-500 disabled:opacity-50 disabled:cursor-not-allowed text-white font-semibold rounded-lg transition-colors"
          >
            {loading ? (
              <>
                <svg className="w-4 h-4 animate-spin" fill="none" viewBox="0 0 24 24">
                  <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
                  <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
                </svg>
                시뮬레이션 중…
              </>
            ) : (
              <>
                <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M14.752 11.168l-3.197-2.132A1 1 0 0010 9.87v4.263a1 1 0 001.555.832l3.197-2.132a1 1 0 000-1.664z" />
                </svg>
                시뮬레이션 실행
              </>
            )}
          </button>
        </form>
      </div>

      {error && (
        <div className="p-4 bg-red-900/30 border border-red-700 rounded-xl text-red-300 text-sm">
          ✗ {error}
        </div>
      )}

      {result && (
        <div className={`rounded-xl border p-6 ${
          result.decision === 'allow'
            ? 'bg-green-900/20 border-green-700'
            : 'bg-red-900/20 border-red-700'
        }`}>
          <div className="flex items-center gap-4 mb-5">
            <div className={`w-16 h-16 rounded-full flex items-center justify-center text-2xl font-black ${
              result.decision === 'allow'
                ? 'bg-green-500/20 text-green-400 border-2 border-green-500'
                : 'bg-red-500/20 text-red-400 border-2 border-red-500'
            }`}>
              {result.decision === 'allow' ? '✓' : '✗'}
            </div>
            <div>
              <div className={`text-3xl font-black tracking-widest ${
                result.decision === 'allow' ? 'text-green-400' : 'text-red-400'
              }`}>
                {result.decision.toUpperCase()}
              </div>
              <div className="text-sm text-gray-400 mt-0.5">정책 결정 완료</div>
            </div>
          </div>

          <div className="space-y-3">
            <div>
              <div className="text-xs text-gray-500 uppercase tracking-wider mb-1">사유</div>
              <div className="text-sm text-gray-200">{result.reason}</div>
            </div>

            {result.matched_rule && (
              <div className="bg-gray-900/50 rounded-lg p-4 border border-gray-700">
                <div className="text-xs text-gray-500 uppercase tracking-wider mb-2">매칭 규칙</div>
                <div className="text-sm text-white font-medium">{result.matched_rule.name}</div>
                <div className="text-xs text-gray-500 font-mono mt-0.5">{result.matched_rule.id}</div>
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
