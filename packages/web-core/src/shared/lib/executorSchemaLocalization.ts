import type { TFunction } from 'i18next';
import type { RJSFSchema } from '@rjsf/utils';

// Generated metadata identifies presentation strings. Keep canonical keys,
// enum values, defaults and all validation keywords exactly as generated.
export function localizeExecutorSchema(
  schema: RJSFSchema,
  t: TFunction<'settings'>
): RJSFSchema {
  const properties = Object.fromEntries(
    Object.entries(schema.properties ?? {}).map(([name, property]) => {
      if (typeof property !== 'object') return [name, property];
      const localized = { ...property };
      for (const annotation of ['title', 'description'] as const) {
        const key = (property as RJSFSchema)[`x-lvk-i18n-${annotation}`];
        if (typeof key === 'string') localized[annotation] = t(key);
      }
      return [name, localized];
    })
  );
  return { ...schema, properties };
}

export function localizeSchemaErrors(
  errors: import('@rjsf/utils').RJSFValidationError[],
  t: TFunction<'settings'>
) {
  return errors.map((error) => {
    const limit = error.params?.limit;
    let message: string;
    switch (error.name) {
      case 'required':
        message = t('settings:executorSchemaValidation.required');
        break;
      case 'type':
        message = t('settings:executorSchemaValidation.type');
        break;
      case 'enum':
      case 'const':
        message = t('settings:executorSchemaValidation.option');
        break;
      case 'minimum':
        message = t('settings:executorSchemaValidation.minimum', { limit });
        break;
      case 'maximum':
        message = t('settings:executorSchemaValidation.maximum', { limit });
        break;
      case 'minLength':
        message = t('settings:executorSchemaValidation.minLength', { limit });
        break;
      case 'maxLength':
        message = t('settings:executorSchemaValidation.maxLength', { limit });
        break;
      case 'minItems':
        message = t('settings:executorSchemaValidation.minItems', { limit });
        break;
      case 'maxItems':
        message = t('settings:executorSchemaValidation.maxItems', { limit });
        break;
      case 'pattern':
      case 'format':
        message = t('settings:executorSchemaValidation.format');
        break;
      case 'additionalProperties':
        message = t('settings:executorSchemaValidation.additionalProperties');
        break;
      case 'uniqueItems':
        message = t('settings:executorSchemaValidation.uniqueItems');
        break;
      default:
        message = t('settings:executorSchemaValidation.invalid');
    }
    return { ...error, message, stack: `${error.property ?? ''}: ${message}` };
  });
}

// RJSF accepts localized enum labels through uiSchema; validation enum values
// stay untouched, including the distinct null inheritance choice.
export function executorSchemaUiOptions(
  schema: RJSFSchema | undefined,
  t: TFunction<'settings'>
) {
  return Object.fromEntries(
    Object.entries(schema?.properties ?? {}).flatMap(([name, property]) => {
      if (typeof property !== 'object') return [];
      const labels: unknown = (property as RJSFSchema)['x-lvk-i18n-enum'];
      if (!Array.isArray(labels) || labels.length !== property.enum?.length)
        return [];
      return [
        [
          name,
          {
            'ui:enumNames': labels.map((label: { key: string }) =>
              t(label.key)
            ),
          },
        ],
      ];
    })
  );
}
