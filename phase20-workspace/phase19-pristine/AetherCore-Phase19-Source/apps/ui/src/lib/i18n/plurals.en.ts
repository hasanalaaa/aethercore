export const enPlurals = {
  'unit.file': { one: '{count} file', other: '{count} files' },
  'unit.category': { one: '{count} category', other: '{count} categories' },
  'unit.action': { one: '{count} action', other: '{count} actions' },
  'unit.deviceAction': { one: '{count} device action', other: '{count} device actions' },
  'unit.update': { one: '{count} update', other: '{count} updates' },
  'unit.serviceTarget': { one: '{count} service target', other: '{count} service targets' },
  'unit.offer': { one: '{count} offer', other: '{count} offers' },
  'unit.day': { one: '{count} day', other: '{count} days' },
  'unit.triageCard': { one: '{count} triage card', other: '{count} triage cards' },
  'unit.warning': { one: '{count} warning', other: '{count} warnings' },
} as const;
export type PluralMessageKey = keyof typeof enPlurals;
