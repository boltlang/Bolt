
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
  index?: number;
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
  index?: number;
  type: 'EnumDeclaration';
  name: string;
  fields: EnumField[];
}

export interface TypeDeclaration {
  index?: number;
  type: 'TypeDeclaration';
  name: string;
  typeNode: TypeNode;
}

export type Declaration
  = NodeDeclaration
  | TypeDeclaration
  | EnumDeclaration

export type Syntax
  = Declaration
  | TypeNode
  | NodeField
  | EnumField

export function hasArrayType(typeNode: TypeNode) {
  if (typeNode.type === 'ReferenceTypeNode') {
    return typeNode.name === 'Vec'
  } else if (typeNode.type === 'UnionTypeNode') {
    return typeNode.elements.some(hasArrayType);
  }
}

export function isTypeOptional(typeNode: TypeNode) {
  if (typeNode.type === 'ReferenceTypeNode') {
    return typeNode.name === 'Option';
  } else if (typeNode.type === 'UnionTypeNode') {
    return typeNode.elements.every(isTypeOptional);
  }
}

