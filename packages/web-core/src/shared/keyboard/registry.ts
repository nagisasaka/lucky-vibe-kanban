import type { SequentialActionId } from './labels';

export enum Scope {
  GLOBAL = 'global',
  DIALOG = 'dialog',
  CONFIRMATION = 'confirmation',
  KANBAN = 'kanban',
  PROJECTS = 'projects',
  SETTINGS = 'settings',
  EDIT_COMMENT = 'edit-comment',
  APPROVALS = 'approvals',
  FOLLOW_UP = 'follow-up',
  FOLLOW_UP_READY = 'follow-up-ready',
  WORKSPACE = 'workspace',
}

export enum Action {
  EXIT = 'exit',
  CREATE = 'create',
  SUBMIT = 'submit',
  FOCUS_SEARCH = 'focus_search',
  NAV_UP = 'nav_up',
  NAV_DOWN = 'nav_down',
  NAV_LEFT = 'nav_left',
  NAV_RIGHT = 'nav_right',
  OPEN_DETAILS = 'open_details',
  SHOW_HELP = 'show_help',
  DELETE_TASK = 'delete_task',
  APPROVE_REQUEST = 'approve_request',
  DENY_APPROVAL = 'deny_approval',
  SUBMIT_FOLLOW_UP = 'submit_follow_up',
  SUBMIT_TASK = 'submit_task',
  SUBMIT_TASK_ALT = 'submit_task_alt',
  SUBMIT_COMMENT = 'submit_comment',
  CYCLE_VIEW_BACKWARD = 'cycle_view_backward',
}

export interface KeyBinding {
  action: Action;
  keys: string | string[];
  scopes?: Scope[];
}

/**
 * Sequential keyboard shortcut binding (e.g., "g s" for Go to Settings)
 */
export interface SequentialBinding {
  id: string;
  keys: string[];
  scopes?: Scope[];
  actionId: SequentialActionId;
}

/**
 * Valid first keys for sequential shortcuts.
 * These keys will be intercepted to start a sequence.
 */
export const SEQUENCE_FIRST_KEYS = new Set([
  'g', // Go/Navigate
  'w', // Workspace
  'v', // View
  'x', // eXecute (git)
  'y', // Yank/Copy
  't', // Toggle
  'r', // Run
  'i', // Issue
]);

/**
 * All sequential keyboard shortcuts organized by namespace
 */
