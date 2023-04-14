
import "reflect-metadata"
import path from "path"
import stream from "stream"
import { InspectOptions } from "util";

export const isDebug = process.env['NODE_ENV'] === 'development';

export const toStringTag = Symbol.for('nodejs.util.inspect.custom');

export type InspectFn = (value: any, options: InspectOptions) => string;

export function isIterable(value: any): value is Iterable<any> {
  if (value === undefined || value === null) {
    return false;
  }
  return typeof(value[Symbol.iterator]) === 'function';
}

export function first<T>(iter: Iterator<T>): T | undefined {
  return iter.next().value;
}

export function last<T>(iter: Iterator<T>): T | undefined {
  let prevValue;
  for (;;) {
    const { done, value } = iter.next();
    if (done) {
      return prevValue;
    }
    prevValue = value;
  }
}

export function stripExtension(filepath: string): string {
  const basename = path.basename(filepath);
  const i = basename.lastIndexOf('.');
  if (i === -1) {
    return filepath;
  }
  return path.join(path.dirname(filepath), basename.substring(0, i));
}

export class IndentWriter {

  private atBlankLine = true;
  private indentLevel = 0;

  public constructor(
    private output: stream.Writable,
    private indentation = '  ',
  ) {
    
  }

  public write(text: string): void {
    for (const ch of text) {
      if (ch === '\n') {
        this.atBlankLine = true;
      } else if (!/[\t ]/.test(ch) && this.atBlankLine) {
        this.output.write(this.indentation.repeat(this.indentLevel));
        this.atBlankLine = false;
      }
      this.output.write(ch);
    }
  }

  public indent(): void {
    this.indentLevel++;
  }

  public dedent(): void {
    this.indentLevel--;
  }

}

const GITHUB_ISSUE_URL = 'https://github.com/boltlang/bolt/issues/'

export function assert(test: boolean): asserts test {
  if (!test) {
    throw new Error(`Assertion failed. See the stack trace for more information. You are invited to search this issue on GitHub or to create a new one at ${GITHUB_ISSUE_URL} .`);
  }
}

export function implementationLimitation(test: boolean): asserts test {
  if (!test) {
    throw new Error(`We encountered a limitation to the implementation of this compiler. You are invited to search this issue on GitHub or to create a new one at ${GITHUB_ISSUE_URL} .`);
  }
}

export function unreachable(): never {
  throw new Error(`Code that should never be executed was reached during operation.`);
}

export function assertNever(value: never): never {
  console.error(value);
  throw new Error(`Assertion failed. See the stack trace for more information.`);
}

export function countDigits(x: number, base: number = 10) {
  return x === 0 ? 1 : Math.ceil(Math.log(x+1) / Math.log(base))
}

export function isEmpty<T>(iter: Iterable<T> | Iterator<T>): boolean {
  if ((iter as any)[Symbol.iterator] !== undefined) {
    iter = (iter as any)[Symbol.iterator]();
  }
  return !!(iter as Iterator<T>).next().done;
}

export type JSONValue = null | boolean | number | string | JSONArray | JSONObject
export type JSONArray = Array<JSONValue>;
export type JSONObject = { [key: string]: JSONValue };

export class MultiDict<K, V> {

  private mapping = new Map<K, V[]>();

  public constructor(iterable?: Iterable<[K, V]>) {
    if (iterable) {
      for (const [key, value] of iterable) {
        this.add(key, value);
      }
    }
  }

  public get(key: K): Iterable<V> {
    return this.mapping.get(key) ?? [];
  }

  public add(key: K, value: V): void {
    const values = this.mapping.get(key);
    if (values) {
      values.push(value);
    } else {
      this.mapping.set(key, [ value ])
    }
  }

  public *[Symbol.iterator](): Iterator<[K, V]> {
    for (const [key, values] of this.mapping) {
      for (const value of values) {
        yield [key, value];
      }
    }
  }

}

export interface Stream<T> {
  get(): T;
  peek(offset?: number): T;
}

export abstract class BufferedStream<T> {

  private buffer: Array<T> = [];

  public abstract read(): T;

  public get(): T {
    if (this.buffer.length > 0) {
      return this.buffer.shift()!;
    }
    return this.read();
  }

  public peek(offset = 1): T {
    while (this.buffer.length < offset) {
      this.buffer.push(this.read());
    }
    return this.buffer[offset-1];
  }

}

export class MultiMap<K, V> {

  private mapping = new Map<K, V[]>();

  public get(key: K): V[] {
    return this.mapping.get(key) ?? [];
  }

  public add(key: K, value: V): void {
    let elements = this.mapping.get(key);
    if (elements === undefined) {
      elements = [];
      this.mapping.set(key, elements);
    }
    elements.push(value);
  }

  public has(key: K, value?: V): boolean {
    if (value === undefined) {
      return this.mapping.has(key);
    }
    const elements = this.mapping.get(key);
    if (elements === undefined) {
      return false;
    }
    return elements.indexOf(value) !== -1;
  }

  public keys(): Iterable<K> {
    return this.mapping.keys();
  }

  public *values(): Iterable<V> {
    for (const elements of this.mapping.values()) {
      yield* elements;
    }
  }

  public *[Symbol.iterator](): Iterator<[K, V]> {
    for (const [key, elements] of this.mapping) {
      for (const value of elements) {
        yield [key, value];
      }
    }
  }

  public delete(key: K, value?: V): number {
    const elements = this.mapping.get(key);
    if (elements === undefined) {
      return 0;
    }
    if (value === undefined) {
      this.mapping.delete(key);
      return elements.length;
    }
    const i = elements.indexOf(value);
    if (i !== -1) {
      elements.splice(i, 1);
      if (elements.length === 0) {
        this.mapping.delete(key);
      }
      return 1;
    }
    return 0;
  }

}

export const classes = new Map<string, any>;

export function deserializable() {
  return (constructor: Function) => {
    if (classes.has(constructor.name)) {
      throw new Error(`A class with the name '${constructor.name}' has already been registered.`);
    }
    classes.set(constructor.name, constructor);
  }
}

export function getIgnoredFields(target: any): Set<string> {
  const fields = Reflect.getOwnMetadata('ignoredFields', target) ?? new Set;
  for (;;) {
    target = Object.getPrototypeOf(target);
    if (target === null) {
      break;
    }
    const otherFields = Reflect.getOwnMetadata('ignoredFields', target);
    if (otherFields !== undefined) {
      for (const field of otherFields) {
        fields.add(field);
      }
    }
  }
  return fields;
}

export function ignore(target: any, propertyKey: string) {
  if (!Reflect.hasOwnMetadata('ignoredFields', target.constructor)) {
    Reflect.defineMetadata('ignoredFields', new Set([ propertyKey ]), target.constructor);
  }
  Reflect.getOwnMetadata('ignoredFields', target.constructor).add(propertyKey);
}

export const nonenumerable: {
    (target: any, name: string): void;
    (target: any, name: string, desc: PropertyDescriptor): PropertyDescriptor;
} = (target: any, name: string, desc?: any) => {
    if (desc) {
        desc.enumerable = false;
        return desc;
    }
    Object.defineProperty(target, name,  {
        set(value) {
            Object.defineProperty(this, name, {
                value, writable: true, configurable: true,
            });
        },
        configurable: true,
    });
};

