import type { AskUserQuestionItem, JsonValue } from 'shared/types';

/** Project provider fields only when their structured shape is present. */
export function nativeInputQuestions(
  metadata: JsonValue | undefined
): AskUserQuestionItem[] | null {
  const object = (value: JsonValue | undefined) =>
    value && typeof value === 'object' && !Array.isArray(value) ? value : null;
  const params = object(object(metadata)?.params);
  if (!Array.isArray(params?.questions)) return null;
  const questions: AskUserQuestionItem[] = [];
  for (const value of params.questions) {
    const question = object(value);
    if (
      typeof question?.question !== 'string' ||
      typeof question.header !== 'string'
    )
      return null;
    const options = Array.isArray(question.options) ? question.options : [];
    questions.push({
      ...(typeof question.id === 'string' ? { id: question.id } : {}),
      question: question.question,
      header: question.header,
      multiSelect: false,
      options: options.flatMap((value) => {
        const option = object(value);
        return typeof option?.label === 'string' &&
          typeof option.description === 'string'
          ? [{ label: option.label, description: option.description }]
          : [];
      }),
    });
  }
  return questions.length ? questions : null;
}
