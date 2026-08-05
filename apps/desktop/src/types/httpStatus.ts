export interface HttpStatusResponseHeader {
  name: string;
  description: string;
}

export interface HttpStatusCode {
  code: number;
  name: string;
  desc: string;
  usage: string;
  causes: string;
  explanation: string;
  troubleshooting: string;
  responseHeaders: HttpStatusResponseHeader[];
}

export interface HttpStatusGroup {
  category: string;
  name: string;
  codes: HttpStatusCode[];
}

export interface HttpStatusListResponse {
  groups: HttpStatusGroup[];
}

export interface HttpStatusClassificationHint {
  code: number;
  category: string;
  name: string;
  message: string;
}

export interface HttpStatusLookupResponse {
  results: HttpStatusCode[];
  classificationHint: HttpStatusClassificationHint | null;
}
