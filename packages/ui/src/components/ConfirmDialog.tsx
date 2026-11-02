import { useId } from 'react';
import { useTranslation } from 'react-i18next';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from './KeyboardDialog';
import { Button } from './Button';
import NiceModal, { useModal } from '@ebay/nice-modal-react';
import {
  WarningIcon,
  InfoIcon,
  CheckCircleIcon,
  XCircleIcon,
} from '@phosphor-icons/react';
import { defineModal, type ConfirmResult } from '../lib/modals';

// Deferred owned text follows live UI language changes while the dialog is open.
export type ConfirmDialogText = string | (() => string);

export interface ConfirmDialogProps {
  title: ConfirmDialogText;
  message: ConfirmDialogText;
  confirmText?: ConfirmDialogText;
  cancelText?: ConfirmDialogText;
  variant?: 'default' | 'destructive' | 'info' | 'success';
  icon?: boolean;
  showCancelButton?: boolean;
}

const ConfirmDialogImpl = NiceModal.create<ConfirmDialogProps>((props) => {
  const { t } = useTranslation(['tasks', 'common']);
  const modal = useModal();
  const titleId = useId();
  const descriptionId = useId();
  const {
    title,
    message,
    confirmText = t('common:confirm.defaultConfirm'),
    cancelText = t('common:confirm.defaultCancel'),
    variant = 'default',
    icon = true,
    showCancelButton = true,
  } = props;

  const text = (value: ConfirmDialogText) =>
    typeof value === 'function' ? value() : value;

  const handleConfirm = () => {
    modal.resolve('confirmed' as ConfirmResult);
    modal.hide();
  };

  const handleCancel = () => {
    modal.resolve('canceled' as ConfirmResult);
    modal.hide();
  };

  const getIcon = () => {
    if (!icon) return null;

    const iconClass = 'h-6 w-6';

    switch (variant) {
      case 'destructive':
        return <WarningIcon className={`${iconClass} text-destructive`} />;
      case 'info':
        return <InfoIcon className={`${iconClass} text-blue-500`} />;
      case 'success':
        return <CheckCircleIcon className={`${iconClass} text-green-500`} />;
      default:
        return <XCircleIcon className={`${iconClass} text-muted-foreground`} />;
    }
  };

  const getConfirmButtonVariant = () => {
    return variant === 'destructive' ? 'destructive' : 'default';
  };

  return (
    <Dialog
      open={modal.visible}
      onOpenChange={handleCancel}
      role="dialog"
      aria-modal="true"
      aria-labelledby={titleId}
      aria-describedby={descriptionId}
    >
      <DialogContent className="sm:max-w-[425px]">
        <DialogHeader>
          <div className="flex items-center gap-3">
            {getIcon()}
            <DialogTitle id={titleId}>{text(title)}</DialogTitle>
          </div>
          <DialogDescription id={descriptionId} className="text-left pt-2">
            {text(message)}
          </DialogDescription>
        </DialogHeader>
        {showCancelButton ? (
          <DialogFooter className="gap-2">
            <Button variant="outline" onClick={handleCancel}>
              {text(cancelText)}
            </Button>
            <Button variant={getConfirmButtonVariant()} onClick={handleConfirm}>
              {text(confirmText)}
            </Button>
          </DialogFooter>
        ) : (
          <div className="flex w-full">
            <Button
              className="ml-auto"
              variant={getConfirmButtonVariant()}
              onClick={handleConfirm}
            >
              {text(confirmText)}
            </Button>
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
});

export const ConfirmDialog = defineModal<ConfirmDialogProps, ConfirmResult>(
  ConfirmDialogImpl
);
