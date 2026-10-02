import type { CareStepReport } from '../../lib/contracts';
import type { Locale } from '../../lib/i18n';

/** Logical deleted-file bytes. Never infer free space or coerce an exact u64 to Number. */
export function measuredDeletedBytes(step: CareStepReport, locale: Locale): string | null {
  if (step.domainKind !== 'Cleanup' || step.state !== 'Completed' || step.outcome !== 'VerifiedByDomain'
    || step.domainVerificationState !== 'Verified' || !step.hasActualDeletedBytes
    || !/^(0|[1-9][0-9]{0,19})$/.test(step.actualDeletedBytes)) return null;
  const bytes = BigInt(step.actualDeletedBytes);
  if (bytes > 18446744073709551615n) return null;
  const units = ['B', 'KiB', 'MiB', 'GiB', 'TiB', 'PiB', 'EiB'];
  let unit = 0, divisor = 1n;
  while (unit < units.length - 1 && bytes >= divisor * 1024n) { ++unit; divisor *= 1024n; }
  const numberLocale = locale === 'ar' ? 'ar-IQ-u-nu-latn' : 'en-US';
  const rounded = (bytes * 100n + divisor / 2n) / divisor;
  const integer = new Intl.NumberFormat(numberLocale).format(rounded / 100n);
  if (unit === 0) return `${integer} B`;
  const separator = new Intl.NumberFormat(numberLocale).formatToParts(1.1).find((p) => p.type === 'decimal')!.value;
  const fraction = new Intl.NumberFormat(numberLocale, { minimumIntegerDigits: 2 }).format(Number(rounded % 100n));
  return `${integer}${separator}${fraction} ${units[unit]}`;
}
