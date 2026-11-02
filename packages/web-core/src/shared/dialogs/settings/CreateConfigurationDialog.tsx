import { useTranslation } from 'react-i18next';
import { useState, useEffect } from 'react';
import { Button } from '@vibe/ui/components/Button';
import { Input } from '@vibe/ui/components/Input';
import { Label } from '@vibe/ui/components/Label';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@vibe/ui/components/KeyboardDialog';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@vibe/ui/components/Select';
import { Alert, AlertDescription } from '@vibe/ui/components/Alert';
import { create, useModal } from '@ebay/nice-modal-react';
import { defineModal } from '@/shared/lib/modals';

type NameError = 'empty' | 'tooLong' | 'invalid' | 'exists';

export interface CreateConfigurationDialogProps {
  executorType: string;
  existingConfigs: string[];
}

export type CreateConfigurationResult = {
  action: 'created' | 'canceled';
  configName?: string;
  cloneFrom?: string | null;
};

const CreateConfigurationDialogImpl = create<CreateConfigurationDialogProps>(
  ({ executorType, existingConfigs }) => {
    const { t } = useTranslation('settings');
    const modal = useModal();
    const [configName, setConfigName] = useState('');
    const [cloneFrom, setCloneFrom] = useState<string | null>(null);
    const [error, setError] = useState<NameError | null>(null);

    useEffect(() => {
      // Reset form when dialog opens
      if (modal.visible) {
        setConfigName('');
        setCloneFrom(null);
        setError(null);
      }
    }, [modal.visible]);

    const validateConfigName = (name: string): NameError | null => {
      const trimmedName = name.trim();
      if (!trimmedName) return 'empty';
      if (trimmedName.length > 40) return 'tooLong';
      if (!/^[a-zA-Z0-9_-]+$/.test(trimmedName)) {
        return 'invalid';
      }
      if (existingConfigs.includes(trimmedName)) {
        return 'exists';
      }
      return null;
    };

    const handleCreate = () => {
      const validationError = validateConfigName(configName);
      if (validationError) {
        setError(validationError);
        return;
      }

      modal.resolve({
        action: 'created',
        configName: configName.trim(),
        cloneFrom,
      } as CreateConfigurationResult);
      modal.hide();
    };

    const handleCancel = () => {
      modal.resolve({ action: 'canceled' } as CreateConfigurationResult);
      modal.hide();
    };

    const handleOpenChange = (open: boolean) => {
      if (!open) {
        handleCancel();
      }
    };

    return (
      <Dialog
        role="dialog"
        aria-modal="true"
        aria-label={t('configuration.createTitle')}
        open={modal.visible}
        onOpenChange={handleOpenChange}
      >
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>{t('configuration.createTitle')}</DialogTitle>
            <DialogDescription>
              {t('configuration.createDescription', { executor: executorType })}
            </DialogDescription>
          </DialogHeader>

          <div className="space-y-4">
            <div className="space-y-2">
              <Label htmlFor="config-name">{t('configuration.name')}</Label>
              <Input
                id="config-name"
                value={configName}
                onChange={(e) => {
                  setConfigName(e.target.value);
                  setError(null);
                }}
                placeholder={t('configuration.namePlaceholder')}
                maxLength={40}
                autoFocus
              />
            </div>

            <div className="space-y-2">
              <Label htmlFor="clone-from">{t('configuration.cloneFrom')}</Label>
              <Select
                value={cloneFrom || '__blank__'}
                onValueChange={(value) =>
                  setCloneFrom(value === '__blank__' ? null : value)
                }
              >
                <SelectTrigger id="clone-from">
                  <SelectValue
                    placeholder={t('configuration.clonePlaceholder')}
                  />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="__blank__">
                    {t('configuration.blank')}
                  </SelectItem>
                  {existingConfigs.map((configuration) => (
                    <SelectItem key={configuration} value={configuration}>
                      {t('configuration.cloneOption', { name: configuration })}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>

            {error && (
              <Alert variant="destructive">
                <AlertDescription>
                  {
                    {
                      empty: t('configuration.errors.empty'),
                      tooLong: t('configuration.errors.tooLong'),
                      invalid: t('configuration.errors.invalid'),
                      exists: t('configuration.errors.exists'),
                    }[error]
                  }
                </AlertDescription>
              </Alert>
            )}
          </div>

          <DialogFooter>
            <Button type="button" variant="outline" onClick={handleCancel}>
              {t('common:buttons.cancel')}
            </Button>
            <Button onClick={handleCreate} disabled={!configName.trim()}>
              {t('configuration.create')}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    );
  }
);

export const CreateConfigurationDialog = defineModal<
  CreateConfigurationDialogProps,
  CreateConfigurationResult
>(CreateConfigurationDialogImpl);
