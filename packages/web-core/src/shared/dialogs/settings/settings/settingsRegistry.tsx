import { lazy } from 'react';
import {
  GearIcon,
  GitBranchIcon,
  BuildingsIcon,
  KanbanIcon,
  CpuIcon,
  PlugIcon,
  WrenchIcon,
} from '@phosphor-icons/react';
import type { Icon } from '@phosphor-icons/react';

const GeneralSettingsSection = lazy(() =>
  import('./GeneralSettingsSection').then((module) => ({
    default: module.GeneralSettingsSection,
  }))
);
const ReposSettingsSection = lazy(() =>
  import('./ReposSettingsSection').then((module) => ({
    default: module.ReposSettingsSection,
  }))
);
const RemoteProjectsSettingsSection = lazy(() =>
  import('./RemoteProjectsSettingsSection').then((module) => ({
    default: module.RemoteProjectsSettingsSection,
  }))
);
const AgentsSettingsSection = lazy(() =>
  import('./AgentsSettingsSection').then((module) => ({
    default: module.AgentsSettingsSection,
  }))
);
const McpSettingsSection = lazy(() =>
  import('./McpSettingsSection').then((module) => ({
    default: module.McpSettingsSection,
  }))
);
const AgentToolsSettingsSection = lazy(() =>
  import('./AgentToolsSettingsSection').then((module) => ({
    default: module.AgentToolsSettingsSection,
  }))
);
const ResourceSettingsSection = lazy(() =>
  import('./ResourceSettingsSection').then((module) => ({
    default: module.ResourceSettingsSection,
  }))
);

export type SettingsSectionType =
  | 'general'
  | 'repos'
  | 'remote-projects'
  | 'agents'
  | 'mcp'
  | 'agent-tools'
  | 'resources';

export type SettingsSectionGroup = 'host' | 'universal';

export type SettingsSectionInitialState = {
  general: undefined;
  repos: { repoId?: string } | undefined;
  'remote-projects':
    | { organizationId?: string; projectId?: string }
    | undefined;
  agents: { executor?: string; variant?: string } | undefined;
  mcp: undefined;
  'agent-tools': undefined;
  resources: undefined;
};

export interface SettingsSectionDefinition {
  id: SettingsSectionType;
  icon: Icon;
  group: SettingsSectionGroup;
  showInNavigation?: boolean;
}

export const SETTINGS_SECTION_DEFINITIONS: SettingsSectionDefinition[] = [
  { id: 'general', icon: GearIcon, group: 'host' },
  {
    id: 'repos',
    icon: GitBranchIcon,
    group: 'host',
    showInNavigation: false,
  },
  { id: 'agents', icon: CpuIcon, group: 'host' },
  {
    id: 'mcp',
    icon: PlugIcon,
    group: 'host',
    showInNavigation: false,
  },
  { id: 'agent-tools', icon: WrenchIcon, group: 'host' },
  { id: 'resources', icon: BuildingsIcon, group: 'host' },
  { id: 'remote-projects', icon: KanbanIcon, group: 'universal' },
];

export function isHostSpecificSettingsSection(
  type: SettingsSectionType
): boolean {
  return (
    SETTINGS_SECTION_DEFINITIONS.find((section) => section.id === type)
      ?.group === 'host'
  );
}

export function renderSettingsSection(
  type: SettingsSectionType,
  initialState?: SettingsSectionInitialState[SettingsSectionType]
) {
  switch (type) {
    case 'general':
      return <GeneralSettingsSection />;
    case 'repos':
      return (
        <ReposSettingsSection
          initialState={initialState as SettingsSectionInitialState['repos']}
        />
      );
    case 'remote-projects':
      return (
        <RemoteProjectsSettingsSection
          initialState={
            initialState as SettingsSectionInitialState['remote-projects']
          }
        />
      );
    case 'agents':
      return <AgentsSettingsSection />;
    case 'mcp':
      return <McpSettingsSection />;
    case 'agent-tools':
      return <AgentToolsSettingsSection />;
    case 'resources':
      return <ResourceSettingsSection key="resources" />;
    default:
      return <GeneralSettingsSection />;
  }
}
