
export function hasOwnProperty<O extends object>(obj: O, property: keyof O): boolean {
  return Object.prototype.hasOwnProperty.call(obj, property);
}

export function assert(test: boolean): asserts test {
  if (!test) {
    throw new Error(`Invariant violation: a run-time check failed to hold. See the stack trace for more information.`);
  }
}

export interface Stream<T> {
  get(): T;
  peek(offset?: number): T;
}

export class CustomSet<T extends object, K extends keyof T> {

  private mapping = new Map<K, T>();

  public constructor(private key: K) {

  }

  public add(element: T): void {
    // @ts-ignore this.key is to general to be used as a property
    this.mapping.set(element[this.key], element);
  }

  public has(element: T): boolean {
    // @ts-ignore this.key is to general to be used as a property
    return this.mapping.has(element[this.key]);
  }

  public [Symbol.iterator]() {
    return this.mapping.values();
  }

  public delete(element: T): void {
    // @ts-ignore this.key is to general to be used as a property
    this.mapping.delete(element[this.key]);
  }

  public clear(): void {
    this.mapping.clear();
  }

}

export class CustomMap<T1 extends object, T2, K extends keyof T1> {

  private mapping = new Map<K, T2>();

  public constructor(private property: K) {

  }

  public set(key: T1, value: T2): void {
    // @ts-ignore this.property is to general to be used as a property
    this.mapping.set(key[this.property], value);
  }

  public get(key: T1): T2 | undefined {
    // @ts-ignore this.property is to general to be used as a property
    return this.mapping.get(key[this.property]);
  }

  public has(key: T1): boolean {
    // @ts-ignore this.property is to general to be used as a property
    return this.mapping.has(key[this.property]);
  }

  public [Symbol.iterator]() {
    return this.mapping.values();
  }

  public delete(key: T1): void {
    // @ts-ignore this.property is to general to be used as a property
    this.mapping.delete(key[this.property]);
  }

  public clear(): void {
    this.mapping.clear();
  }

}

export abstract class BufferedStream<T> implements Stream<T> {

  private buffer: Array<T> = [];

  protected abstract read(): T;

  public get(): T {
    if (this.buffer.length > 0) {
      return this.buffer.shift()!;
    }
    return this.read()
  }

  public peek(offset = 1): T {
    while (this.buffer.length < offset) {
      const result = this.read();
      this.buffer.push(result);
    }
    return this.buffer[offset - 1];
  }

}

export function countDigits(x: number, base: number = 10) {
  return x === 0
       ? 1 : Math.ceil(Math.log(x+1) / Math.log(base))
}

export enum CompareMode {
  None           = 0,
  Greater        = 1,
  Lesser         = 2,
  Equal          = 4,
  GreaterOrEqual = Greater | Equal,
  LesserOrEqual  = Lesser | Equal,
}
