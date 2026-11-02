import { useState, useEffect, useCallback, useMemo, useRef } from 'react';
import { createPortal } from 'react-dom';
import { useTranslation } from 'react-i18next';
import { CaretLeftIcon, XIcon } from '@phosphor-icons/react';
import { create, useModal } from '@ebay/nice-modal-react';
import { defineModal } from '@/shared/lib/modals';

import { cn } from '@/shared/lib/utils';
import { SettingsSection } from './settings/SettingsSection';

import type {
  SettingsSectionType,
  SettingsSectionInitialState,
} from './settings/SettingsSection';
import {
  SETTINGS_SECTION_DEFINITIONS,
  isHostSpecificSettingsSection,
} from './settings/settingsRegistry';
import {
  SettingsDirtyProvider,
  useSettingsDirty,
} from './settings/SettingsDirtyContext';
import {
  SettingsHostProvider,
  useSettingsHost,
} from './settings/SettingsHostContext';
import { SettingsMachineUserSystemProvider } from './settings/SettingsMachineUserSystemProvider';
import { useSettingsAccess } from './settings/SettingsAccessContext';
import { ConfirmDialog } from '@vibe/ui/components/ConfirmDialog';

export interface SettingsDialogProps {
  initialSection?: SettingsSectionType;
  initialState?: SettingsSectionInitialState[SettingsSectionType];
  initialHostId?: string | 'local';
}

interface SettingsDialogContentProps {
  initialSection?: SettingsSectionType;
  initialState?: SettingsSectionInitialState[SettingsSectionType];
  onClose: () => void;
}

function SettingsDialogNavigation({
  activeSection,
  onSectionSelect,
}: {
  activeSection: SettingsSectionType;
  onSectionSelect: (sectionId: SettingsSectionType) => void;
}) {
  const { t } = useTranslation('settings');
  const access = useSettingsAccess();
  const groups = {
    experience: ['general', 'notifications'],
    ai: ['execution', 'agents', 'agent-tools'],
    projects: ['remote-projects', 'repos'],
    connections: [
      'mcp-integration',
      'mcp',
      ...(access?.management ? ['access'] : []),
    ],
    operations: ['resources'],
    ...(access?.account ? { account: ['account'] } : {}),
  };
  return (
    <nav className="flex-1 p-2 flex flex-col gap-1 overflow-y-auto">
      {Object.entries(groups).map(([group, ids]) => (
        <div key={group} className="space-y-1 mb-3">
          <h3 className="px-3 pt-2 text-xs font-medium text-low">
            {t(`settings.groups.${group as keyof typeof groups}`)}
          </h3>
          {ids
            .map(
              (id) =>
                SETTINGS_SECTION_DEFINITIONS.find(
                  (section) => section.id === id
                )!
            )
            .map((section) => {
              const Icon = section.icon;
              return (
                <button
                  key={section.id}
                  type="button"
                  aria-current={
                    activeSection === section.id ? 'page' : undefined
                  }
                  onClick={() => onSectionSelect(section.id)}
                  className={cn(
                    'w-full flex items-center gap-3 text-left px-3 py-3 md:py-2 rounded-sm text-sm transition-colors focus-visible:ring-2 focus-visible:ring-brand',
                    activeSection === section.id
                      ? 'bg-brand/10 text-brand font-medium'
                      : 'text-normal hover:bg-primary/10'
                  )}
                >
                  <Icon className="size-icon-sm shrink-0" weight="bold" />
                  <span>{t(`settings.layout.nav.${section.id}`)}</span>
                </button>
              );
            })}
        </div>
      ))}
    </nav>
  );
}

