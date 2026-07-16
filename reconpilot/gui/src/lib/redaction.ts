const REDACTION_PATTERNS: Array<[RegExp, string]> = [
  [/(Authorization\s*:\s*Bearer\s+)[A-Za-z0-9\-._~+/=]+/gi, "$1[REDACTED]"],
  [/(Bearer\s+)[A-Za-z0-9\-._~+/=]+/gi, "$1[REDACTED]"],
  [/eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9._-]{10,}\.[A-Za-z0-9._-]{10,}/g, "[REDACTED-JWT]"],
  [/\b(?:AKIA|ASIA|AIza|sk_live_|sk_test_)[A-Za-z0-9_\-]{8,}\b/g, "[REDACTED-API-KEY]"],
  [/\b[a-f0-9]{32,}\b/gi, "[REDACTED-HEX]"],
  [/\b[A-Za-z0-9+/=]{40,}\b/g, "[REDACTED-BLOB]"]
];

export function redactText(value: string, enabled: boolean): string {
  if (!enabled || !value) {
    return value;
  }

  return REDACTION_PATTERNS.reduce((output, [pattern, replacement]) => {
    return output.replace(pattern, replacement);
  }, value);
}

export function redactUnknown(value: unknown, enabled: boolean): string {
  if (value === null || value === undefined) {
    return "";
  }

  if (typeof value === "string") {
    return redactText(value, enabled);
  }

  return redactText(JSON.stringify(value, null, 2), enabled);
}
