import { useState, useEffect, useCallback, useRef } from 'react';
import type { AuditEvent } from '../types';
import { getAuditLogs } from '../api/client';

const PAGE_SIZE = 100;

const EVENT_TYPE_OPTIONS = [
  { value: '', label: '전체 이벤트' },
  { value: 'connection_attempt', label: 'Connection Attempt' },
  { value: 'connection_allowed', label: 'Connection Allowed' },
  { value: 'connection_denied', label: 'Connection Denied' },
  { value: 'query_executed', label: 'Query Executed' },
  { value: 'query_blocked', label: 'Query Blocked' },
  { value: 'query_alert', label: 'Query Alert' },
  { value: 'policy_changed', label: 'Policy Changed' },
];

function eventTypeColor(et: AuditEvent['event_type']): string {
  switch (et) {
    case 'connection_allowed':
    case 'query_executed':
      return 'text-green-400';
    case 'connection_denied':
    case 'query_blocked':
      return 'text-red-400';
    case 'query_alert':
      return 'text-yellow-400';
    case 'policy_changed':
      return 'text-blue-400';
    default:
      return 'text-gray-400';
  }
}

function eventTypeBg(et: AuditEvent['event_type']): string {
  switch (et) {
    case 'connection_allowed':
    case 'query_executed':
      return 'bg-green-900/40 border border-green-700/40';
    case 'connection_denied':
    case 'query_blocked':
      return 'bg-red-900/40 border border-red-700/40';
    case 'query_alert':
      return 'bg-yellow-900/40 border border-yellow-700/40';
    case 'policy_changed':
      return 'bg-blue-900/40 border border-blue-700/40';
    default:
      return 'bg-gray-800/40 border border-gray-700/40';
  }
}

function formatTs(ts: string): string {
  try {
    return new Date(ts).toLocaleString('ko-KR', { timeZone: 'Asia/Seoul' });
  } catch {
    return ts;
  }
}

