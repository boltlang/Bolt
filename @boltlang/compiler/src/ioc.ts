
export type Newable<T> = {
  new(...args: any): T;
}

export type Factory<T> = Newable<T>;

export type ServiceID = string | symbol | Factory<any>;

function isFactory(value: any): boolean {
  return typeof value === 'object'
      && value !== null
      && value instanceof Function
}

export function inject(target: any, key: PropertyKey, index: number) {
  if (!Reflect.hasMetadata('di:paramindices', target)) {
    Reflect.defineMetadata('di:paramindices', [], target)
  }
  const indices = Reflect.getMetadata('di:paramindices', target);
  indices.push(index);
}

function describeFactory(factory: Factory<any>): string {
  return factory.name;
}

export class Container {

  private factories: Factory<any>[] = [];
  private singletons = new Map<ServiceID, object>();

  public bindSelf<T extends object>(value: Factory<T> | T): void {
    if (isFactory(value)) {
      this.factories.push(value as Factory<T>);
    } else {
      this.singletons.set((value as T).constructor as ServiceID, value)
    }
  }

  public resolve<T>(factory: Factory<T>): T;
  public resolve(serviceId: ServiceID): any {
    return this.singletons.get(serviceId);
  }

  public createInstance<T>(factory: Factory<T>, ...args: any[]): T {
    const newArgs: any[] = [];
    const paramTypes = Reflect.getMetadata('design:paramtypes', factory);
    const indices = Reflect.getMetadata('di:paramindices', factory);
    if (paramTypes === undefined) {
      return new factory(...args);
    }
    let i = 0;
    for (const paramType of paramTypes) {
      if (indices.indexOf(i) !== -1) {
        newArgs.push(this.resolve(paramType));
      } else {
        if (i >= args.length) {
          throw new Error(`Too few arguments provided to constructor of '${describeFactory(factory)}'.`);
        }
        newArgs.push(args[i++]);
      }
    }
    if (i < args.length) {
      throw new Error(`Too many arguments provided to factory function '${describeFactory(factory)}'.`);
    }
    return new factory(...newArgs);
  }

}


