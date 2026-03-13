import { useEffect, useState, useCallback } from 'react';
import {
  getRules, toggleRule, deleteRule, createRule,
  reloadRules, exportRules,
} from '../api/client';
import type { AccessRule, Condition } from '../types';
import Badge from '../components/Badge';
import Modal from '../components/Modal';

function conditionSummary(c: Condition): string {
  switch (c.type) {
    case 'ip_range': return `IP: ${c.cidr}`;
    case 'db_user': return `DB유저: ${c.pattern}`;
    case 'db_type': return `DB종류: ${c.db_type}`;
    case 'target_db': return `대상DB: ${c.pattern}`;
    case 'query_pattern': return `쿼리패턴: ${c.regex}`;
    case 'time_range': return `시간: ${c.start_hour}~${c.end_hour}시`;
    case 'iam_arn': return `IAM: ${c.pattern}`;
    case 'block_dangerous_query': return '위험 쿼리 차단';
    default: return JSON.stringify(c);
  }
}

const defaultJson = JSON.stringify(
  {
    name: '새 규칙',
    description: '설명 (선택)',
    priority: 100,
    action: 'deny',
    enabled: true,
    conditions: [{ type: 'ip_range', cidr: '10.0.0.0/8' }],
  },
  null,
  2
);

export default function Rules() {
  const [rules, setRules] = useState<AccessRule[]>([]);
  const [loading, setLoading] = useState(true);
  const [toast, setToast] = useState<{ msg: string; type: 'ok' | 'err' } | null>(null);
  const [addOpen, setAddOpen] = useState(false);
  const [detailRule, setDetailRule] = useState<AccessRule | null>(null);
  const [jsonInput, setJsonInput] = useState(defaultJson);
  const [jsonError, setJsonError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  const showToast = (msg: string, type: 'ok' | 'err' = 'ok') => {
    setToast({ msg, type });
    setTimeout(() => setToast(null), 3500);
  };

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const data = await getRules();
      setRules(data.rules);
    } catch (e) {
      showToast((e as Error).message, 'err');
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { load(); }, [load]);

  const handleToggle = async (rule: AccessRule) => {
    try {
      const updated = await toggleRule(rule.id);
      setRules((prev) => prev.map((r) => (r.id === rule.id ? updated : r)));
      showToast(`"${rule.name}" ${updated.enabled ? '활성화' : '비활성화'} 완료`);
    } catch (e) {
      showToast((e as Error).message, 'err');
    }
  };

  const handleDelete = async (rule: AccessRule) => {
    if (!confirm(`"${rule.name}" 규칙을 삭제하시겠습니까?`)) return;
    try {
      await deleteRule(rule.id);
      setRules((prev) => prev.filter((r) => r.id !== rule.id));
      showToast(`"${rule.name}" 삭제 완료`);
    } catch (e) {
      showToast((e as Error).message, 'err');
    }
  };

  const handleReload = async () => {
    try {
      await reloadRules();
      showToast('규칙 리로드 완료');
      load();
    } catch (e) {
      showToast((e as Error).message, 'err');
    }
  };

  const handleExport = async () => {
    try {
      const data = await exportRules();
      const blob = new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = 'dac-rules.json';
      a.click();
      URL.revokeObjectURL(url);
      showToast('내보내기 완료');
    } catch (e) {
      showToast((e as Error).message, 'err');
    }
  };

  const handleAdd = async () => {
    setJsonError(null);
    let parsed: Omit<AccessRule, 'id'>;
    try {
      parsed = JSON.parse(jsonInput);
    } catch {
      setJsonError('유효하지 않은 JSON입니다.');
      return;
    }
    setSubmitting(true);
    try {
      const created = await createRule(parsed);
      setRules((prev) => [...prev, created]);
      setAddOpen(false);
      setJsonInput(defaultJson);
      showToast(`"${created.name}" 생성 완료`);
    } catch (e) {
      setJsonError((e as Error).message);
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="p-8">
      {/* Toast */}
      {toast && (
        <div className={`fixed top-5 right-5 z-50 px-5 py-3 rounded-lg shadow-lg text-sm font-medium transition-all ${
          toast.type === 'ok' ? 'bg-green-800 border border-green-600 text-green-100' : 'bg-red-800 border border-red-600 text-red-100'
        }`}>
          {toast.type === 'ok' ? '✓ ' : '✗ '}{toast.msg}
        </div>
      )}

      <div className="flex items-center justify-between mb-8">
        <div>
          <h1 className="text-2xl font-bold text-white">접근 규칙</h1>
          <p className="text-gray-500 text-sm mt-1">총 {rules.length}개의 규칙</p>
        </div>
        <div className="flex items-center gap-2">
          <button
            onClick={handleReload}
            className="flex items-center gap-2 px-4 py-2 text-sm bg-gray-700 hover:bg-gray-600 text-gray-200 rounded-lg border border-gray-600 transition-colors"
          >
            <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
            </svg>
            리로드
          </button>
          <button
            onClick={handleExport}
            className="flex items-center gap-2 px-4 py-2 text-sm bg-gray-700 hover:bg-gray-600 text-gray-200 rounded-lg border border-gray-600 transition-colors"
          >
            <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4" />
            </svg>
            내보내기
          </button>
          <button
            onClick={() => setAddOpen(true)}
            className="flex items-center gap-2 px-4 py-2 text-sm bg-indigo-600 hover:bg-indigo-500 text-white rounded-lg font-medium transition-colors"
          >
            <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4v16m8-8H4" />
            </svg>
            규칙 추가
          </button>
        </div>
      </div>

      <div className="bg-gray-800 border border-gray-700 rounded-xl overflow-hidden">
        <table className="w-full">
          <thead>
            <tr className="border-b border-gray-700 bg-gray-900/50">
              <th className="text-left text-xs font-semibold text-gray-400 uppercase tracking-wider px-5 py-3.5">이름</th>
              <th className="text-left text-xs font-semibold text-gray-400 uppercase tracking-wider px-5 py-3.5">우선순위</th>
              <th className="text-left text-xs font-semibold text-gray-400 uppercase tracking-wider px-5 py-3.5">액션</th>
              <th className="text-left text-xs font-semibold text-gray-400 uppercase tracking-wider px-5 py-3.5">상태</th>
              <th className="text-left text-xs font-semibold text-gray-400 uppercase tracking-wider px-5 py-3.5">조건 수</th>
              <th className="text-right text-xs font-semibold text-gray-400 uppercase tracking-wider px-5 py-3.5">작업</th>
            </tr>
          </thead>
          <tbody>
            {loading ? (
              [...Array(4)].map((_, i) => (
                <tr key={i} className="border-b border-gray-700/50">
                  {[...Array(6)].map((_, j) => (
                    <td key={j} className="px-5 py-4">
                      <div className="h-4 bg-gray-700 rounded animate-pulse" />
                    </td>
                  ))}
                </tr>
              ))
            ) : rules.length === 0 ? (
              <tr>
                <td colSpan={6} className="px-5 py-12 text-center text-gray-500">
                  규칙이 없습니다. 새 규칙을 추가해 보세요.
                </td>
              </tr>
            ) : (
              rules.map((rule) => (
                <tr
                  key={rule.id}
                  className="border-b border-gray-700/50 hover:bg-gray-700/30 transition-colors"
                >
                  <td className="px-5 py-4">
                    <button
                      onClick={() => setDetailRule(rule)}
                      className="text-left"
                    >
                      <div className="font-medium text-white hover:text-indigo-300 transition-colors">{rule.name}</div>
                      {rule.description && (
                        <div className="text-xs text-gray-500 mt-0.5 truncate max-w-xs">{rule.description}</div>
                      )}
                      <div className="text-xs text-gray-600 font-mono mt-0.5">{rule.id.slice(0, 8)}…</div>
                    </button>
                  </td>
                  <td className="px-5 py-4">
                    <span className="text-sm font-mono text-gray-300">{rule.priority}</span>
                  </td>
                  <td className="px-5 py-4">
                    <Badge action={rule.action} />
                  </td>
                  <td className="px-5 py-4">
                    <button
                      onClick={() => handleToggle(rule)}
                      className={`inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full text-xs font-medium transition-colors ${
                        rule.enabled
                          ? 'bg-green-900/40 text-green-400 hover:bg-green-900/70 border border-green-700/50'
                          : 'bg-gray-700/40 text-gray-400 hover:bg-gray-700/70 border border-gray-600/50'
                      }`}
                    >
                      <div className={`w-1.5 h-1.5 rounded-full ${rule.enabled ? 'bg-green-400' : 'bg-gray-500'}`} />
                      {rule.enabled ? '활성' : '비활성'}
                    </button>
                  </td>
                  <td className="px-5 py-4">
                    <span className="text-sm text-gray-400">{rule.conditions.length}개</span>
                  </td>
                  <td className="px-5 py-4 text-right">
                    <button
                      onClick={() => handleDelete(rule)}
                      className="text-gray-500 hover:text-red-400 transition-colors p-1.5 rounded hover:bg-red-900/30"
                      title="삭제"
                    >
                      <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                      </svg>
                    </button>
                  </td>
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>

      {/* Add Rule Modal */}
      <Modal open={addOpen} onClose={() => { setAddOpen(false); setJsonError(null); }} title="규칙 추가 (JSON)">
        <div className="space-y-4">
          <p className="text-sm text-gray-400">
            규칙 설정을 JSON 형식으로 입력하세요. <code className="text-indigo-300">id</code>는 서버에서 자동 생성됩니다.
          </p>
          <textarea
            value={jsonInput}
            onChange={(e) => setJsonInput(e.target.value)}
            className="w-full h-72 bg-gray-900 border border-gray-600 rounded-lg px-4 py-3 text-sm text-green-300 font-mono focus:outline-none focus:border-indigo-500 resize-none"
            spellCheck={false}
          />
          {jsonError && (
            <div className="text-sm text-red-400 bg-red-900/20 border border-red-800 rounded-lg p-3">
              {jsonError}
            </div>
          )}
          <div className="flex justify-end gap-3">
            <button onClick={() => { setAddOpen(false); setJsonError(null); }} className="px-4 py-2 text-sm text-gray-400 hover:text-white border border-gray-600 rounded-lg hover:border-gray-500 transition-colors">
              취소
            </button>
            <button
              onClick={handleAdd}
              disabled={submitting}
              className="px-4 py-2 text-sm bg-indigo-600 hover:bg-indigo-500 disabled:opacity-50 text-white rounded-lg font-medium transition-colors"
            >
              {submitting ? '저장 중…' : '저장'}
            </button>
          </div>
        </div>
      </Modal>

      {/* Detail Modal */}
      <Modal open={!!detailRule} onClose={() => setDetailRule(null)} title="규칙 상세">
        {detailRule && (
          <div className="space-y-4">
            <div className="grid grid-cols-2 gap-4">
              <div>
                <div className="text-xs text-gray-500 mb-1">이름</div>
                <div className="text-sm text-white font-medium">{detailRule.name}</div>
              </div>
              <div>
                <div className="text-xs text-gray-500 mb-1">액션</div>
                <Badge action={detailRule.action} />
              </div>
              <div>
                <div className="text-xs text-gray-500 mb-1">우선순위</div>
                <div className="text-sm text-gray-300 font-mono">{detailRule.priority}</div>
              </div>
              <div>
                <div className="text-xs text-gray-500 mb-1">상태</div>
                <div className={`text-sm font-medium ${detailRule.enabled ? 'text-green-400' : 'text-gray-500'}`}>
                  {detailRule.enabled ? '활성' : '비활성'}
                </div>
              </div>
            </div>
            {detailRule.description && (
              <div>
                <div className="text-xs text-gray-500 mb-1">설명</div>
                <div className="text-sm text-gray-300">{detailRule.description}</div>
              </div>
            )}
            <div>
              <div className="text-xs text-gray-500 mb-2">조건 ({detailRule.conditions.length}개)</div>
              <div className="space-y-1.5">
                {detailRule.conditions.map((c, i) => (
                  <div key={i} className="text-xs font-mono bg-gray-900 rounded px-3 py-2 text-gray-300 border border-gray-700">
                    {conditionSummary(c)}
                  </div>
                ))}
              </div>
            </div>
            <div>
              <div className="text-xs text-gray-500 mb-1">ID</div>
              <div className="text-xs font-mono text-gray-500 break-all">{detailRule.id}</div>
            </div>
          </div>
        )}
      </Modal>
    </div>
  );
}
