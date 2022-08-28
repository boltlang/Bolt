
export class UnexpectedCharDiagnostic {

  public constructor(
    public text: string,
    public offset: number,
    public actual: string,
  ) {

  }

  public format(): string {
    let out = `error: unexpeced character '${this.actual}'.`;
    return out;
  }

}

export type Diagnostic
  = UnexpectedCharDiagnostic;

export class Diagnostics {

  private savedDiagnostics: Diagnostic[] = [];

  public add(diagnostic: Diagnostic): void {
    this.savedDiagnostics.push(diagnostic);
    process.stderr.write(diagnostic.format());
  }

}

