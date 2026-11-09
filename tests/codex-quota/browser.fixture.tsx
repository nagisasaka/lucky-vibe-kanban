import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';
import en from '../../packages/web-core/src/i18n/locales/en/common.json';
import ja from '../../packages/web-core/src/i18n/locales/ja/common.json';
void i18n.use(initReactI18next).init({
  lng: new URLSearchParams(location.search).get('lang') || 'ja',
  fallbackLng: 'en',
  resources: { en: { common: en }, ja: { common: ja } },
  initImmediate: false,
  interpolation: { escapeValue: false },
});
import { EnvironmentQuota } from '../../packages/web-core/src/shared/components/EnvironmentQuota';
import { claudeQuotaView } from '../../packages/web-core/src/shared/lib/claudeQuotaView';
import { quotaView } from '../../packages/ui/src/lib/codexQuota';
import { useTranslation } from 'react-i18next';
import { CodexQuota } from '../../packages/ui/src/components/CodexQuota';
import { useCodexQuota } from '../../packages/web-core/src/shared/hooks/useCodexQuota';
import { createRoot } from 'react-dom/client';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { GearIcon } from '@phosphor-icons/react';
import { Navbar } from '../../packages/ui/src/components/Navbar';
import { ResourceSummary } from '../../packages/ui/src/components/ResourceSummary';
import { useResourceObservation } from '../../packages/web-core/src/shared/hooks/useResourceObservation';

function DuplicateSubscriber() {
  useResourceObservation();
  useCodexQuota(true);
  return null;
}
function ClaudeFixture({ now }: { now: number }) {
  const quota = useCodexQuota(true, 'claude_code');
  const view = claudeQuotaView(quota.data, quota.unavailable, now);
  const { t } = useTranslation('common');
  return (
    <EnvironmentQuota
      name="Claude"
      {...quota}
      {...view}
      reason={view.reasonCode ? t(`utilityPanel.claudeQuotaReason.${view.reasonCode}`) : view.reason}
      now={now}
      rows={
        view.windows.length
          ? [
              {
                name: '',
                windows: view.windows.map((w) => ({
                  ...w,
                  label:
                    w.key === 'five_hour'
                      ? t('codexQuota.period.hours', { count: 5 })
                      : t('codexQuota.period.days', { count: 7 }),
                })),
              },
            ]
          : []
      }
    />
  );
}
function App() {
  const resources = useResourceObservation();
  const quota = useCodexQuota(resources.visible);
  const { t } = useTranslation('common');
  const params = new URLSearchParams(location.search);
  const testAge = params.get('age');
  const view = quotaView(quota.data, quota.unavailable, resources.now);
  if (new URLSearchParams(location.search).has('sidebar'))
    return (
      <div
        className="overflow-hidden bg-primary text-normal"
        style={{ width: Number(params.get('width') || 300) }}
      >
        <EnvironmentQuota
          name="Codex"
          {...quota}
          {...view}
          now={resources.now}
          observed={
            testAge === null
              ? view.observed
              : testAge === 'missing'
                ? null
                : testAge === 'invalid'
                  ? NaN
                  : resources.now - Number(testAge)
          }
          status={(params.get('state') as typeof view.status) || view.status}
          rows={view.rows.map((row) => ({
            ...row,
            name: row.name,
            windows: row.windows.map((w) => ({
              ...w,
              label: w.period
                ? t(`codexQuota.period.${w.period.unit}`, {
                    count: w.period.count,
                  })
                : t('codexQuota.unknownPeriod'),
            })),
          }))}
        />
        {params.has('dual') && <ClaudeFixture now={resources.now} />}
      </div>
    );
  return (
    <>
      <Navbar
        workspaceTitle="Example workspace"
        mobileMode={innerWidth < 768}
        isOnProjectPage={new URLSearchParams(location.search).has('project')}
        onReload={() => {}}
        onOpenSettings={() => {}}
        onOpenCommandBar={() => {}}
        rightItems={['Integration', 'Command Bar', 'Guide', 'Settings'].map(
          (label) => ({ id: label, icon: GearIcon, tooltip: label })
        )}
        mobileUserSlot={
          <button aria-label="User settings" className="size-10 shrink-0">
            <GearIcon />
          </button>
        }
        resourceSlot={
          <div className="flex min-w-0 flex-wrap xl:max-w-none xl:shrink-0 xl:flex-nowrap items-center gap-base">
            <ResourceSummary {...resources} />
            <CodexQuota {...quota} now={resources.now} />
          </div>
        }
      />
      <DuplicateSubscriber />
    </>
  );
}
createRoot(document.getElementById('root')!).render(
  <QueryClientProvider client={new QueryClient()}>
    <App />
  </QueryClientProvider>
);
