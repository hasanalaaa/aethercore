/**
 * Phase 12 bidi safety helpers.
 *
 * AetherCore-owned prose is localized normally. Technical evidence must keep
 * byte-for-byte visual order inside RTL prose, so we identify only strongly
 * technical token shapes and render those tokens through TechnicalText.
 * This is presentation-only: the underlying evidence string is never changed.
 */
export type BidiSegment = Readonly<{ text: string; technical: boolean }>;

const TECHNICAL_TOKEN = new RegExp([
  // Windows drive paths, extended paths and UNC paths.
  String.raw`(?:\\\\\?\\)?[A-Za-z]:\\[^\s؛،,<>"']+`,
  String.raw`\\\\[^\s؛،,<>"']+`,
  String.raw`%[A-Za-z0-9_]+%\\[^\s؛،,<>"']+`,
  // GUIDs / braced identifiers.
  String.raw`\{?[0-9A-Fa-f]{8}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{12}\}?`,
  // Hex addresses / bugchecks / HRESULT-shaped values.
  String.raw`0x[0-9A-Fa-f]+`,
  // Long cryptographic-looking hexadecimal digests.
  String.raw`\b[0-9A-Fa-f]{32,128}\b`,
  // Driver/firmware version tuples (avoid ordinary decimal numbers).
  String.raw`\b\d+(?:\.\d+){2,5}\b`,
  // Driver package names and dump filenames when embedded in prose.
  String.raw`\b[^\s؛،,<>"']+\.(?:inf|sys|dmp|mdmp)\b`,
].join('|'), 'g');

export function segmentBidiEvidence(value: string): BidiSegment[] {
  if (!value) return [];
  const result: BidiSegment[] = [];
  let cursor = 0;
  TECHNICAL_TOKEN.lastIndex = 0;
  for (let match = TECHNICAL_TOKEN.exec(value); match; match = TECHNICAL_TOKEN.exec(value)) {
    if (match.index > cursor) result.push({ text: value.slice(cursor, match.index), technical: false });
    result.push({ text: match[0], technical: true });
    cursor = match.index + match[0].length;
  }
  if (cursor < value.length) result.push({ text: value.slice(cursor), technical: false });
  return result.length ? result : [{ text: value, technical: false }];
}
