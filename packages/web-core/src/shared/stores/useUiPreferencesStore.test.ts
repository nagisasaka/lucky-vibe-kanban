import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  RIGHT_MAIN_PANEL_MODES as MODES,
  useUiPreferencesStore,
} from './useUiPreferencesStore';

const initialState = useUiPreferencesStore.getInitialState();

function viewport(width = 1440) {
  vi.stubGlobal('window', {
    innerWidth: width,
    matchMedia: () => ({ matches: width < 768 }),
  });
}

function panel(workspaceId = 'workspace-a') {
  return useUiPreferencesStore.getState().getWorkspacePanelState(workspaceId);
}

beforeEach(() => {
  useUiPreferencesStore.setState(initialState, true);
  viewport();
});

afterEach(() => vi.unstubAllGlobals());

describe('Wiki uses ordinary workspace panel navigation', () => {
  it.each(['setRightMainPanelMode', 'toggleRightMainPanelMode'] as const)(
    '%s opens beside chat and closes without restoring another pane',
    (action) => {
      const store = useUiPreferencesStore.getState();
      store.setRightMainPanelMode(MODES.LOGS, 'workspace-a');
      store[action](MODES.WIKI, 'workspace-a');
      expect(panel()).toEqual({
        rightMainPanelMode: MODES.WIKI,
        isLeftMainPanelVisible: true,
      });
      if (action === 'setRightMainPanelMode')
        store.setRightMainPanelMode(null, 'workspace-a');
      else store.toggleRightMainPanelMode(MODES.WIKI, 'workspace-a');
      expect(panel()).toEqual({
        rightMainPanelMode: null,
        isLeftMainPanelVisible: true,
      });
    }
  );

  it.each(Object.values(MODES).filter((mode) => mode !== MODES.WIKI))(
    'switches directly between Wiki and %s with the same chat visibility',
    (mode) => {
      const store = useUiPreferencesStore.getState();
      store.toggleRightMainPanelMode(MODES.WIKI, 'workspace-a');
      store.toggleRightMainPanelMode(mode, 'workspace-a');
      expect(panel()).toEqual({
        rightMainPanelMode: mode,
        isLeftMainPanelVisible: true,
      });
      store.toggleRightMainPanelMode(MODES.WIKI, 'workspace-a');
      expect(panel().isLeftMainPanelVisible).toBe(true);
    }
  );

  it('preserves explicit chat and sidebar preferences like other panes', () => {
    const store = useUiPreferencesStore.getState();
    store.setWorkspacePanelState('workspace-a', {
      rightMainPanelMode: MODES.FILES,
      isLeftMainPanelVisible: false,
    });
    useUiPreferencesStore.setState({ isRightSidebarVisible: false });
    store.setRightMainPanelMode(MODES.WIKI, 'workspace-a');
    expect(panel().isLeftMainPanelVisible).toBe(false);
    expect(useUiPreferencesStore.getState().isRightSidebarVisible).toBe(false);
    store.setRightMainPanelMode(MODES.FILES, 'workspace-a');
    expect(panel().isLeftMainPanelVisible).toBe(false);
  });

  it.each([390, 1000, 1440, 2560])(
    'uses existing sidebar and mobile rules at width %s',
    (width) => {
      viewport(width);
      useUiPreferencesStore
        .getState()
        .toggleRightMainPanelMode(MODES.WIKI, 'workspace-a');
      const state = useUiPreferencesStore.getState();
      expect(state.isLeftSidebarVisible).toBe(width > 2048);
      expect(state.mobileActiveTab).toBe(
        width < 768 ? 'wiki' : initialState.mobileActiveTab
      );
      expect(panel().isLeftMainPanelVisible).toBe(true);
    }
  );

  it('keeps workspace panel selections independent', () => {
    const store = useUiPreferencesStore.getState();
    store.setRightMainPanelMode(MODES.FILES, 'workspace-b');
    store.toggleRightMainPanelMode(MODES.WIKI, 'workspace-a');
    store.toggleRightMainPanelMode(MODES.WIKI, 'workspace-a');
    expect(panel('workspace-b').rightMainPanelMode).toBe(MODES.FILES);
  });

  it('closes a restored Wiki using only the persisted common panel state', () => {
    const store = useUiPreferencesStore.getState();
    store.setWorkspacePanelState('workspace-a', {
      rightMainPanelMode: MODES.WIKI,
      isLeftMainPanelVisible: true,
    });
    store.toggleRightMainPanelMode(MODES.WIKI, 'workspace-a');
    expect(panel().rightMainPanelMode).toBeNull();
  });

  it('is a no-op without a selected workspace', () => {
    const before = useUiPreferencesStore.getState();
    before.toggleRightMainPanelMode(MODES.WIKI);
    before.setRightMainPanelMode(MODES.WIKI);
    expect(useUiPreferencesStore.getState()).toBe(before);
  });
});
