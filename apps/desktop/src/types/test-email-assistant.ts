export interface TestEmailAssistantInspectResult {
  templatePath: string;
  placeholders: string[];
}

export interface TestEmailAssistantGenerateResult {
  outputPath: string;
  fileName: string;
}
