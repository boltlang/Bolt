
export interface ReferenceTypeNode {
  type: 'ReferenceTypeNode';
  name: string;
  typeArgs: TypeNode[];
}

export interface UnionTypeNode {
  type: 'UnionTypeNode';
  elements: TypeNode[];
}

export type TypeNode
  = ReferenceTypeNode
  | UnionTypeNode

export interface NodeField {
  name: string;
  typeNode: TypeNode;
}

export interface NodeDeclaration {
  type: 'NodeDeclaration';
  name: string;
  parents: string[];
  fields: NodeField[];
}

export interface EnumField {
  name: string;
  value: number | null;
}

export interface EnumDeclaration {
  type: 'EnumDeclaration';
  name: string;
  fields: EnumField[];
}

export interface TypeDeclaration {
  type: 'TypeDeclaration';
  name: string;
  typeNode: TypeNode;
}

export type Declaration
  = NodeDeclaration
  | TypeDeclaration
  | EnumDeclaration

