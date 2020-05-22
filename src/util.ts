
import * as path from "path"
import * as fs from "fs"
import moment from "moment"
import chalk from "chalk"

import { TextSpan, TextPos } from "./text"
import { Scanner } from "./scanner"
import { kindToString, Syntax, BoltQualName, BoltDeclaration, BoltDeclarationModifiers, createEndOfFile, SyntaxKind, isBoltPunctuated } from "./ast"

export function createTokenStream(node: Syntax) {
  if (isBoltPunctuated(node)) {
    const origPos = node.span!.start;
    const startPos = new TextPos(origPos.offset+1, origPos.line, origPos.column+1);
    return new Scanner(node.span!.file, node.text, startPos);
  } else if (node.kind === SyntaxKind.BoltSentence) {
    return new StreamWrapper(
      node.tokens,
      () => createEndOfFile(new TextSpan(node.span!.file, node.span!.end.clone(), node.span!.end.clone()))
    );
  } else {
    throw new Error(`Could not convert ${kindToString(node.kind)} to a token stream.`);
  }
}

export interface JsonArray extends Array<Json> {  };
export interface JsonObject { [key: string]: Json }
export type Json = null | string | boolean | number | JsonArray | JsonObject;

export function uniq<T>(elements: T[]): T[] {
  const out: T[] = [];
  const visited = new Set<T>();
  for (const element of elements) {
    if (visited.has(element)) {
      continue;
    }
    visited.add(element);
    out.push(element);
  }
  return out;
}


export class FastStringMap<K extends PropertyKey, V> {

  private mapping = Object.create(null);

  public get(key: K): V {
    if (!(key in this.mapping)) {
      throw new Error(`No value found for key '${key}'.`);
    }
    return this.mapping[key];
  }

  public set(key: K, value: V): void {
    if (key in this.mapping) {
      throw new Error(`A value for key '${key}' already exists.`);
    }
    this.mapping[key] = value
  }

  public has(key: K): boolean {
    return key in this.mapping;
  }

  public delete(key: K): void {
    if (!(key in this.mapping)) {
      throw new Error(`No value found for key '${key}'.`);
    }
    delete this.mapping[key];
  }

}

class DeepMap {

  private rootMap = new Map<any, any>();

  public has(key: any[]) {
    let curr = this.rootMap;
    for (const element of key) {
      if (!curr.has(element)) {
        return false;
      }
      curr = curr.get(element)!;
    }
    return true;
  }

  public get(key: any[]) {
    let curr = this.rootMap;
    for (const element of key) {
      if (!curr.has(element)) {
        return;
      }
      curr = curr.get(element)!;
    }
    return curr;
  }

  public set(key: any[], value: any) {
    let curr = this.rootMap;
    for (const element of key.slice(0, -1)) {
      if (!curr.has(element)) {
        curr.set(element, new Map());
      }
      curr = curr.get(element)!;
    }
    curr.set(key[key.length-1], value);
  }

}

export function memoize(target: any, key: PropertyKey) {
  const origMethod = target[key];
  target[key] = function wrapper(...args: any[]) {
    if (this.__MEMOIZE_CACHE === undefined) {
      this.__MEMOIZE_CACHE = Object.create(null);
    }
    if (this.__MEMOIZE_CACHE[key] === undefined) {
      this.__MEMOIZE_CACHE[key] = new DeepMap();
    }
    const cache = this.__MEMOIZE_CACHE[key];
    if (cache.has(args)) {
      return cache.get(args);
    }
    const result = origMethod.apply(this, args);
    cache.set(args, result);
    return result;
  }
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
  return (node.modifiers & BoltDeclarationModifiers.Public) > 0;
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

const DATETIME_FORMAT = 'YYYY-MM-DD HH:mm:ss'

export function verbose(message: string) {
  console.error(chalk.gray('[') + chalk.magenta('verb') + ' ' + chalk.gray(moment().format(DATETIME_FORMAT) + ']') + ' ' + message);
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

