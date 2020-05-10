
import * as path from "path"
import * as fs from "fs"

import { TextSpan } from "./text"
import { kindToString, Syntax, BoltToken, BoltQualName, BoltDeclaration, BoltDeclarationModifiers } from "./ast"

export type BoltTokenStream = Stream<BoltToken>;

export interface JsonArray extends Array<Json> {  };
export interface JsonObject { [key: string]: Json }
export type Json = null | string | boolean | number | JsonArray | JsonObject;

export interface FastStringMap<T> {
  [key: string]: T
}

const supportedLanguages = ['Bolt', 'JS'];

export function getLanguage(node: Syntax): string {
  const kindStr = kindToString(node.kind);
  for (const prefix of supportedLanguages) {
    if (kindStr.startsWith(prefix)) {
      return prefix;
    }
  }
  throw new Error(`Could not determine the language of ${kindStr}`);
}

export function cloneSpan(span: TextSpan | null) {
  if (span === null) {
    return null;
  }
  return span.clone();
}

export function setOrigNodeRange(node: Syntax, startNode: Syntax, endNode: Syntax): void {
  node.span = new TextSpan(startNode.span!.file, startNode.span!.start.clone(), endNode.span!.end.clone());
}

export function hasPublicModifier(node: BoltDeclaration) {
  return (node.modifiers & BoltDeclarationModifiers.IsPublic) > 0;
}

export function getFullTextOfQualName(node: BoltQualName) {
  let out = ''
  for (const element of node.modulePath) {
    out += element.text + '.';
  }
  return out + node.name.text;
}

export interface Stream<T> {
  get(): T;
  peek(count?: number): T;
}

export class StreamWrapper<T> {

  offset = 0

  constructor(protected data: T[], protected createSentry: () => T) {

  }

  peek(count = 1) {
    const offset = this.offset + (count - 1);
    if (offset >= this.data.length) {
      return this.createSentry();
    }
    return this.data[offset];
  }

  get() {
    if (this.offset >= this.data.length) {
      return this.createSentry();
    }
    return this.data[this.offset++];
  }

}

export function upsearchSync(filename: string, startDir = '.') {
  let currDir = startDir;
  while (true) {
    const filePath = path.join(currDir, filename);
    if (fs.existsSync(filePath)) {
      return filePath
    }
    const  { root, dir } = path.parse(currDir);
    if (dir === root) {
      return null;
    }
    currDir = dir;
  }
}

export function getFileStem(filepath: string): string {
  return path.basename(filepath).split('.')[0];
}