export default function AuditLogs() {
  const [logs, setLogs] = useState<AuditEvent[]>([]);
  const [total, setTotal] = useState(0);
  const [hasMore, setHasMore] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Filters
  const [eventType, setEventType] = useState('');
  const [dbUser, setDbUser] = useState('');
  const [clientIp, setClientIp] = useState('');
  const [allowedFilter, setAllowedFilter] = useState<'all' | 'allowed' | 'denied'>('all');

  // Pagination
  const [offset, setOffset] = useState(0);

  // Auto-refresh
  const [autoRefresh, setAutoRefresh] = useState(false);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const fetchLogs = useCallback(async (off = offset) => {
    setLoading(true);
    setError(null);
    try {
      const params: Parameters<typeof getAuditLogs>[0] = {
        limit: PAGE_SIZE,
        offset: off,
      };
      if (eventType) params.event_type = eventType;
      if (dbUser) params.db_user = dbUser;
      if (clientIp) params.client_ip = clientIp;
      if (allowedFilter === 'allowed') params.allowed = true;
      if (allowedFilter === 'denied') params.allowed = false;

      const res = await getAuditLogs(params);
      setLogs(res.logs);
      setTotal(res.total);
      setHasMore(res.has_more);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [offset, eventType, dbUser, clientIp, allowedFilter]);

  // Initial load & filter change → reset to page 0
  useEffect(() => {
    setOffset(0);
    fetchLogs(0);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [eventType, dbUser, clientIp, allowedFilter]);

  useEffect(() => {
    fetchLogs(offset);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [offset]);

  // Auto-refresh
  useEffect(() => {
    if (autoRefresh) {
      intervalRef.current = setInterval(() => fetchLogs(offset), 5000);
    } else {
      if (intervalRef.current) clearInterval(intervalRef.current);
    }
    return () => {
      if (intervalRef.current) clearInterval(intervalRef.current);
    };
  }, [autoRefresh, fetchLogs, offset]);

  const handleSearch = () => {
    setOffset(0);
    fetchLogs(0);
  };

  const totalPages = Math.ceil(total / PAGE_SIZE);
  const currentPage = Math.floor(offset / PAGE_SIZE) + 1;

  return (
    <div className="p-6 space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-white">감사 로그</h1>
          <p className="text-sm text-gray-400 mt-0.5">개인정보보호법 §29 접근이력 관리</p>
        </div>
        <div className="flex items-center gap-3">
          <label className="flex items-center gap-2 text-sm text-gray-400 cursor-pointer select-none">
            <div
              onClick={() => setAutoRefresh(v => !v)}
              className={`relative w-10 h-5 rounded-full transition-colors cursor-pointer ${autoRefresh ? 'bg-indigo-600' : 'bg-gray-700'}`}
            >
              <span className={`absolute top-0.5 left-0.5 w-4 h-4 rounded-full bg-white transition-transform ${autoRefresh ? 'translate-x-5' : ''}`} />
            </div>
            자동 새로고침 (5초)
          </label>
          <button
            onClick={() => fetchLogs(offset)}
            disabled={loading}
            className="px-3 py-2 text-sm bg-gray-700 hover:bg-gray-600 text-white rounded-lg transition-colors disabled:opacity-50"
          >
            {loading ? '로딩...' : '새로고침'}
          </button>
        </div>
      </div>

      {/* Filters */}
      <div className="bg-gray-800/60 border border-gray-700 rounded-xl p-4 space-y-3">
        <div className="flex flex-wrap gap-3 items-end">
          {/* Event Type */}
          <div className="flex-1 min-w-[160px]">
            <label className="block text-xs text-gray-400 mb-1">이벤트 타입</label>
            <select
              value={eventType}
              onChange={e => setEventType(e.target.value)}
              className="w-full bg-gray-900 border border-gray-600 rounded-lg px-3 py-2 text-sm text-white focus:outline-none focus:border-indigo-500"
            >
              {EVENT_TYPE_OPTIONS.map(o => (
                <option key={o.value} value={o.value}>{o.label}</option>
              ))}
            </select>
          </div>

          {/* DB User */}
          <div className="flex-1 min-w-[140px]">
            <label className="block text-xs text-gray-400 mb-1">DB 사용자</label>
            <input
              type="text"
              value={dbUser}
              onChange={e => setDbUser(e.target.value)}
              placeholder="부분매칭..."
              className="w-full bg-gray-900 border border-gray-600 rounded-lg px-3 py-2 text-sm text-white placeholder-gray-500 focus:outline-none focus:border-indigo-500"
            />
          </div>

          {/* Client IP */}
          <div className="flex-1 min-w-[140px]">
            <label className="block text-xs text-gray-400 mb-1">클라이언트 IP</label>
            <input
              type="text"
              value={clientIp}
              onChange={e => setClientIp(e.target.value)}
              placeholder="부분매칭..."
              className="w-full bg-gray-900 border border-gray-600 rounded-lg px-3 py-2 text-sm text-white placeholder-gray-500 focus:outline-none focus:border-indigo-500"
            />
          </div>

          {/* Allowed Filter */}
          <div>
            <label className="block text-xs text-gray-400 mb-1">결과</label>
            <div className="flex gap-1 bg-gray-900 border border-gray-600 rounded-lg p-1">
              {(['all', 'allowed', 'denied'] as const).map(v => (
                <button
                  key={v}
                  onClick={() => setAllowedFilter(v)}
                  className={`px-3 py-1 rounded text-xs font-medium transition-colors ${
                    allowedFilter === v
                      ? v === 'allowed' ? 'bg-green-600 text-white'
                        : v === 'denied' ? 'bg-red-600 text-white'
                        : 'bg-indigo-600 text-white'
                      : 'text-gray-400 hover:text-white'
                  }`}
                >
                  {v === 'all' ? 'ALL' : v === 'allowed' ? 'ALLOW' : 'DENY'}
                </button>
              ))}
            </div>
          </div>

          <button
            onClick={handleSearch}
            className="px-4 py-2 bg-indigo-600 hover:bg-indigo-500 text-white text-sm rounded-lg font-medium transition-colors"
          >
            검색
          </button>
        </div>
      </div>

      {/* Error */}
      {error && (
        <div className="bg-red-900/40 border border-red-700 rounded-lg px-4 py-3 text-sm text-red-300">
          {error}
        </div>
      )}

      {/* Stats */}
      <div className="flex items-center justify-between text-sm text-gray-400">
        <span>총 <strong className="text-white">{total.toLocaleString()}</strong>개 로그</span>
        {totalPages > 1 && (
          <span>{currentPage} / {totalPages} 페이지</span>
        )}
      </div>

      {/* Table */}
      <div className="bg-gray-800/60 border border-gray-700 rounded-xl overflow-hidden">
        {logs.length === 0 && !loading ? (
          <div className="flex flex-col items-center justify-center py-20 text-gray-500">
            <svg className="w-12 h-12 mb-3 opacity-30" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5}
                d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2" />
            </svg>
            <p className="text-sm">감사 로그가 없습니다</p>
            <p className="text-xs mt-1 text-gray-600">필터 조건을 변경하거나 로그가 생성될 때까지 기다려주세요.</p>
          </div>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-gray-700 bg-gray-900/50">
                  <th className="px-4 py-3 text-left text-xs font-medium text-gray-400 uppercase tracking-wider">시각</th>
                  <th className="px-4 py-3 text-left text-xs font-medium text-gray-400 uppercase tracking-wider">이벤트</th>
                  <th className="px-4 py-3 text-left text-xs font-medium text-gray-400 uppercase tracking-wider">클라이언트 IP</th>
                  <th className="px-4 py-3 text-left text-xs font-medium text-gray-400 uppercase tracking-wider">DB 사용자</th>
                  <th className="px-4 py-3 text-left text-xs font-medium text-gray-400 uppercase tracking-wider">DB 타입</th>
                  <th className="px-4 py-3 text-left text-xs font-medium text-gray-400 uppercase tracking-wider">대상 DB</th>
                  <th className="px-4 py-3 text-left text-xs font-medium text-gray-400 uppercase tracking-wider">쿼리</th>
                  <th className="px-4 py-3 text-left text-xs font-medium text-gray-400 uppercase tracking-wider">결과</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-gray-700/50">
                {logs.map((log) => (
                  <tr key={log.id} className="hover:bg-gray-700/20 transition-colors">
                    <td className="px-4 py-3 text-xs text-gray-400 whitespace-nowrap">
                      {formatTs(log.timestamp)}
                    </td>
                    <td className="px-4 py-3">
                      <span className={`inline-flex items-center px-2 py-0.5 rounded text-xs font-medium ${eventTypeBg(log.event_type)} ${eventTypeColor(log.event_type)}`}>
                        {log.event_type}
                      </span>
                    </td>
                    <td className="px-4 py-3 text-xs text-gray-300 font-mono whitespace-nowrap">
                      {log.client_ip}
                    </td>
                    <td className="px-4 py-3 text-xs text-gray-300 whitespace-nowrap">
                      {log.db_user}
                    </td>
                    <td className="px-4 py-3 text-xs text-gray-400 whitespace-nowrap">
                      {log.db_type}
                    </td>
                    <td className="px-4 py-3 text-xs text-gray-300 whitespace-nowrap">
                      {log.target_db}
                    </td>
                    <td className="px-4 py-3 text-xs text-gray-400 font-mono max-w-[200px]">
                      {log.query
                        ? <span title={log.query}>{log.query.length > 50 ? log.query.slice(0, 50) + '…' : log.query}</span>
                        : <span className="text-gray-600">—</span>}
                    </td>
                    <td className="px-4 py-3">
                      {log.allowed ? (
                        <span className="inline-flex items-center px-2 py-0.5 rounded text-xs font-bold bg-green-900/50 border border-green-700/50 text-green-400">
                          ALLOW
                        </span>
                      ) : (
                        <span className="inline-flex items-center px-2 py-0.5 rounded text-xs font-bold bg-red-900/50 border border-red-700/50 text-red-400">
                          DENY
                        </span>
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>

      {/* Pagination */}
      {totalPages > 1 && (
        <div className="flex items-center justify-center gap-2">
          <button
            onClick={() => setOffset(Math.max(0, offset - PAGE_SIZE))}
            disabled={offset === 0}
            className="px-3 py-1.5 text-sm bg-gray-700 hover:bg-gray-600 text-white rounded-lg transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
          >
            ← 이전
          </button>
          <span className="text-sm text-gray-400">
            {currentPage} / {totalPages}
          </span>
          <button
            onClick={() => setOffset(offset + PAGE_SIZE)}
            disabled={!hasMore}
            className="px-3 py-1.5 text-sm bg-gray-700 hover:bg-gray-600 text-white rounded-lg transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
          >
            다음 →
          </button>
        </div>
      )}
    </div>
  );
}