export const sequentialBindings: SequentialBinding[] = [
  // Navigation (G = Go)
  {
    id: 'seq-go-settings',
    keys: ['g', 's'],
    actionId: 'settings',
  },
  {
    id: 'seq-go-new-workspace',
    keys: ['g', 'n'],
    actionId: 'new-workspace',
  },

  // Workspace (W)
  {
    id: 'seq-workspace-duplicate',
    keys: ['w', 'd'],
    actionId: 'duplicate-workspace',
  },
  {
    id: 'seq-workspace-rename',
    keys: ['w', 'r'],
    actionId: 'rename-workspace',
  },
  {
    id: 'seq-workspace-pin',
    keys: ['w', 'p'],
    actionId: 'pin-workspace',
  },
  {
    id: 'seq-workspace-archive',
    keys: ['w', 'a'],
    actionId: 'archive-workspace',
  },
  {
    id: 'seq-workspace-delete',
    keys: ['w', 'x'],
    actionId: 'delete-workspace',
  },

  // View (V)
  {
    id: 'seq-view-changes',
    keys: ['v', 'c'],
    actionId: 'toggle-changes-mode',
  },
  {
    id: 'seq-view-logs',
    keys: ['v', 'l'],
    actionId: 'toggle-logs-mode',
  },
  {
    id: 'seq-view-preview',
    keys: ['v', 'p'],
    actionId: 'toggle-preview-mode',
  },
  {
    id: 'seq-view-sidebar',
    keys: ['v', 's'],
    actionId: 'toggle-left-sidebar',
  },
  {
    id: 'seq-view-chat',
    keys: ['v', 'h'],
    actionId: 'toggle-left-main-panel',
  },

  // Git (X = eXecute)
  {
    id: 'seq-git-pr',
    keys: ['x', 'p'],
    scopes: [Scope.WORKSPACE],
    actionId: 'git-create-pr',
  },
  {
    id: 'seq-git-merge',
    keys: ['x', 'm'],
    scopes: [Scope.WORKSPACE],
    actionId: 'git-merge',
  },
  {
    id: 'seq-git-rebase',
    keys: ['x', 'r'],
    scopes: [Scope.WORKSPACE],
    actionId: 'git-rebase',
  },
  {
    id: 'seq-git-push',
    keys: ['x', 'u'],
    scopes: [Scope.WORKSPACE],
    actionId: 'git-push',
  },

  // Yank/Copy (Y)
  {
    id: 'seq-yank-path',
    keys: ['y', 'p'],
    scopes: [Scope.WORKSPACE],
    actionId: 'copy-path',
  },
  {
    id: 'seq-yank-logs',
    keys: ['y', 'l'],
    scopes: [Scope.WORKSPACE],
    actionId: 'copy-raw-logs',
  },

  // Toggle (T)
  {
    id: 'seq-toggle-dev-server',
    keys: ['t', 'd'],
    scopes: [Scope.WORKSPACE],
    actionId: 'toggle-dev-server',
  },
  {
    id: 'seq-toggle-wrap',
    keys: ['t', 'w'],
    scopes: [Scope.WORKSPACE],
    actionId: 'toggle-wrap-lines',
  },

  // Run (R)
  {
    id: 'seq-run-setup',
    keys: ['r', 's'],
    scopes: [Scope.WORKSPACE],
    actionId: 'run-setup-script',
  },
  {
    id: 'seq-run-cleanup',
    keys: ['r', 'c'],
    scopes: [Scope.WORKSPACE],
    actionId: 'run-cleanup-script',
  },
  {
    id: 'seq-run-archive',
    keys: ['r', 'a'],
    scopes: [Scope.WORKSPACE],
    actionId: 'run-archive-script',
  },

  // Issue (I)
  {
    id: 'seq-issue-create',
    keys: ['i', 'c'],
    actionId: 'create-issue',
  },
  {
    id: 'seq-issue-status',
    keys: ['i', 's'],
    actionId: 'change-issue-status',
  },
  {
    id: 'seq-issue-priority',
    keys: ['i', 'p'],
    actionId: 'change-issue-priority',
  },
  {
    id: 'seq-issue-assignees',
    keys: ['i', 'a'],
    actionId: 'change-assignees',
  },
  {
    id: 'seq-issue-make-sub-issue',
    keys: ['i', 'm'],
    actionId: 'make-sub-issue-of',
  },
  {
    id: 'seq-issue-add-sub-issue',
    keys: ['i', 'b'],
    actionId: 'add-sub-issue',
  },
  {
    id: 'seq-issue-remove-parent',
    keys: ['i', 'u'],
    actionId: 'remove-parent-issue',
  },
  {
    id: 'seq-issue-link-workspace',
    keys: ['i', 'w'],
    actionId: 'link-workspace',
  },
  {
    id: 'seq-issue-duplicate',
    keys: ['i', 'd'],
    actionId: 'duplicate-issue',
  },
  {
    id: 'seq-issue-delete',
    keys: ['i', 'x'],
    actionId: 'delete-issue',
  },
];

