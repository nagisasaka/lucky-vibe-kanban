import type { ReactNode } from 'react';
import { cn } from '@/shared/lib/utils';
import { useTranslation } from 'react-i18next';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
  DropdownMenuTriggerButton,
} from '@vibe/ui/components/Dropdown';
import { PrimaryButton } from '@vibe/ui/components/PrimaryButton';
import type { Icon } from '@phosphor-icons/react';

// ============================================================================
// Two-Column Picker Components
// ============================================================================

// TwoColumnPicker - Container for the two-column layout
interface TwoColumnPickerProps {
  children: ReactNode;
}

export function TwoColumnPicker({ children }: TwoColumnPickerProps) {
  return (
    <div className="flex flex-col md:flex-row border border-border rounded-sm overflow-hidden">
      {children}
    </div>
  );
}

// TwoColumnPickerColumn - A single column within the picker
interface TwoColumnPickerColumnProps {
  label: string;
  headerAction?: ReactNode;
  isFirst?: boolean;
  children: ReactNode;
}

export function TwoColumnPickerColumn({
  label,
  headerAction,
  isFirst,
  children,
}: TwoColumnPickerColumnProps) {
  return (
    <div
      className={cn(
        'min-w-0 flex-1',
        isFirst && 'border-b md:border-b-0 md:border-r border-border'
      )}
    >
      <div className="h-9 px-base border-b border-border bg-secondary/50 flex items-center justify-between">
        <span className="block text-sm font-medium leading-snug text-normal">
          {label}
        </span>
        {headerAction}
      </div>
      <div className="max-h-32 md:h-32 overflow-y-auto bg-panel">
        {children}
      </div>
    </div>
  );
}

// TwoColumnPickerItem - A selectable row within a column
interface TwoColumnPickerItemProps {
  selected?: boolean;
  onClick?: () => void;
  leading?: ReactNode;
  trailing?: ReactNode;
  children: ReactNode;
}

export function TwoColumnPickerItem({
  selected,
  onClick,
  leading,
  trailing,
  children,
}: TwoColumnPickerItemProps) {
  return (
    <div
      className={cn(
        'group flex items-center gap-half px-base py-half cursor-pointer transition-colors',
        'hover:bg-secondary',
        selected && 'bg-brand/10 text-brand'
      )}
      onClick={onClick}
    >
      {leading}
      <span
        className={cn(
          'text-sm truncate flex-1',
          selected ? 'text-brand font-medium' : 'text-normal'
        )}
      >
        {children}
      </span>
      {trailing}
    </div>
  );
}

// TwoColumnPickerBadge - A small badge/tag for items
interface TwoColumnPickerBadgeProps {
  variant?: 'default' | 'brand';
  children: ReactNode;
}

export function TwoColumnPickerBadge({
  variant = 'default',
  children,
}: TwoColumnPickerBadgeProps) {
  return (
    <span
      className={cn(
        'text-xs px-half rounded font-medium shrink-0',
        variant === 'brand' ? 'bg-brand/15 text-brand' : 'bg-secondary text-low'
      )}
    >
      {children}
    </span>
  );
}

// TwoColumnPickerEmpty - Empty state message for a column
interface TwoColumnPickerEmptyProps {
  children: ReactNode;
}

export function TwoColumnPickerEmpty({ children }: TwoColumnPickerEmptyProps) {
  return (
    <div className="px-base py-plusfifty text-sm text-low text-center">
      {children}
    </div>
  );
}

// ============================================================================
// Settings Card Components
// ============================================================================

// SettingsCard - A card container for a settings subsection
export function SettingsCard({
  title,
  description,
  children,
  headerAction,
}: {
  title: string;
  description?: ReactNode;
  children: React.ReactNode;
  headerAction?: React.ReactNode;
}) {
  return (
    <div className="min-w-0 rounded-sm border border-border bg-primary">
      <div className="flex flex-wrap items-start justify-between gap-3 border-b border-border px-4 py-4 md:px-5">
        <div className="min-w-0 flex-1">
          <h3 className="text-base font-semibold leading-snug text-high">
            {title}
          </h3>
          {description && (
            <p className="mt-2 max-w-prose text-sm leading-relaxed text-low">
              {description}
            </p>
          )}
        </div>
        {headerAction && <div className="shrink-0 ml-2">{headerAction}</div>}
      </div>
      <div className="space-y-5 px-4 py-5 md:px-5">{children}</div>
    </div>
  );
}

// SettingsField - A labeled field wrapper
export function SettingsField({
  label,
  htmlFor,
  description,
  error,
  children,
}: {
  label: string;
  htmlFor?: string;
  description?: React.ReactNode;
  error?: string | null;
  children: React.ReactNode;
}) {
  return (
    <div className="min-w-0 space-y-2">
      {label && (
        <label
          htmlFor={htmlFor}
          className="block text-sm font-medium leading-snug text-normal"
        >
          {label}
        </label>
      )}
      {children}
      {error && <p className="text-sm text-error">{error}</p>}
      {description && !error && (
        <p className="max-w-prose text-sm leading-relaxed text-low">
          {description}
        </p>
      )}
    </div>
  );
}

