import { useState } from 'react';
import { getSecret, refreshSecret, getRotationStatus } from '../api/client';
import type { SecretInfo, RotationStatus } from '../types';

export default function Secrets() {
  const [secretId, setSecretId] = useState('');
  const [secret, setSecret] = useState<SecretInfo | null>(null);
  const [rotation, setRotation] = useState<RotationStatus | null>(null);
  const [showPassword, setShowPassword] = useState(false);
  const [loading, setLoading] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [toast, setToast] = useState<string | null>(null);

  const showToast = (msg: string) => {
    setToast(msg);
    setTimeout(() => setToast(null), 3000);
  };

  const handleFetch = async () => {
    if (!secretId.trim()) return;
    setError(null);
    setSecret(null);
    setRotation(null);
    setShowPassword(false);
    setLoading(true);
    try {
      const [s, r] = await Promise.allSettled([
        getSecret(secretId),
        getRotationStatus(secretId),
      ]);
      if (s.status === 'fulfilled') setSecret(s.value);
      else throw new Error((s.reason as Error).message);
      if (r.status === 'fulfilled') setRotation(r.value);
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setLoading(false);
    }
  };

  const handleRefresh = async () => {
    if (!secretId.trim()) return;
    setRefreshing(true);
    try {
      const updated = await refreshSecret(secretId);
      setSecret(updated);
      showToast('시크릿 갱신 완료');
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setRefreshing(false);
    }
  };

  const maskedPassword = '●'.repeat(16);

  return (
    <div className="p-8 max-w-3xl">
      {toast && (
        <div className="fixed top-5 right-5 z-50 px-5 py-3 rounded-lg shadow-lg text-sm font-medium bg-green-800 border border-green-600 text-green-100">
          ✓ {toast}
        </div>
      )}

      <div className="mb-8">
        <h1 className="text-2xl font-bold text-white">Secrets Manager</h1>
        <p className="text-gray-500 text-sm mt-1">AWS Secrets Manager에서 DB 자격증명을 조회합니다</p>
      </div>

      <div className="bg-gray-800 border border-gray-700 rounded-xl p-6 mb-6">
        <div className="flex gap-3">
          <input
            type="text"
            placeholder="Secret ID (예: prod/db/postgres)"
            value={secretId}
            onChange={(e) => setSecretId(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && handleFetch()}
            className="flex-1 bg-gray-900 border border-gray-600 rounded-lg px-4 py-2.5 text-sm text-white placeholder-gray-500 focus:outline-none focus:border-indigo-500 focus:ring-1 focus:ring-indigo-500 font-mono"
          />
          <button
            onClick={handleFetch}
            disabled={loading || !secretId.trim()}
            className="flex items-center gap-2 px-5 py-2.5 bg-indigo-600 hover:bg-indigo-500 disabled:opacity-50 text-white text-sm font-medium rounded-lg transition-colors"
          >
            {loading ? (
              <svg className="w-4 h-4 animate-spin" fill="none" viewBox="0 0 24 24">
                <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
                <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
              </svg>
            ) : (
              <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
              </svg>
            )}
            조회
          </button>
        </div>
      </div>

      {error && (
        <div className="mb-6 p-4 bg-red-900/30 border border-red-700 rounded-xl text-red-300 text-sm">
          ✗ {error}
        </div>
      )}

      {secret && (
        <>
          <div className="bg-gray-800 border border-gray-700 rounded-xl overflow-hidden mb-4">
            <div className="flex items-center justify-between px-5 py-4 border-b border-gray-700">
              <div>
                <h2 className="font-semibold text-white">자격증명 정보</h2>
                <div className="text-xs text-gray-500 font-mono mt-0.5">{secretId}</div>
              </div>
              <button
                onClick={handleRefresh}
                disabled={refreshing}
                className="flex items-center gap-2 px-3 py-2 text-sm bg-gray-700 hover:bg-gray-600 disabled:opacity-50 text-gray-200 rounded-lg border border-gray-600 transition-colors"
              >
                <svg className={`w-4 h-4 ${refreshing ? 'animate-spin' : ''}`} fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
                </svg>
                {refreshing ? '갱신 중…' : 'Refresh'}
              </button>
            </div>

            <table className="w-full">
              <tbody>
                {[
                  { label: '사용자명', value: secret.username },
                  { label: 'DB 엔진', value: secret.engine },
                  { label: '호스트', value: secret.host },
                  { label: '포트', value: String(secret.port) },
                  { label: 'DB 이름', value: secret.db_name },
                ].map(({ label, value }) => (
                  <tr key={label} className="border-b border-gray-700/50">
                    <td className="px-5 py-3.5 text-xs font-medium text-gray-500 uppercase tracking-wider w-32">{label}</td>
                    <td className="px-5 py-3.5 text-sm font-mono text-gray-200">{value ?? '-'}</td>
                  </tr>
                ))}
                <tr>
                  <td className="px-5 py-3.5 text-xs font-medium text-gray-500 uppercase tracking-wider">비밀번호</td>
                  <td className="px-5 py-3.5">
                    <div className="flex items-center gap-3">
                      <span className="text-sm font-mono text-gray-200">
                        {showPassword ? secret.password : maskedPassword}
                      </span>
                      <button
                        onClick={() => setShowPassword((v) => !v)}
                        className="text-xs text-gray-500 hover:text-gray-300 transition-colors border border-gray-600 rounded px-2 py-0.5"
                      >
                        {showPassword ? '숨기기' : '보기'}
                      </button>
                    </div>
                  </td>
                </tr>
              </tbody>
            </table>
          </div>

          {rotation && (
            <div className="bg-gray-800 border border-gray-700 rounded-xl p-5">
              <div className="flex items-center gap-3 mb-4">
                <div className={`w-2.5 h-2.5 rounded-full ${rotation.rotation_enabled ? 'bg-green-500' : 'bg-gray-600'}`} />
                <h3 className="font-semibold text-white text-sm">로테이션 상태</h3>
              </div>
              <div className="grid grid-cols-2 gap-4 text-sm">
                <div>
                  <div className="text-xs text-gray-500 mb-1">자동 로테이션</div>
                  <div className={rotation.rotation_enabled ? 'text-green-400 font-medium' : 'text-gray-500'}>
                    {rotation.rotation_enabled ? '활성화' : '비활성화'}
                  </div>
                </div>
                {rotation.rotation_days && (
                  <div>
                    <div className="text-xs text-gray-500 mb-1">주기</div>
                    <div className="text-gray-300">{rotation.rotation_days}일</div>
                  </div>
                )}
                {rotation.last_rotated_date && (
                  <div>
                    <div className="text-xs text-gray-500 mb-1">마지막 로테이션</div>
                    <div className="text-gray-300 font-mono">{rotation.last_rotated_date}</div>
                  </div>
                )}
                {rotation.next_rotation_date && (
                  <div>
                    <div className="text-xs text-gray-500 mb-1">다음 로테이션</div>
                    <div className="text-gray-300 font-mono">{rotation.next_rotation_date}</div>
                  </div>
                )}
              </div>
            </div>
          )}
        </>
      )}
    </div>
  );
}
