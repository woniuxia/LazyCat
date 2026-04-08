export type Base64DetectedKind = "standard" | "url-safe" | "ambiguous" | "none";
export type Base64ResolvedKind = "standard" | "url-safe";
export type Base64ManualChoice = Base64ResolvedKind | null;

interface ResolveBase64DecodeKindOptions {
  detectedKind: Base64DetectedKind;
  manualChoice: Base64ManualChoice;
  currentKind: Base64ResolvedKind;
}

const BASE64_INPUT_RE = /^[A-Za-z0-9+/=_-]+$/;
const WHITESPACE_RE = /\s/;
const STANDARD_EXCLUSIVE_RE = /[+/]/;
const URL_SAFE_EXCLUSIVE_RE = /[-_]/;

export function detectBase64Kind(input: string): Base64DetectedKind {
  if (input.trim() === "") {
    return "none";
  }

  if (WHITESPACE_RE.test(input)) {
    return "none";
  }

  if (!BASE64_INPUT_RE.test(input)) {
    return "none";
  }

  const hasStandardExclusive = STANDARD_EXCLUSIVE_RE.test(input);
  const hasUrlSafeExclusive = URL_SAFE_EXCLUSIVE_RE.test(input);

  if (hasStandardExclusive && hasUrlSafeExclusive) {
    return "none";
  }

  if (hasUrlSafeExclusive) {
    return isValidBase64ForKind(input, "url-safe") ? "url-safe" : "none";
  }

  if (hasStandardExclusive) {
    return isValidBase64ForKind(input, "standard") ? "standard" : "none";
  }

  if (input.includes("=")) {
    return isValidBase64ForKind(input, "standard") ? "standard" : "none";
  }

  const standardValid = isValidBase64ForKind(input, "standard");
  const urlSafeValid = isValidBase64ForKind(input, "url-safe");

  if (standardValid && urlSafeValid) {
    return "ambiguous";
  }

  if (standardValid) {
    return "standard";
  }

  if (urlSafeValid) {
    return "url-safe";
  }

  return "none";
}

export function resolveBase64DecodeKind(
  options: ResolveBase64DecodeKindOptions
): Base64ResolvedKind {
  const { detectedKind, manualChoice, currentKind } = options;

  if (detectedKind === "standard" || detectedKind === "url-safe") {
    return detectedKind;
  }

  if (detectedKind === "ambiguous") {
    return manualChoice ?? "standard";
  }

  return currentKind;
}

function isValidBase64ForKind(input: string, kind: Base64ResolvedKind): boolean {
  if (!input) {
    return false;
  }

  const allowPadding = kind === "standard";
  const paddingStart = input.indexOf("=");
  let payload = input;
  let paddingCount = 0;

  if (paddingStart !== -1) {
    if (!allowPadding) {
      return false;
    }

    payload = input.slice(0, paddingStart);
    paddingCount = input.length - paddingStart;

    if (!/^=+$/.test(input.slice(paddingStart))) {
      return false;
    }

    if (paddingCount > 2) {
      return false;
    }

    const remainder = payload.length % 4;
    if (remainder < 2) {
      return false;
    }

    if ((remainder === 2 && paddingCount !== 2) || (remainder === 3 && paddingCount !== 1)) {
      return false;
    }
  }

  const remainder = payload.length % 4;

  if (remainder === 1) {
    return false;
  }

  if (allowPadding) {
    if (paddingCount === 0 && input.length % 4 !== 0) {
      return false;
    }
  } else if (paddingCount > 0) {
    return false;
  }

  for (const char of payload) {
    if (getAlphabetValue(char, kind) === null) {
      return false;
    }
  }

  return hasCanonicalTrailingBits(payload, kind);
}

function hasCanonicalTrailingBits(input: string, kind: Base64ResolvedKind): boolean {
  const remainder = input.length % 4;

  if (remainder === 0) {
    return true;
  }

  const lastValue = getAlphabetValue(input[input.length - 1], kind);
  if (lastValue === null) {
    return false;
  }

  if (remainder === 2) {
    return (lastValue & 0b0000_1111) === 0;
  }

  if (remainder === 3) {
    return (lastValue & 0b0000_0011) === 0;
  }

  return false;
}

function getAlphabetValue(char: string, kind: Base64ResolvedKind): number | null {
  const code = char.charCodeAt(0);

  if (code >= 65 && code <= 90) {
    return code - 65;
  }

  if (code >= 97 && code <= 122) {
    return code - 97 + 26;
  }

  if (code >= 48 && code <= 57) {
    return code - 48 + 52;
  }

  if (kind === "standard") {
    if (char === "+") {
      return 62;
    }

    if (char === "/") {
      return 63;
    }
  } else {
    if (char === "-") {
      return 62;
    }

    if (char === "_") {
      return 63;
    }
  }

  return null;
}
