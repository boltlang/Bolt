import { SyntaxKind, Syntax } from "./ast";
import { serialize, Json } from "./util";
import { DiagnosticIndex } from "./diagnostics";
import { Test } from "@marktest/core"

export async function scanner(test: Test): Promise<Json> {
    const { DiagnosticIndex } = await import("./diagnostics")
    const diagnostics = new DiagnosticIndex;
    const { Scanner } = await import("./scanner");
    const scanner = new Scanner(test.file, test.text, test.startPos);
    const tokens = []
    while (true) {
        const token = scanner.scan();
        if (token.kind === SyntaxKind.EndOfFile) {
            break;
        }
        tokens.push(token);
    }
    return serialize({
        diagnostics,
        tokens,
    });
}

export async function parser(test: Test): Promise<Json> {
    const kind = test.args.kind ?? 'SourceFile';
    const { Scanner } = await import("./scanner")
    const { Parser } = await import("./parser");
    const diagnostics = new DiagnosticIndex;
    const parser = new Parser();
    const tokens = new Scanner(test.file, test.text);
    let results: Syntax[];
    switch (kind) {
        case 'SourceFile':
            results = [ parser.parseSourceFile(tokens) ];
            break;
        case 'SourceElements':
            results = parser.parseSourceElements(tokens);
            break;
        default:
            throw new Error(`I did not know how to parse ${kind}`)
    }
    return serialize({
        diagnostics,
        results,
    })
}

export function check(test: Test): Json {
    // TODO
}
