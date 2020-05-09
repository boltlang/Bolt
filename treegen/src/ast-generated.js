
const NODE_TYPES = [
  'StringLiteral': new Map([
    [value, string],
  ]),
  'IntegerLiteral': new Map([
    [value, bigint],
  ]),
  'Identifier': new Map([
    [text, string],
  ]),
  'Operator': new Map([
    [text, string],
  ]),
  'EOS': new Map([
  ]),
  'Comma': new Map([
  ]),
  'Semi': new Map([
  ]),
  'Dot': new Map([
  ]),
  'DotDot': new Map([
  ]),
  'RArrow': new Map([
  ]),
  'LArrow': new Map([
  ]),
  'EqSign': new Map([
  ]),
  'FnKeyword': new Map([
  ]),
  'ForeignKeyword': new Map([
  ]),
  'LetKeyword': new Map([
  ]),
  'ImportKeyword': new Map([
  ]),
  'PubKeyword': new Map([
  ]),
  'ModKeyword': new Map([
  ]),
  'EnumKeyword': new Map([
  ]),
  'StructKeyword': new Map([
  ]),
  'NewTypeKeyword': new Map([
  ]),
  'Parenthesized': new Map([
    [text, string],
  ]),
  'Braced': new Map([
    [text, string],
  ]),
  'Bracketed': new Map([
    [text, string],
  ]),
  'SourceElement': new Map([
  ]),
  'SourceFile': new Map([
    [elements, SourceElement[]],
  ]),
  'QualName': new Map([
    [modulePath, Identifier[]],
    [name, Symbol],
  ]),
  'TypeReference': new Map([
    [name, QualName],
    [arguments, TypeNode[] | null],
  ]),
  'BindPattern': new Map([
    [name, string],
  ]),
  'TypePattern': new Map([
    [typeNode, TypeNode],
    [nestedPattern, Pattern],
  ]),
  'ExpressionPattern': new Map([
    [expression, Expression],
  ]),
  'TuplePatternElement': new Map([
    [index, bigint],
    [pattern, Pattern],
  ]),
  'TuplePattern': new Map([
    [elements, TuplePatternElement[]],
  ]),
  'RecordPatternField': new Map([
    [name, Identifier],
    [pattern, Pattern],
  ]),
  'RecordPattern': new Map([
    [fields, RecordPatternField[]],
  ]),
  'CallExpression': new Map([
    [operator, Expression],
    [operands, Expression[]],
  ]),
  'YieldExpression': new Map([
    [value, Expression],
  ]),
  'MatchArm': new Map([
    [pattern, Pattern],
    [body, Expression],
  ]),
  'MatchExpression': new Map([
    [value, Expression],
    [arms, MatchArm[]],
  ]),
  'Case': new Map([
    [test, Expression],
    [result, Expression],
  ]),
  'CaseExpression': new Map([
    [cases, Case[]],
  ]),
  'BlockExpression': new Map([
    [statements, Statement[]],
  ]),
  'ConstantExpression': new Map([
    [value, Value],
  ]),
  'ReturnStatement': new Map([
    [value, Expression | null],
  ]),
  'ResumeStatement': new Map([
    [value, Expression],
  ]),
  'ExpressionStatement': new Map([
    [expression, Expression],
  ]),
  'Module': new Map([
    [modifiers, Modifiers],
    [name, QualName],
    [elements, SourceElement],
  ]),
  'Parameter': new Map([
    [index, bigint],
    [bindings, Pattern],
    [typeNode, TypeNode | null],
    [defaultValue, Expression | null],
  ]),
  'FunctionDeclaration': new Map([
    [modifiers, Modifiers],
    [name, Symbol],
    [params, Parameter[]],
    [type, TypeNode | null],
    [body, Expression],
  ]),
  'ForeignFunctionDeclaration': new Map([
    [modifiers, Modifiers],
    [name, Symbol],
    [params, Parameter[]],
    [type, TypeNode | null],
    [body, Statement[]],
  ]),
  'VariableDeclaration': new Map([
    [modifiers, Modifiers],
    [name, Symbol],
    [type, TypeNode | null],
    [value, Expression | null],
  ]),
  'PlainImportSymbol': new Map([
    [name, QualName],
  ]),
  'ImportDeclaration': new Map([
    [file, string],
    [symbols, ImportSymbol[]],
  ]),
  'RecordDeclarationField': new Map([
    [name, Identifier],
    [type, TypeNode],
  ]),
  'RecordDeclaration': new Map([
    [name, QualName],
    [fields, RecordDeclarationField[]],
  ]),
];

