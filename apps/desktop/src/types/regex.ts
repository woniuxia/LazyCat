export interface RegexTemplate {
  id: string;
  name: string;
  category: string;
  description: string;
  expression: string;
  example_input: string;
  example_match: string;
}

export interface RegexCaptureGroup {
  index: number;
  name: string | null;
  value: string | null;
  start: number | null;
  end: number | null;
}

export interface RegexMatchResult {
  index: number;
  end: number;
  match: string;
  groups: RegexCaptureGroup[];
}