function SettingsDialogContent({
  initialSection,
  initialState,
  onClose,
}: SettingsDialogContentProps) {
  const { t } = useTranslation('settings');
  const { isDirty, getIsDirty } = useSettingsDirty();
  const access = useSettingsAccess();
  const dialogRef = useRef<HTMLDivElement>(null);
  const contentRef = useRef<HTMLDivElement>(null);
  const {
    availableHosts,
    hostsResolved,
    selectedHost,
    selectedHostId,
    canEdit,
  } = useSettingsHost();

  const resolvedInitialSection = useMemo<SettingsSectionType>(() => {
    if (
      initialSection &&
      (initialSection !== 'access' || access?.management) &&
      (initialSection !== 'account' || access?.account) &&
      SETTINGS_SECTION_DEFINITIONS.some(
        (section) => section.id === initialSection
      )
    ) {
      return initialSection;
    }

    if (hostsResolved && availableHosts.length === 0 && !selectedHostId) {
      return 'general';
    }

    return 'general';
  }, [
    access,
    availableHosts.length,
    hostsResolved,
    initialSection,
    selectedHostId,
  ]);

  const [activeSection, setActiveSection] = useState<SettingsSectionType>(
    resolvedInitialSection
  );
  // On mobile, null means show the nav menu, a section means show that section
  const [mobileShowContent, setMobileShowContent] = useState<boolean>(
    initialSection === resolvedInitialSection
  );
  const isConfirmingRef = useRef(false);

  const handleCloseWithConfirmation = useCallback(async () => {
    if (isConfirmingRef.current) return;

    if (getIsDirty()) {
      isConfirmingRef.current = true;
      try {
        const result = await ConfirmDialog.show({
          title: t('settings.unsavedChanges.title'),
          message: t('settings.unsavedChanges.message'),
          confirmText: t('settings.unsavedChanges.discard'),
          cancelText: t('settings.unsavedChanges.cancel'),
          variant: 'destructive',
        });
        if (result === 'confirmed') {
          onClose();
        }
      } finally {
        isConfirmingRef.current = false;
      }
    } else {
      onClose();
    }
  }, [getIsDirty, onClose, t]);

  const handleSectionSelect = async (sectionId: SettingsSectionType) => {
    if (sectionId !== activeSection && getIsDirty()) {
      if (isConfirmingRef.current) return;
      isConfirmingRef.current = true;
      try {
        const result = await ConfirmDialog.show({
          title: t('settings.unsavedChanges.title'),
          message: t('settings.unsavedChanges.message'),
          confirmText: t('settings.unsavedChanges.discard'),
          cancelText: t('settings.unsavedChanges.cancel'),
          variant: 'destructive',
        });
        if (result !== 'confirmed') return;
      } finally {
        isConfirmingRef.current = false;
      }
    }
    setActiveSection(sectionId);
    setMobileShowContent(true);
  };

  useEffect(() => {
    contentRef.current?.scrollTo(0, 0);
    if (mobileShowContent) contentRef.current?.focus();
  }, [activeSection, mobileShowContent]);

  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    const overflow = document.body.style.overflow;
    document.body.style.overflow = 'hidden';
    const appRoot = document.getElementById('root');
    const wasInert = appRoot?.inert ?? false;
    if (appRoot) appRoot.inert = true;
    dialogRef.current?.focus();
    return () => {
      document.body.style.overflow = overflow;
      if (appRoot) appRoot.inert = wasInert;
      previous?.focus();
    };
  }, []);

  useEffect(() => {
    const protect = (event: BeforeUnloadEvent) => {
      if (getIsDirty()) event.preventDefault();
    };
    window.addEventListener('beforeunload', protect);
    return () => window.removeEventListener('beforeunload', protect);
  }, [getIsDirty]);

  useEffect(() => {
    if (
      hostsResolved &&
      isHostSpecificSettingsSection(activeSection) &&
      availableHosts.length === 0 &&
      !selectedHostId
    ) {
      setActiveSection('general');
    }
  }, [activeSection, availableHosts.length, hostsResolved, selectedHostId]);

  const handleMobileBack = () => {
    setMobileShowContent(false);
  };

  // Handle ESC key
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Tab' && !isConfirmingRef.current) {
        const elements = Array.from(
          dialogRef.current?.querySelectorAll<HTMLElement>(
            'button:not(:disabled), a[href], input:not(:disabled), select:not(:disabled), textarea:not(:disabled), summary, [tabindex="0"]'
          ) ?? []
        ).filter((el) => el.getClientRects().length > 0);
        const first = elements[0],
          last = elements.at(-1);
        if (
          e.shiftKey &&
          (document.activeElement === first ||
            document.activeElement === dialogRef.current)
        ) {
          e.preventDefault();
          last?.focus();
        } else if (!e.shiftKey && document.activeElement === last) {
          e.preventDefault();
          first?.focus();
        }
      }
      if (
        e.key === 'Escape' &&
        !isConfirmingRef.current &&
        dialogRef.current?.contains(document.activeElement)
      ) {
        e.preventDefault();
        e.stopImmediatePropagation();
        void handleCloseWithConfirmation();
      }
    };
    window.addEventListener('keydown', handleKeyDown, true);
    return () => window.removeEventListener('keydown', handleKeyDown, true);
  }, [handleCloseWithConfirmation]);

  return (
    <>
      {/* Overlay */}
      <div
        className="fixed inset-0 z-[9998] bg-black/50 animate-in fade-in-0 duration-200"
        onClick={handleCloseWithConfirmation}
      />
      {/* Dialog wrapper - handles positioning */}
      <div
        className={cn(
          'fixed z-[9999]',
          // Mobile: full screen
          'inset-0',
          // Desktop: centered with fixed size
          'md:inset-auto md:left-1/2 md:top-1/2 md:-translate-x-1/2 md:-translate-y-1/2'
        )}
      >
        {/* Dialog content - handles animation */}
        <div
          ref={dialogRef}
          role="dialog"
          aria-modal="true"
          aria-label={t('settings.layout.nav.title')}
          tabIndex={-1}
          className={cn(
            'h-full w-full flex overflow-hidden',
            'bg-primary shadow-lg',
            'animate-in fade-in-0 slide-in-from-bottom-4 duration-200',
            // Mobile: full screen, no rounded corners
            'rounded-none border-0',
            // Desktop: fixed size with rounded corners
            'md:w-[min(1120px,94vw)] md:h-[min(820px,92dvh)] md:rounded-sm md:border md:border-border/50'
          )}
        >
          {/* Sidebar - hidden on mobile when showing content */}
          <div
            className={cn(
              'bg-secondary border-r border-border flex flex-col',
              // Mobile: full width, hidden when showing content
              'w-full',
              mobileShowContent && 'hidden',
              // Desktop: fixed width sidebar, always visible
              'md:w-64 md:flex shrink-0 min-h-0'
            )}
          >
            {/* Header */}
            <div className="p-4 border-b border-border flex items-center justify-between">
              <h2 className="text-lg font-semibold text-high">
                {t('settings.layout.nav.title')}
              </h2>
              {/* Close button - mobile only */}
              <button
                onClick={handleCloseWithConfirmation}
                aria-label={t('buttons.close', { ns: 'common' })}
                className="p-2 rounded-sm hover:bg-secondary text-low hover:text-normal md:hidden"
              >
                <XIcon className="size-icon-sm" weight="bold" />
              </button>
            </div>
            <SettingsDialogNavigation
              activeSection={activeSection}
              onSectionSelect={handleSectionSelect}
            />
          </div>
          {/* Content - hidden on mobile when showing nav */}
          <div
            className={cn(
              'min-w-0 flex-1 flex flex-col relative overflow-hidden',
              // Mobile: full width, hidden when showing nav
              !mobileShowContent && 'hidden',
              // Desktop: always visible
              'md:flex'
            )}
          >
            {/* Mobile header with back button */}
            <div className="flex items-center gap-2 p-3 border-b border-border md:hidden">
              <button
                onClick={handleMobileBack}
                aria-label={t('settings.layout.nav.title')}
                className="p-2 rounded-sm hover:bg-secondary text-low hover:text-normal"
              >
                <CaretLeftIcon className="size-icon-sm" weight="bold" />
              </button>
              <span className="text-sm font-medium text-high">
                {t(`settings.layout.nav.${activeSection}`)}
              </span>
              <button
                onClick={handleCloseWithConfirmation}
                aria-label={t('buttons.close', { ns: 'common' })}
                className="ml-auto p-2 rounded-sm hover:bg-secondary text-low hover:text-normal"
              >
                <XIcon className="size-icon-sm" weight="bold" />
              </button>
            </div>
            {/* Section content */}
            <div
              ref={contentRef}
              tabIndex={-1}
              className="min-h-0 flex-1 overflow-y-auto focus:outline-none"
            >
              <div className="px-4 py-3 border-b border-border bg-secondary text-xs text-low space-y-1">
                <p>{t(`settings.scope.${activeSection}`)}</p>
                {isDirty && (
                  <p role="status" className="text-brand">
                    {t('settings.common.unsavedChanges')}
                  </p>
                )}
              </div>
              {activeSection === 'access' || activeSection === 'account' ? (
                <>
                  <div className="hidden md:flex items-center justify-between p-4 border-b border-border">
                    <h2 className="text-lg text-high font-semibold">
                      {t(`settings.layout.nav.${activeSection}`)}
                    </h2>
                    <button
                      onClick={handleCloseWithConfirmation}
                      aria-label={t('buttons.close', { ns: 'common' })}
                      className="p-2 focus-visible:ring-2 focus-visible:ring-brand"
                    >
                      <XIcon className="size-icon-sm" />
                    </button>
                  </div>
                  {activeSection === 'access'
                    ? access?.management
                    : access?.account}
                </>
              ) : isHostSpecificSettingsSection(activeSection) ? (
                selectedHost ? (
                  <SettingsMachineUserSystemProvider key={selectedHostId}>
                    {!canEdit && (
                      <p role="status" className="p-4 text-warning text-sm">
                        {t('settings.hostPicker.readOnlyHint')}
                      </p>
                    )}
                    <SettingsSection
                      type={activeSection}
                      onClose={handleCloseWithConfirmation}
                      initialState={
                        activeSection === resolvedInitialSection
                          ? initialState
                          : undefined
                      }
                    />
                  </SettingsMachineUserSystemProvider>
                ) : !hostsResolved ? (
                  <div className="px-6 py-8 text-sm text-low">
                    {t('settings.general.loading')}
                  </div>
                ) : selectedHostId ? (
                  <div role="alert" className="px-6 py-8 text-sm text-warning">
                    {t('settings.hostPicker.unavailableHint', {
                      host: selectedHostId,
                    })}
                  </div>
                ) : availableHosts.length > 0 ? (
                  <div className="px-6 py-8 text-sm text-low">
                    {t('settings.hostPicker.selectMachineHint')}
                  </div>
                ) : (
                  <div className="px-6 py-8 text-sm text-low">
                    {t('settings.hostPicker.noHostAvailable')}
                  </div>
                )
              ) : (
                <SettingsSection
                  type={activeSection}
                  onClose={handleCloseWithConfirmation}
                  initialState={
                    activeSection === resolvedInitialSection
                      ? initialState
                      : undefined
                  }
                />
              )}
            </div>
          </div>
        </div>
      </div>
    </>
  );
}

const SettingsDialogImpl = create<SettingsDialogProps>(
  ({ initialSection, initialState, initialHostId }) => {
    const modal = useModal();
    const handleClose = useCallback(() => {
      modal.hide();
      modal.resolve();
      modal.remove();
    }, [modal]);

    return createPortal(
      <SettingsDirtyProvider>
        <SettingsHostProvider initialHostId={initialHostId}>
          <SettingsDialogContent
            initialSection={initialSection}
            initialState={initialState}
            onClose={handleClose}
          />
        </SettingsHostProvider>
      </SettingsDirtyProvider>,
      document.body
    );
  }
);

export const SettingsDialog = defineModal<SettingsDialogProps | void, void>(
  SettingsDialogImpl
);
