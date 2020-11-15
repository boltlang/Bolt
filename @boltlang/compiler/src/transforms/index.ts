
import { Program } from "../program"
import { Container, Newable } from "../ioc"
import {SourceFile} from "../ast";

export interface Transformer {
  isApplicable(node: SourceFile): boolean;
  transform(node: SourceFile): SourceFile;
}

export interface RegisterTransformerOptions {

}

export class TransformManager {

  private transformers: Transformer[] = [];

  constructor(private container: Container) {

  }

  public register(transformerFactory: Newable<Transformer>, options: RegisterTransformerOptions = {}) {
    const transformer = this.container.createInstance(transformerFactory, this) as Transformer;
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

