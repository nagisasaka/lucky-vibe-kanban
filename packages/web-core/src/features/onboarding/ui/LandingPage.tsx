import { useTranslation } from 'react-i18next';
import { notificationSoundLabel } from '@/shared/lib/notificationSoundLabel';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  BookOpenIcon,
  BirdIcon,
  CheckIcon,
  CowIcon,
  DeviceMobileIcon,
  GithubLogoIcon,
  MusicNoteIcon,
  MusicNotesIcon,
  SpeakerHighIcon,
  SpeakerXIcon,
  WarningIcon,
  WaveformIcon,
  type Icon,
} from '@phosphor-icons/react';
import { usePostHog } from 'posthog-js/react';
import { LVK_GITHUB_URL } from '@vibe/ui/lib/repository';
import {
  BaseCodingAgent,
  SoundFile,
  ThemeMode,
  type EditorConfig,
} from 'shared/types';
import { useUserSystem } from '@/shared/hooks/useUserSystem';
import { useTheme } from '@/shared/hooks/useTheme';
import { AgentIcon, getAgentName } from '@/shared/components/AgentIcon';
import { ExternalEditorSettings } from '@/shared/components/ExternalEditorSettings';
import { filterVisibleAgents } from '@/shared/lib/agentVisibility';
import { cn, playSound } from '@/shared/lib/utils';
import { useAppNavigation } from '@/shared/hooks/useAppNavigation';
import { PrimaryButton } from '@vibe/ui/components/PrimaryButton';

type SoundOption = {
  value: SoundFile;
  icon: Icon;
};

const SOUND_OPTIONS: SoundOption[] = [
  {
    value: SoundFile.ABSTRACT_SOUND1,
    icon: WaveformIcon,
  },
  {
    value: SoundFile.ABSTRACT_SOUND2,
    icon: MusicNoteIcon,
  },
  {
    value: SoundFile.ABSTRACT_SOUND3,
    icon: MusicNotesIcon,
  },
  {
    value: SoundFile.ABSTRACT_SOUND4,
    icon: SpeakerHighIcon,
  },
  {
    value: SoundFile.COW_MOOING,
    icon: CowIcon,
  },
  {
    value: SoundFile.PHONE_VIBRATION,
    icon: DeviceMobileIcon,
  },
  {
    value: SoundFile.ROOSTER,
    icon: BirdIcon,
  },
];

const AGENT_PRIORITY: BaseCodingAgent[] = [
  BaseCodingAgent.CLAUDE_CODE,
  BaseCodingAgent.CODEX,
];

const SOCIAL_LINKS = [
  {
    label: (t: (key: string) => string) => t('onboarding.github'),
    href: LVK_GITHUB_URL,
    icon: GithubLogoIcon,
  },
  {
    label: (t: (key: string) => string) => t('onboarding.docs'),
    href: 'https://www.vibekanban.com/docs',
    icon: BookOpenIcon,
  },
];

const REMOTE_ONBOARDING_EVENTS = {
  STAGE_VIEWED: 'remote_onboarding_ui_stage_viewed',
  STAGE_SUBMITTED: 'remote_onboarding_ui_stage_submitted',
  STAGE_COMPLETED: 'remote_onboarding_ui_stage_completed',
  STAGE_FAILED: 'remote_onboarding_ui_stage_failed',
} as const;

function randomDefaultSoundFile(): SoundFile {
  const randomIndex = Math.floor(Math.random() * SOUND_OPTIONS.length);
  return SOUND_OPTIONS[randomIndex]?.value ?? SoundFile.COW_MOOING;
}

function resolveTheme(theme: ThemeMode): 'light' | 'dark' {
  if (theme === ThemeMode.SYSTEM) {
    return window.matchMedia('(prefers-color-scheme: dark)').matches
      ? 'dark'
      : 'light';
  }
  return theme === ThemeMode.DARK ? 'dark' : 'light';
}

