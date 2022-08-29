
export function assert(test: boolean): asserts test {
  if (!test) {
    throw new Error(`Assertion failed. See the stack trace for more information.`);
  }
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

