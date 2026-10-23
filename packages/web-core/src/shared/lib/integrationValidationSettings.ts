// File-editor transport; the plan itself remains the repository's JSON file.
export interface IntegrationValidationSettings {
  branch: string;
  head: string;
  revision: string;
  content: string | null;
}

export interface SaveIntegrationValidationSettings {
  expected_revision: string;
  content: string;
}

export function editableCommands(content: string): string[] | null {
  try {
    const plan = JSON.parse(content);
    return plan &&
      !Array.isArray(plan) &&
      Object.keys(plan).length === 1 &&
      Array.isArray(plan.commands) &&
      plan.commands.every((command: unknown) => typeof command === 'string')
      ? plan.commands
      : null;
  } catch {
    return null;
  }
}

export function validationCommands(content: string): string[] | null {
  const commands = editableCommands(content);
  return commands &&
    commands.length >= 1 &&
    commands.length <= 32 &&
    commands.every(
      (command) =>
        command.trim() && !['true', ':', 'exit 0'].includes(command.trim())
    )
    ? commands
    : null;
}
