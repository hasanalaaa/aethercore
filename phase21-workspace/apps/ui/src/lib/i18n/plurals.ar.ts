import type { PluralMessageKey } from './plurals.en';

type ArabicPlural = { zero: string; one: string; two: string; few: string; many: string; other: string };
export const arPlurals = {
  'unit.finding': { zero: 'لا نتائج', one: 'نتيجة واحدة', two: 'نتيجتان', few: '{count} نتائج', many: '{count} نتيجة', other: '{count} نتيجة' },
  'unit.file': { zero: 'لا ملفات', one: 'ملف واحد', two: 'ملفان', few: '{count} ملفات', many: '{count} ملفًا', other: '{count} ملف' },
  'unit.category': { zero: 'لا فئات', one: 'فئة واحدة', two: 'فئتان', few: '{count} فئات', many: '{count} فئةً', other: '{count} فئة' },
  'unit.action': { zero: 'لا إجراءات', one: 'إجراء واحد', two: 'إجراءان', few: '{count} إجراءات', many: '{count} إجراءً', other: '{count} إجراء' },
  'unit.deviceAction': { zero: 'لا إجراءات أجهزة', one: 'إجراء جهاز واحد', two: 'إجراءا جهاز', few: '{count} إجراءات أجهزة', many: '{count} إجراء جهاز', other: '{count} إجراء جهاز' },
  'unit.update': { zero: 'لا تحديثات', one: 'تحديث واحد', two: 'تحديثان', few: '{count} تحديثات', many: '{count} تحديثًا', other: '{count} تحديث' },
  'unit.serviceTarget': { zero: 'لا أهداف خدمات', one: 'هدف خدمة واحد', two: 'هدفا خدمة', few: '{count} أهداف خدمات', many: '{count} هدف خدمة', other: '{count} هدف خدمة' },
  'unit.offer': { zero: 'لا عروض', one: 'عرض واحد', two: 'عرضان', few: '{count} عروض', many: '{count} عرضًا', other: '{count} عرض' },
  'unit.day': { zero: 'صفر يوم', one: 'يوم واحد', two: 'يومان', few: '{count} أيام', many: '{count} يومًا', other: '{count} يوم' },
  'unit.triageCard': { zero: 'لا بطاقات فرز', one: 'بطاقة فرز واحدة', two: 'بطاقتا فرز', few: '{count} بطاقات فرز', many: '{count} بطاقة فرز', other: '{count} بطاقة فرز' },
  'unit.warning': { zero: 'لا تحذيرات', one: 'تحذير واحد', two: 'تحذيران', few: '{count} تحذيرات', many: '{count} تحذيرًا', other: '{count} تحذير' },
  'unit.occurrence': { zero: 'لا تكرارات', one: 'تكرار واحد', two: 'تكراران', few: '{count} تكرارات', many: '{count} تكرارًا', other: '{count} تكرار' },
  'unit.careStep': { zero: 'لا خطوات عناية', one: 'خطوة عناية واحدة', two: 'خطوتا عناية', few: '{count} خطوات عناية', many: '{count} خطوة عناية', other: '{count} خطوة عناية' },
} satisfies Record<PluralMessageKey, ArabicPlural>;
