export interface SqlEntityBaseClass {
  id: number;
  alias: string;
  qualifiedName: string;
  fields: string[];
  sortOrder: number;
  createdAt: string;
  updatedAt: string;
}

export interface SqlEntityBaseClassDraft {
  alias: string;
  qualifiedName: string;
  fieldsText: string;
}

export interface SqlEntityBaseClassListResponse {
  items: SqlEntityBaseClass[];
}
