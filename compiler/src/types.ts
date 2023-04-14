
export interface Pass<In, Out> {
  apply(input: In): Out;
}

export interface Newable<T> {
  new (...args: any[]): T;
}
