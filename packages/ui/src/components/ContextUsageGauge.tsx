import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { cn } from '../lib/cn';
import { Tooltip } from './Tooltip';

export interface ContextUsageInfo {
  total_tokens: number;
  model_context_window: number;
  cumulative_tokens?: number | bigint | null;
}

export function contextUsagePercentage(
  info?: ContextUsageInfo | null
): number | null {
  if (
    !info ||
    !Number.isSafeInteger(info.total_tokens) ||
    info.total_tokens < 0 ||
    !Number.isSafeInteger(info.model_context_window) ||
    info.model_context_window <= 0
  )
    return null;
  return Math.min(100, (info.total_tokens / info.model_context_window) * 100);
}

function clamp(value: number, min: number, max: number) {
  return Math.min(Math.max(value, min), max);
}

export interface ContextUsageGaugeProps {
  tokenUsageInfo?: ContextUsageInfo | null;
  className?: string;
}

export function ContextUsageGauge({
  tokenUsageInfo,
  className,
}: ContextUsageGaugeProps) {
  const { t } = useTranslation('common');
  const { percentage, formattedUsed, formattedTotal, status } = useMemo(() => {
    const validPercentage = contextUsagePercentage(tokenUsageInfo);
    if (!tokenUsageInfo || validPercentage === null) {
      return {
        percentage: 0,
        formattedUsed: '0',
        formattedTotal: '0',
        status: 'empty' as const,
      };
    }

    const pct = validPercentage;

    const formatTokens = (n: number) => {
      if (n >= 1_000_000) {
        const m = n / 1_000_000;
        return m % 1 === 0 ? `${m}M` : `${m.toFixed(1)}M`;
      }
      if (n >= 1_000) return `${Math.round(n / 1_000)}K`;
      return n.toString();
    };

    let statusValue: 'low' | 'medium' | 'high' | 'critical' | 'empty';
    if (pct < 50) statusValue = 'low';
    else if (pct < 75) statusValue = 'medium';
    else if (pct < 90) statusValue = 'high';
    else statusValue = 'critical';

    return {
      percentage: pct,
      formattedUsed: formatTokens(tokenUsageInfo.total_tokens),
      formattedTotal: formatTokens(tokenUsageInfo.model_context_window),
      status: statusValue,
    };
  }, [tokenUsageInfo]);

  const progress = clamp(percentage / 100, 0, 1);

  const contextTooltip =
    status === 'empty'
      ? t('contextUsage.emptyTooltip')
      : t('contextUsage.tooltip', {
          percentage: Math.round(percentage),
          used: formattedUsed,
          total: formattedTotal,
        });

  const cumulative =
    tokenUsageInfo?.cumulative_tokens == null
      ? null
      : Number(tokenUsageInfo.cumulative_tokens);
  const tooltip =
    cumulative !== null && Number.isSafeInteger(cumulative) && cumulative >= 0
      ? `${contextTooltip} · ${t('contextUsage.cumulative', { tokens: cumulative.toLocaleString() })}`
      : contextTooltip;

  const progressColor =
    status === 'empty'
      ? 'text-low/40'
      : status === 'critical'
        ? 'text-error'
        : status === 'high'
          ? 'text-brand-secondary'
          : status === 'medium'
            ? 'text-normal'
            : 'text-low';

  const radius = 8;
  const strokeWidth = 2;
  const circumference = 2 * Math.PI * radius;
  const dashOffset = circumference * (1 - progress);

  return (
    <Tooltip content={tooltip} side="bottom">
      <div
        className={cn(
          'flex items-center justify-center rounded-sm p-half',
          'hover:bg-panel transition-colors cursor-help',
          className
        )}
        aria-label={
          status === 'empty'
            ? t('contextUsage.label')
            : t('contextUsage.ariaLabel', {
                percentage: Math.round(percentage),
              })
        }
        role="img"
      >
        {status === 'empty' ? (
          <span aria-hidden="true">—</span>
        ) : (
          <svg
            viewBox="0 0 20 20"
            className="size-icon-base -rotate-90"
            aria-hidden="true"
          >
            <circle
              cx="10"
              cy="10"
              r={radius}
              fill="none"
              stroke="currentColor"
              strokeWidth={strokeWidth}
              className="text-border/60"
            />
            <circle
              cx="10"
              cy="10"
              r={radius}
              fill="none"
              stroke="currentColor"
              strokeWidth={strokeWidth}
              strokeLinecap="round"
              strokeDasharray={`${circumference} ${circumference}`}
              strokeDashoffset={dashOffset}
              className={cn(
                progressColor,
                'transition-all duration-500 ease-out'
              )}
            />
          </svg>
        )}
      </div>
    </Tooltip>
  );
}
