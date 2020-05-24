
import * as path from "path"
import * as fs from "fs"
import moment from "moment"
import chalk from "chalk"

export function assert(test: boolean): void {
  if (!test) {
    throw new Error(`Invariant violation: an internal sanity check failed.`);
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

  public *values(): IterableIterator<V> {
    for (const key of Object.keys(this.mapping)) {
      yield this.mapping[key];
    }
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

export function memoize(hasher: (...args: any[]) => string) {
  return function (target: any, key: PropertyKey) {
    const origMethod = target[key];
    target[key] = function wrapper(...args: any[]) {
      if (this.__MEMOIZE_CACHE === undefined) {
        this.__MEMOIZE_CACHE = Object.create(null);
      }
      if (this.__MEMOIZE_CACHE[key] === undefined) {
        this.__MEMOIZE_CACHE[key] = Object.create(null);
      }
      const hashed = hasher(...args);
      const cache = this.__MEMOIZE_CACHE[key];
      if (hashed in cache) {
        return cache[hashed];
      }
      const result = origMethod.apply(this, args);
      cache[hashed] = result;
      return result;
    }
    return target;
  }
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

export function enumerate(elements: string[]) {
  if (elements.length === 1) {
    return elements[0]
  } else {
    return elements.slice(0, elements.length-1).join(', ') + ' or ' + elements[elements.length-1]
  }
}

export function escapeChar(ch: string) {
  switch (ch) {
    case '\a': return '\\a';
    case '\b': return '\\b';
    case '\f': return '\\f';
    case '\n': return '\\n';
    case '\r': return '\\r';
    case '\t': return '\\t';
    case '\v': return '\\v';
    case '\0': return '\\0';
    case '\'': return '\\\'';
    default:
      const code = ch.charCodeAt(0);
      if (code >= 0x20 && code <= 0x7E)  {
        return ch
      } else if (code < 0x7F) {
        return `\\x${code.toString(16).padStart(2, '0')}`
      } else {
        return `\\u${code.toString(16).padStart(4, '0')}`
      }
  }
}

export interface MapLike<T> {
  [key: string]: T;
}

export type FormatArg = string | Date | number

export function format(message: string, data: MapLike<FormatArg>) {

  let out = ''

  let name = '';
  let insideParam = false;

  for (const ch of message) {
    if (insideParam) {
      if (ch === '}') {
        out += data[name]!.toString();
        reset();
      } else {
        name += ch;
      }
    } else {
      if (ch === '{') {
        insideParam = true;
      } else {
        out += ch;
      }
    }
  }

  return out;

  function reset() {
    name = '';
    insideParam = false;
  }

}

