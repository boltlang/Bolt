
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
