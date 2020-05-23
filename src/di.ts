
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
  //return function(target: any) {
    // This is a no-op because adding the decorator
    // is enough for TypeScript to emit metadata
  //}
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

  private canResolve(serviceId: ServiceID): boolean {
    return this.singletons.has(serviceId);
  }

  private resolve<T>(factory: Factory<T>): T;
  private resolve(serviceId: ServiceID): any {
    return this.singletons.get(serviceId);
  }

  public createInstance<T>(factory: Factory<T>, ...args: any[]): T {
    const newArgs: any[] = [];
    const paramTypes = Reflect.getMetadata('design:paramtypes', factory);
    if (paramTypes === undefined) {
      return new factory(...args);
    }
    let i = 0;
    for (const paramType of paramTypes) {
      if (this.canResolve(paramType)) {
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


