import { describe, expect, it } from "vitest";

import { redactText } from "./redaction";

describe("redaction", () => {
  it("redacts authorization headers and bearer tokens", () => {
    const input = "Authorization: Bearer secret-token-value";
    const output = redactText(input, true);
    expect(output).toContain("Authorization: Bearer [REDACTED]");
    expect(output).not.toContain("secret-token-value");
  });

  it("redacts jwt-like strings", () => {
    const input =
      "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.signaturevalue";
    const output = redactText(input, true);
    expect(output).toContain("[REDACTED-JWT]");
  });

  it("returns original text when redaction is disabled", () => {
    const input = "Authorization: Bearer keep-me";
    expect(redactText(input, false)).toBe(input);
  });
});
