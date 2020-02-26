
import { Value, RecordValue } from "./evaluator"

interface Binding {
  name: string;
  createValue: () => Value,
}

export const bindings = new Map<string, Binding>();

export function bind(name: string) {
  return function (target: any) {
    if (bindings.has(name)) {
      throw new Error(`A binding with the name '${name}' already exists.`)
    }
    bindings.set(name, {
      name,
      createValue: (...args) => new RecordValue(target.META_TYPE, new target(...args)),
    });
  }
}

