import { describe, expect, it } from 'vitest';
import {
  editableCommands,
  validationCommands,
} from './integrationValidationSettings';

describe('Integration validation input', () => {
  it('preserves legacy order and multiline commands without executing them', () => {
    expect(
      validationCommands('{"commands":["echo first\\necho second","false"]}')
    ).toEqual(['echo first\necho second', 'false']);
    expect(editableCommands('{"commands":[""]}')).toEqual(['']);
  });
  it.each([
    'null',
    '[]',
    '{}',
    '{',
    '{"commands":[]}',
    '{"commands":[null]}',
    '{"commands":[" "]}',
    '{"commands":["true"]}',
    '{"commands":[":"]}',
    '{"commands":["exit 0"]}',
    '{"commands":["echo test"],"extra":true}',
  ])('rejects %s', (content) => {
    expect(validationCommands(content)).toBeNull();
  });
  it('enforces 1–32 commands', () => {
    expect(
      validationCommands(
        JSON.stringify({ commands: Array(32).fill('echo test') })
      )
    ).toHaveLength(32);
    expect(
      validationCommands(
        JSON.stringify({ commands: Array(33).fill('echo test') })
      )
    ).toBeNull();
  });
});