// SettingsCheckbox - A checkbox with label and optional description
export function SettingsCheckbox({
  id,
  label,
  description,
  checked,
  onChange,
  disabled,
}: {
  id: string;
  label: string;
  description?: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
}) {
  return (
    <div className="flex items-start gap-3">
      <input
        type="checkbox"
        id={id}
        checked={checked}
        onChange={(e) => onChange(e.target.checked)}
        disabled={disabled}
        className={cn(
          'mt-0.5 h-4 w-4 shrink-0 rounded border-border bg-secondary text-brand focus:ring-brand focus:ring-offset-0',
          disabled && 'opacity-50 cursor-not-allowed'
        )}
      />
      <div className="space-y-0.5">
        <label
          htmlFor={id}
          className={cn(
            'text-sm font-medium text-normal cursor-pointer',
            disabled && 'opacity-50 cursor-not-allowed'
          )}
        >
          {label}
        </label>
        {description && (
          <p className="max-w-prose text-sm leading-relaxed text-low">
            {description}
          </p>
        )}
      </div>
    </div>
  );
}

// SettingsSelect - A dropdown select component
export function SettingsSelect<T extends string>({
  id,
  value,
  options,
  actions,
  onChange,
  placeholder,
  disabled,
  className,
}: {
  id?: string;
  value: T | undefined;
  options: { value: T; label: string }[];
  actions?: { label: string; icon?: Icon; onClick: () => void }[];
  onChange: (value: T) => void;
  placeholder?: string;
  disabled?: boolean;
  className?: string;
}) {
  const selectedOption = options.find((opt) => opt.value === value);
  const hasMenuItems = options.length > 0 || (actions?.length ?? 0) > 0;
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <DropdownMenuTriggerButton
          id={id}
          label={selectedOption?.label || placeholder}
          className={cn('min-h-10 w-full justify-between text-left', className)}
          disabled={disabled || !hasMenuItems}
        />
      </DropdownMenuTrigger>
      <DropdownMenuContent className="w-[var(--radix-dropdown-menu-trigger-width)]">
        {options.map((option) => (
          <DropdownMenuItem
            key={option.value}
            onClick={() => onChange(option.value)}
          >
            {option.label}
          </DropdownMenuItem>
        ))}
        {actions?.map((action) => (
          <DropdownMenuItem
            key={action.label}
            icon={action.icon}
            onClick={action.onClick}
          >
            {action.label}
          </DropdownMenuItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

// SettingsInput - A text input field
export function SettingsInput({
  id,
  value,
  onChange,
  placeholder,
  error,
  disabled,
}: {
  id?: string;
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  error?: boolean;
  disabled?: boolean;
}) {
  return (
    <input
      id={id}
      type="text"
      value={value}
      onChange={(e) => onChange(e.target.value)}
      placeholder={placeholder}
      disabled={disabled}
      aria-invalid={error || undefined}
      className={cn(
        'min-h-10 w-full bg-secondary border rounded-sm px-3 py-2 text-sm text-high',
        'placeholder:text-low placeholder:opacity-80 focus:outline-none focus:ring-1 focus:ring-brand',
        error ? 'border-error' : 'border-border',
        disabled && 'opacity-50 cursor-not-allowed'
      )}
    />
  );
}

// SettingsTextarea - A multi-line text input
export function SettingsTextarea({
  value,
  onChange,
  placeholder,
  disabled,
  rows = 4,
  monospace = false,
}: {
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  disabled?: boolean;
  rows?: number;
  monospace?: boolean;
}) {
  return (
    <textarea
      value={value}
      onChange={(e) => onChange(e.target.value)}
      placeholder={placeholder}
      disabled={disabled}
      rows={rows}
      className={cn(
        'w-full bg-secondary border border-border rounded-sm px-3 py-2 text-sm leading-relaxed text-high',
        'placeholder:text-low placeholder:opacity-80 focus:outline-none focus:ring-1 focus:ring-brand',
        'resize-y',
        monospace && 'font-ibm-plex-mono',
        disabled && 'opacity-50 cursor-not-allowed'
      )}
    />
  );
}

// SettingsSaveBar - A sticky save bar for unsaved changes
export function SettingsSaveBar({
  show,
  saving,
  saveDisabled,
  onSave,
  onDiscard,
}: {
  show: boolean;
  saving: boolean;
  saveDisabled?: boolean;
  onSave: () => void;
  onDiscard?: () => void;
}) {
  const { t } = useTranslation(['settings', 'common']);

  if (!show) {
    return <div />;
  }

  return (
    <div className="sticky bottom-0 z-20 bg-primary border-t border-border py-4 -mx-4 px-4 md:-mx-6 md:px-6 -mb-6">
      <div
        className={cn(
          'flex flex-wrap gap-2 items-center',
          onDiscard ? 'justify-between' : 'justify-end'
        )}
      >
        {onDiscard && (
          <span className="max-w-prose text-sm leading-relaxed text-low">
            {t('settings.common.unsavedChanges')}
          </span>
        )}
        <div className="ml-auto flex flex-wrap gap-2">
          {onDiscard && (
            <PrimaryButton
              variant="tertiary"
              value={t('common:buttons.discard')}
              onClick={onDiscard}
              disabled={saving}
            />
          )}
          <PrimaryButton
            value={t('common:buttons.save')}
            onClick={onSave}
            disabled={saving || saveDisabled}
            actionIcon={saving ? 'spinner' : undefined}
          />
        </div>
      </div>
    </div>
  );
}
