import type { Action } from '../types';

interface BadgeProps {
  action: Action;
  className?: string;
}

const config: Record<Action, { label: string; cls: string }> = {
  allow: { label: 'ALLOW', cls: 'bg-green-900 text-green-300 border border-green-700' },
  deny:  { label: 'DENY',  cls: 'bg-red-900 text-red-300 border border-red-700' },
  alert: { label: 'ALERT', cls: 'bg-yellow-900 text-yellow-300 border border-yellow-700' },
};

export default function Badge({ action, className = '' }: BadgeProps) {
  const { label, cls } = config[action];
  return (
    <span className={`inline-flex items-center px-2 py-0.5 rounded text-xs font-bold tracking-wider ${cls} ${className}`}>
      {label}
    </span>
  );
}
