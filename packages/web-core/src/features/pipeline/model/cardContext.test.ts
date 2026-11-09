import { describe, expect, it } from 'vitest';
import type { Pipeline } from 'shared/types';
import { buildWorkspaceCreatePrompt } from '@/shared/lib/workspaceCreateState';
import {
  composePipelineBlock,
  extractPipelineBlock,
  updatePipelineBlock,
} from './cardPipeline';
import {
  cardContextPreview,
  defaultCardContext,
  hasSharedDirectories,
  replaceCardDescription,
  SHARED_DIRECTORIES_CONTEXT,
  splitCardContext,
  toggleSharedDirectories,
  withCardContext,
} from './cardContext';

const pipeline: Pipeline = {
  id: 'quality',
  name: 'Quality checks',
  description: null,
  stages: [
    {
      id: 'inspect',
      label: 'Inspect',
      prompt_fragment: 'Inspect the proposed change.',
      default_enabled: true,
      heavy: false,
    },
    {
      id: 'report',
      label: 'Report',
      prompt_fragment: 'Report verification results.',
      default_enabled: true,
      heavy: false,
    },
  ],
};

describe('card context', () => {
  it('passes shared directories without enabling a Pipeline through the existing initial workspace request', () => {
    const stored = defaultCardContext('Implement CRUD', [pipeline]);
    const prompt = buildWorkspaceCreatePrompt('Feature', stored);
    expect(prompt).toBe(`Feature\n\n${stored}`);
    expect(extractPipelineBlock(prompt!)).toBe('');
    expect(prompt).toContain(SHARED_DIRECTORIES_CONTEXT);
    expect(prompt).toContain('.lvk/shared/tmp/');
    expect(prompt).toContain('Run completion does not delete tmp contents');
    expect(prompt).toContain('never delete pending handoffs or active data');
    expect(prompt).toContain('appropriate retained destination');
    expect(
      splitCardContext(JSON.parse(JSON.stringify(stored))).description
    ).toBe('Implement CRUD');
  });

  it('defaults shared directories only while keeping the task editor free of generated text', () => {
    const stored = defaultCardContext('Build a feature', [pipeline]);
    const split = splitCardContext(stored);
    expect(split.description).toBe('Build a feature');
    expect(extractPipelineBlock(stored)).toBe('');
    expect(hasSharedDirectories(split.context)).toBe(true);
    expect(defaultCardContext(stored, [pipeline])).toBe(stored);
    expect(cardContextPreview(split.context)).not.toContain('<!--');
    expect(cardContextPreview(split.context)).not.toContain(
      'Inspect the proposed change.'
    );
  });

  it('keeps explicit opt-outs on draft reopening and description edits', () => {
    const disabled = withCardContext('Task', '');
    expect(defaultCardContext(disabled, [pipeline])).toBe(disabled);
    const edited = replaceCardDescription(disabled, 'New task');
    expect(splitCardContext(defaultCardContext(edited, [pipeline]))).toEqual({
      description: 'New task',
      context: '',
    });
  });

  it('reads stored Pipeline blocks and preserves their exact instructions on description edits', () => {
    const block = composePipelineBlock(pipeline, ['inspect']);
    const old = `Before\n\n${block}\n\nAfter`;
    expect(splitCardContext(old)).toEqual({
      description: 'Before\n\nAfter',
      context: block,
    });
    expect(defaultCardContext(old, [pipeline])).toBe(old);
    expect(replaceCardDescription(old, 'Edited')).toBe(`Edited\n\n${block}`);
    expect(splitCardContext('plain existing card').context).toBe('');
  });

  it('preserves stored context even if the preset definition changes', () => {
    const original = withCardContext(
      'Task',
      composePipelineBlock(pipeline, ['inspect'])
    );
    const changed = { ...pipeline, stages: [] };
    expect(
      splitCardContext(
        defaultCardContext(replaceCardDescription(original, 'Edit'), [changed])
      ).context
    ).toBe(splitCardContext(original).context);
  });

  it('toggles shared directories independently of Pipelines and never duplicates them', () => {
    const context = toggleSharedDirectories(
      composePipelineBlock(pipeline, ['inspect']),
      true
    );
    const noPipeline = updatePipelineBlock(context, null, [], [pipeline]);
    expect(hasSharedDirectories(noPipeline)).toBe(true);
    expect(extractPipelineBlock(noPipeline)).toBe('');
    const noShared = toggleSharedDirectories(context, false);
    expect(extractPipelineBlock(noShared)).toBe(
      composePipelineBlock(pipeline, ['inspect'])
    );
    expect(
      toggleSharedDirectories(toggleSharedDirectories('', true), true)
    ).toBe(SHARED_DIRECTORIES_CONTEXT);
  });

  it('does not interpret inline marker mentions or incomplete blocks as context', () => {
    for (const description of [
      'Explain <!-- lvk:card-context:start --> here',
      '<!-- lvk:card-context:start -->\nUnfinished',
    ]) {
      expect(splitCardContext(description)).toEqual({
        description,
        context: '',
      });
    }
  });
});
