import { describe, expect, it } from 'vitest';
import type { Pipeline } from 'shared/types';
import {
  PIPELINE_END,
  PIPELINE_START,
  composePipelineBlock,
  extractPipelineBlock,
  extractManualPipelineLines,
  inferPipelineSelection,
  removePipelineBlock,
  updatePipelineBlock,
} from './cardPipeline';

const pipeline: Pipeline = {
  id: 'quality',
  name: 'Quality checks',
  description: null,
  stages: [
    {
      id: 'inspect',
      label: 'Inspect changes',
      prompt_fragment: 'Inspect.',
      default_enabled: true,
      heavy: false,
    },
    {
      id: 'report',
      label: 'Report results',
      prompt_fragment: 'Report.',
      default_enabled: true,
      heavy: false,
    },
  ],
};

describe('card pipeline block', () => {
  it('preserves stage order and is idempotent', () => {
    const once = updatePipelineBlock('Manual text', pipeline, [
      'inspect',
      'report',
    ]);
    const twice = updatePipelineBlock(once, pipeline, ['inspect', 'report']);
    expect(twice).toBe(once);
    expect(once.indexOf('Inspect.')).toBeLessThan(once.indexOf('Report.'));
  });

  it('replaces or removes only the generated block', () => {
    const initial = updatePipelineBlock('Before\n\nAfter', pipeline, [
      'inspect',
      'report',
    ]);
    expect(removePipelineBlock(initial)).toBe('Before\n\nAfter');
    expect(updatePipelineBlock(initial, pipeline, ['report'])).toContain(
      'Report.'
    );
    expect(updatePipelineBlock(initial, pipeline, ['report'])).not.toContain(
      'Inspect.'
    );
  });

  it('preserves manual additions made inside a generated block', () => {
    const initial = composePipelineBlock(pipeline, [
      'inspect',
      'report',
    ]).replace(
      PIPELINE_END,
      `Operator note: keep this constraint.\n${PIPELINE_END}`
    );
    expect(extractManualPipelineLines(initial, [pipeline])).toEqual([
      'Operator note: keep this constraint.',
    ]);
    const updated = updatePipelineBlock(
      initial,
      pipeline,
      ['inspect'],
      [pipeline]
    );
    expect(updated).toContain('Operator note: keep this constraint.');
    expect(updated).not.toContain('Report.');
    expect(updatePipelineBlock(initial, null, [], [pipeline])).toBe(
      'Operator note: keep this constraint.'
    );
  });

  it('does not preserve obsolete generated prompts after a definition update', () => {
    const initial = composePipelineBlock(pipeline, ['inspect']);
    const updatedPipeline = {
      ...pipeline,
      stages: pipeline.stages.map((stage) =>
        stage.id === 'inspect'
          ? { ...stage, prompt_fragment: 'Inspect with the new rules.' }
          : stage
      ),
    };
    const updated = updatePipelineBlock(
      initial,
      updatedPipeline,
      ['inspect'],
      [updatedPipeline]
    );
    expect(updated).toContain('Inspect with the new rules.');
    expect(updated).not.toContain('**Inspect changes:** Inspect.');
  });

  it('does not interpret marker text embedded in prose', () => {
    const prose = `quote ${PIPELINE_START} here and ${PIPELINE_END} there`;
    expect(removePipelineBlock(prose)).toBe(prose);
  });

  it('extracts the last complete block and infers selected stages', () => {
    const old = composePipelineBlock(pipeline, ['inspect']);
    const current = composePipelineBlock(pipeline, ['report']);
    const description = `${old}\nmanual\n${current}`;
    expect(extractPipelineBlock(description)).toBe(current);
    expect(inferPipelineSelection(description, [pipeline])).toEqual({
      pipelineId: 'quality',
      stageIds: new Set(['report']),
    });
  });

  it('leaves ordinary cards unchanged', () => {
    expect(removePipelineBlock('ordinary')).toBe('ordinary');
    expect(updatePipelineBlock('ordinary', null, [])).toBe('ordinary');
  });
});
