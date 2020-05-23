
import { SourceFile, Program } from "./program"
import { Container } from "./di"
import {Evaluator} from "./evaluator";
import {TypeChecker} from "./checker";

export interface Transformer {
  isApplicable(node: SourceFile): boolean;
  transform(node: SourceFile): SourceFile;
}

export interface RegisterTransformerOptions {

}

function createInstance<T>(factory: Factory<T>, ...args: any[]): T {
  return new factory(...args);
}

export class TransformManager {

  private transformers: Transformer[] = [];

  constructor(private container: Container) {

  }

  public register(transformerFactory: Newable<Transformer>, options: RegisterTransformerOptions = {}) {
    const transformer = this.container.createInstance(transformerFactory, this);
    this.transformers.push(transformer);
  }

  public apply(program: Program) {
    for (const transformer of this.transformers) {
      for (const sourceFile of program.getAllSourceFiles()) {
        if (transformer.isApplicable(sourceFile)) {
          const newSourceFile = transformer.transform(sourceFile);
          if (newSourceFile !== sourceFile) {
            program.updateSourceFile(sourceFile, newSourceFile);
          }
        }
      }
    }
  }

}