export const keyBindings: KeyBinding[] = [
  // Exit/Close actions
  {
    action: Action.EXIT,
    keys: 'esc',
    scopes: [Scope.CONFIRMATION],
  },
  {
    action: Action.EXIT,
    keys: 'esc',
    scopes: [Scope.DIALOG],
  },
  {
    action: Action.EXIT,
    keys: 'esc',
    scopes: [Scope.KANBAN],
  },
  {
    action: Action.EXIT,
    keys: 'esc',
    scopes: [Scope.EDIT_COMMENT],
  },
  {
    action: Action.EXIT,
    keys: 'esc',
    scopes: [Scope.SETTINGS],
  },

  // Creation actions
  {
    action: Action.CREATE,
    keys: 'c',
    scopes: [Scope.KANBAN],
  },
  {
    action: Action.CREATE,
    keys: 'c',
    scopes: [Scope.PROJECTS],
  },

  // Submit actions
  {
    action: Action.SUBMIT,
    keys: 'enter',
    scopes: [Scope.DIALOG],
  },

  // Navigation actions
  {
    action: Action.FOCUS_SEARCH,
    keys: 'slash',
    scopes: [Scope.KANBAN],
  },
  {
    action: Action.NAV_UP,
    keys: 'k',
    scopes: [Scope.KANBAN],
  },
  {
    action: Action.NAV_DOWN,
    keys: 'j',
    scopes: [Scope.KANBAN],
  },
  {
    action: Action.NAV_LEFT,
    keys: 'h',
    scopes: [Scope.KANBAN],
  },
  {
    action: Action.NAV_RIGHT,
    keys: 'l',
    scopes: [Scope.KANBAN],
  },
  {
    action: Action.OPEN_DETAILS,
    keys: ['meta+enter', 'ctrl+enter'],
    scopes: [Scope.KANBAN],
  },
  {
    action: Action.CYCLE_VIEW_BACKWARD,
    keys: ['meta+shift+enter', 'ctrl+shift+enter'],
    scopes: [Scope.KANBAN],
  },

  // Global actions
  {
    action: Action.SHOW_HELP,
    keys: 'shift+slash',
    scopes: [Scope.GLOBAL],
  },

  // Task actions
  {
    action: Action.DELETE_TASK,
    keys: 'd',
    scopes: [Scope.KANBAN],
  },

  // Approval actions
  {
    action: Action.APPROVE_REQUEST,
    keys: 'enter',
    scopes: [Scope.APPROVALS],
  },
  {
    action: Action.DENY_APPROVAL,
    keys: ['meta+enter', 'ctrl+enter'],
    scopes: [Scope.APPROVALS],
  },

  // Follow-up actions
  {
    action: Action.SUBMIT_FOLLOW_UP,
    keys: 'meta+enter',
    scopes: [Scope.FOLLOW_UP_READY],
  },
  {
    action: Action.SUBMIT_TASK,
    keys: ['meta+enter', 'ctrl+enter'],
    scopes: [Scope.DIALOG],
  },
  {
    action: Action.SUBMIT_TASK_ALT,
    keys: ['meta+shift+enter', 'ctrl+shift+enter'],
    scopes: [Scope.DIALOG],
  },
  {
    action: Action.SUBMIT_COMMENT,
    keys: ['meta+enter', 'ctrl+enter'],
    scopes: [Scope.EDIT_COMMENT],
  },
];

/**
 * Get keyboard bindings for a specific action and scope
 */
export function getKeysFor(action: Action, scope?: Scope): string[] {
  const bindings = keyBindings
    .filter(
      (binding) =>
        binding.action === action &&
        (!scope || !binding.scopes || binding.scopes.includes(scope))
    )
    .flatMap((binding) =>
      Array.isArray(binding.keys) ? binding.keys : [binding.keys]
    );

  return bindings;
}

/**
 * Get binding info for a specific action and scope
 */
export function getBindingFor(
  action: Action,
  scope?: Scope
): KeyBinding | undefined {
  return keyBindings.find(
    (binding) =>
      binding.action === action &&
      (!scope || !binding.scopes || binding.scopes.includes(scope))
  );
}

/**
 * Get sequential binding for a specific action ID
 */
export function getSequentialBindingFor(
  actionId: string
): SequentialBinding | undefined {
  return sequentialBindings.find((binding) => binding.actionId === actionId);
}

/**
 * Format sequential keys for display (e.g., ['g', 's'] -> 'G S')
 */
export function formatSequentialKeys(keys: string[]): string {
  return keys.map((k) => k.toUpperCase()).join(' ');
}