export function LandingPage() {
  const { t } = useTranslation('common');
  const appNavigation = useAppNavigation();
  const { theme } = useTheme();
  const { config, profiles, updateAndSaveConfig, loading } = useUserSystem();
  const posthog = usePostHog();

  const [initialized, setInitialized] = useState(false);
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState(false);
  const [selectedAgent, setSelectedAgent] = useState<BaseCodingAgent>(
    BaseCodingAgent.CLAUDE_CODE
  );
  const [editorConfig, setEditorConfig] = useState<EditorConfig>({
    entries: [],
  });
  const [soundEnabled, setSoundEnabled] = useState(true);
  const [soundFile, setSoundFile] = useState<SoundFile>(randomDefaultSoundFile);
  const hasTrackedStageViewRef = useRef(false);
  const hasRedirectedToRootRef = useRef(false);

  const trackRemoteOnboardingEvent = useCallback(
    (eventName: string, properties: Record<string, unknown> = {}) => {
      posthog?.capture(eventName, {
        ...properties,
        flow: 'remote_onboarding_ui',
        source: 'frontend',
      });
    },
    [posthog]
  );

  const logoSrc =
    resolveTheme(theme) === 'dark' ? '/lvk-logo-dark.svg' : '/lvk-logo.svg';

  useEffect(() => {
    if (!config || initialized) return;

    setSelectedAgent(config.executor_profile.executor);
    setEditorConfig(config.editor);
    setInitialized(true);
  }, [config, initialized]);

  useEffect(() => {
    if (!config || !initialized || hasTrackedStageViewRef.current) return;

    trackRemoteOnboardingEvent(REMOTE_ONBOARDING_EVENTS.STAGE_VIEWED, {
      stage: 'landing',
    });
    hasTrackedStageViewRef.current = true;
  }, [config, initialized, trackRemoteOnboardingEvent]);

  useEffect(() => {
    if (!config?.onboarding_acknowledged || hasRedirectedToRootRef.current) {
      return;
    }

    hasRedirectedToRootRef.current = true;
    appNavigation.goToRoot({ replace: true });
  }, [appNavigation, config?.onboarding_acknowledged]);

  const executorOptions = useMemo(() => {
    const compareAgents = (a: BaseCodingAgent, b: BaseCodingAgent) => {
      const priorityA = AGENT_PRIORITY.indexOf(a);
      const priorityB = AGENT_PRIORITY.indexOf(b);
      const hasPriorityA = priorityA !== -1;
      const hasPriorityB = priorityB !== -1;

      if (hasPriorityA && hasPriorityB) {
        return priorityA - priorityB;
      }
      if (hasPriorityA) return -1;
      if (hasPriorityB) return 1;

      return getAgentName(a).localeCompare(getAgentName(b));
    };

    if (profiles) {
      return filterVisibleAgents({
        agents: (Object.keys(profiles) as BaseCodingAgent[]).sort(
          compareAgents
        ),
        hiddenAgents: config?.hidden_agents,
        preserveAgents: [selectedAgent],
      });
    }
    return filterVisibleAgents({
      agents: [...Object.values(BaseCodingAgent)].sort(compareAgents),
      hiddenAgents: config?.hidden_agents,
      preserveAgents: [selectedAgent],
    });
  }, [config?.hidden_agents, profiles, selectedAgent]);

  const previewSound = async (value: SoundFile) => {
    try {
      await playSound(`/api/sounds/${value}`);
    } catch (err) {
      console.error('Failed to play sound:', err);
    }
  };

  const openExternalLink = (url: string) => {
    window.open(url, '_blank', 'noopener,noreferrer');
  };

  const handleSoundSelect = (value: SoundFile) => {
    setSoundEnabled(true);
    setSoundFile(value);
    void previewSound(value);
  };

  const canContinue = !saving;

  const handleContinue = async () => {
    if (!config || !canContinue) return;

    trackRemoteOnboardingEvent(REMOTE_ONBOARDING_EVENTS.STAGE_SUBMITTED, {
      stage: 'landing',
      method: 'continue',
      selected_agent: selectedAgent,
      sound_enabled: soundEnabled,
      sound_file: soundEnabled ? soundFile : null,
    });

    setSaveError(false);
    setSaving(true);
    const success = await updateAndSaveConfig(
      {
        onboarding_acknowledged: true,
        disclaimer_acknowledged: true,
        executor_profile: {
          executor: selectedAgent,
          variant: null,
        },
        editor: editorConfig,
        notifications: {
          ...config.notifications,
          sound_enabled: soundEnabled,
          sound_file: soundFile,
        },
      },
      () => setSaveError(true)
    );
    setSaving(false);

    if (success) {
      trackRemoteOnboardingEvent(REMOTE_ONBOARDING_EVENTS.STAGE_COMPLETED, {
        stage: 'landing',
        destination: '/',
      });
      appNavigation.goToRoot({
        replace: true,
      });
      return;
    }

    trackRemoteOnboardingEvent(REMOTE_ONBOARDING_EVENTS.STAGE_FAILED, {
      stage: 'landing',
      reason: 'config_save_failed',
    });
  };

  if (loading || !config || !initialized) {
    return (
      <div className="h-screen bg-primary flex items-center justify-center">
        <p className="text-low">{t('onboarding.loading')}</p>
      </div>
    );
  }

  if (config.onboarding_acknowledged) {
    return null;
  }

  return (
    <div className="h-screen bg-primary flex items-center justify-center p-double">
      <div className="flex max-h-full w-full max-w-5xl flex-col rounded-sm border border-border bg-secondary">
        {/* Header */}
        <header className="shrink-0 space-y-base p-double pb-base">
          <div className="flex flex-wrap items-center justify-between gap-base">
            <img
              src={logoSrc}
              alt="Lucky Vibe Kanban"
              className="h-8 w-auto logo"
            />
            <div className="flex flex-wrap items-center gap-2">
              {SOCIAL_LINKS.map((link) => (
                <PrimaryButton
                  key={link.href}
                  value={link.label(t)}
                  variant="tertiary"
                  actionIcon={link.icon}
                  onClick={() => openExternalLink(link.href)}
                />
              ))}
            </div>
          </div>
          <div className="rounded-sm border border-brand bg-brand/20 p-base">
            <div className="flex items-start gap-base">
              <WarningIcon
                className="size-icon-sm text-brand shrink-0 mt-[2px]"
                weight="fill"
              />
              <p className="text-sm text-normal">
                {t('onboarding.safetyBefore')}{' '}
                <code>--dangerously-skip-permissions</code> /{' '}
                <code>--yolo</code> {t('onboarding.safetyAfter')}{' '}
                <a
                  href="https://www.vibekanban.com/docs/getting-started#safety-notice"
                  target="_blank"
                  rel="noopener noreferrer"
                  className="text-brand hover:underline"
                >
                  {t('onboarding.learnMore')}
                </a>
                {t('onboarding.safetyEnd')}
              </p>
            </div>
          </div>
        </header>

        {/* 3-column grid */}
        <div className="min-h-0 flex-1 overflow-y-auto px-double pb-double">
          <div className="grid grid-cols-1 md:grid-cols-3 gap-double">
            {/* Column 1: Coding Agent */}
            <section className="space-y-half">
              <h2 className="text-sm font-medium text-high">
                {t('onboarding.codingAgent')}
              </h2>
              <div className="grid gap-1.5">
                {executorOptions.map((agent) => {
                  const selected = selectedAgent === agent;

                  return (
                    <button
                      key={agent}
                      type="button"
                      onClick={() => setSelectedAgent(agent)}
                      aria-label={getAgentName(agent)}
                      aria-pressed={selected}
                      className={cn(
                        'flex items-center gap-base rounded-sm border px-base py-half text-left',
                        selected
                          ? 'border-brand bg-brand/10'
                          : 'border-border bg-panel hover:bg-primary'
                      )}
                    >
                      <AgentIcon
                        agent={agent}
                        className="size-icon-xl shrink-0"
                      />
                      <span className="text-sm text-normal flex-1 truncate">
                        {getAgentName(agent)}
                      </span>
                      {selected && (
                        <CheckIcon
                          className="size-icon-xs text-brand shrink-0"
                          weight="bold"
                        />
                      )}
                    </button>
                  );
                })}
              </div>
            </section>

            <section className="space-y-half">
              <ExternalEditorSettings
                value={editorConfig}
                onChange={setEditorConfig}
              />
              {saveError && (
                <p role="alert" className="text-error">
                  {t('onboarding.saveUnconfirmed')}
                </p>
              )}
            </section>

            {/* Column 3: Notification Sound */}
            <section className="space-y-half">
              <h2 className="text-sm font-medium text-high">
                {t('onboarding.notificationSound')}
              </h2>
              <div className="grid gap-1.5">
                {SOUND_OPTIONS.map((option) => {
                  const Icon = option.icon;
                  const selected = soundEnabled && soundFile === option.value;

                  return (
                    <button
                      key={option.value}
                      type="button"
                      onClick={() => handleSoundSelect(option.value)}
                      aria-pressed={selected}
                      className={cn(
                        'flex items-center gap-base rounded-sm border px-base py-half text-left',
                        selected
                          ? 'border-brand bg-brand/10'
                          : 'border-border bg-panel hover:bg-primary'
                      )}
                    >
                      <Icon
                        className={cn(
                          'size-icon-sm shrink-0',
                          selected ? 'text-brand' : 'text-normal'
                        )}
                        weight={selected ? 'fill' : 'bold'}
                      />
                      <span className="text-sm text-normal flex-1 truncate">
                        {notificationSoundLabel(option.value, t)}
                      </span>
                      {selected && (
                        <CheckIcon
                          className="size-icon-xs text-brand shrink-0"
                          weight="bold"
                        />
                      )}
                    </button>
                  );
                })}
                <button
                  type="button"
                  onClick={() => setSoundEnabled(false)}
                  aria-pressed={!soundEnabled}
                  className={cn(
                    'flex items-center gap-base rounded-sm border px-base py-half text-left',
                    !soundEnabled
                      ? 'border-brand bg-brand/10'
                      : 'border-border bg-panel hover:bg-primary'
                  )}
                >
                  <SpeakerXIcon
                    className={cn(
                      'size-icon-sm shrink-0',
                      !soundEnabled ? 'text-brand' : 'text-normal'
                    )}
                    weight={!soundEnabled ? 'fill' : 'bold'}
                  />
                  <span className="text-sm text-normal flex-1">
                    {t('onboarding.noSound')}
                  </span>
                  {!soundEnabled && (
                    <CheckIcon
                      className="size-icon-xs text-brand shrink-0"
                      weight="bold"
                    />
                  )}
                </button>
              </div>
            </section>
          </div>
        </div>

        {/* Footer */}
        <div className="shrink-0 border-t border-border p-double pt-base flex flex-wrap items-center justify-between gap-base">
          <p className="text-xs text-low">
            {t('onboarding.termsBefore')}{' '}
            <a
              href="https://www.vibekanban.com/terms"
              target="_blank"
              rel="noopener noreferrer"
              className="text-brand hover:underline"
            >
              {t('onboarding.terms')}
            </a>{' '}
            {t('onboarding.termsAnd')}{' '}
            <a
              href="https://www.vibekanban.com/privacy"
              target="_blank"
              rel="noopener noreferrer"
              className="text-brand hover:underline"
            >
              {t('onboarding.privacy')}
            </a>
            {t('onboarding.termsAfter')}
          </p>
          <PrimaryButton
            value={saving ? t('onboarding.saving') : t('onboarding.continue')}
            onClick={handleContinue}
            disabled={!canContinue}
          />
        </div>
      </div>
    </div>
  );
}
