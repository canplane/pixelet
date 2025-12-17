import i18next from 'i18next';
import type { i18n, TOptions } from 'i18next';
import { useTranslation } from 'react-i18next';
import { enUS, ko } from 'date-fns/locale';

/* ------------------------------------------------------------------ */
/* Locale helpers */
/* ------------------------------------------------------------------ */

/** Returns date-fns locale based on current i18n language */
export function getI18nDateLocale(instance: i18n) {
  return instance.language.startsWith('ko') ? ko : enUS;
}

/* ------------------------------------------------------------------ */
/* String utilities */
/* ------------------------------------------------------------------ */

/**
 * Normalizes translated strings:
 * - Converts legacy newline tokens to actual newlines
 * - Replaces %s / %d placeholders with positional arguments
 */
function formatLegacyPlaceholders(
  text: string,
  args?: readonly string[],
): string {
  // Normalize newline tokens
  const normalized = text.replace(/\/n /g, '\n').replace(/\\\\n/g, '\n');

  if (!args || args.length === 0) {
    return normalized.replace(/%d|%s/g, '');
  }

  const parts = normalized.split(/%d|%s/g);
  const count = Math.min(args.length, parts.length - 1);

  let result = '';
  for (let i = 0; i < count; i += 1) {
    result += parts[i] + args[i];
  }

  return result + parts.slice(count).join('');
}

/* ------------------------------------------------------------------ */
/* React hook */
/* ------------------------------------------------------------------ */

/**
 * React hook for accessing translated text resources.
 *
 * Example:
 *   const text = useTextResource('common.save');
 */
export function useTextResource(key: string, args?: readonly string[]): string {
  const { t } = useTranslation();
  return formatLegacyPlaceholders(t(key), args);
}

/* ------------------------------------------------------------------ */
/* Non-hook utility (safe outside React) */
/* ------------------------------------------------------------------ */

/**
 * Retrieves a translated text resource.
 * Can be safely used outside React components.
 */
export function getTextResource(
  key: string,
  options?: TOptions | readonly string[],
): string {
  // i18next interpolation (object)
  if (options && !Array.isArray(options)) {
    return i18next.t(key, options as TOptions);
  }

  // Legacy positional placeholders
  const translated = i18next.t(key);
  return formatLegacyPlaceholders(translated, options);
}
