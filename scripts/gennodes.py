#!/usr/bin/env python3

import re
from collections import deque
from pathlib import Path
import argparse
from typing import List, Optional, final
from sweetener.record import Record, warn

EOF = '\uFFFF'

END_OF_FILE      = 0
IDENTIFIER       = 1
SEMI             = 2
EXTERNAL_KEYWORD = 3
NODE_KEYWORD     = 4
LBRACE           = 5
RBRACE           = 6
LESSTHAN         = 7
GREATERTHAN      = 8
COLON            = 9
LPAREN           = 10
RPAREN           = 11
VBAR             = 12
COMMA            = 13
HASH             = 14
STRING           = 15
CONST_KEYWORD    = 16
NOEXCEPT_KEYWORD = 17

_re_whitespace = re.compile(r"[\n\r\t ]")
_re_ident_start = re.compile(r"[a-zA-Z_]")
_re_ident_part  = re.compile(r"[a-zA-Z_0-9]")

_keywords = {
    'external': EXTERNAL_KEYWORD,
    'node': NODE_KEYWORD,
    'const': CONST_KEYWORD,
    'noexcept': NOEXCEPT_KEYWORD,
    }

def to_upper_camel_case(snake_str):
    return "".join(x.capitalize() for x in snake_str.lower().split("_"))

def escape_char(ch):
    code = ord(ch)
    if code >= 32 and code < 126:
        return ch
    if code <= 127:
        return f"\\x{code:02X}"
    return f"\\u{code:04X}"

class ScanError(RuntimeError):

    def __init__(self, file, position, actual):
        super().__init__(f"{file.name}:{position.line}:{position.column}: unexpected character '{escape_char(actual)}'")
        self.file = file
        self.position = position
        self.actual = actual

_token_type_to_string = {
    LPAREN: '(',
    RPAREN: ')',
    LBRACE: '{',
    RBRACE: '}',
    LESSTHAN: '<',
    GREATERTHAN: '>',
    SEMI: ';',
    COLON: ':',
    COMMA: ',',
    VBAR: '|',
    HASH: '#',
    }

class Token:

    def __init__(self, type, position=None, value=None):
        self.type = type
        self.start_pos = position
        self.value = value

    @property
    def text(self):
        if self.type in _token_type_to_string:
            return _token_type_to_string[self.type]
        if self.type == IDENTIFIER:
            return self.value
        if self.type == STRING:
            return f'"{self.value}"'
        if self.type == END_OF_FILE:
            return ''
        return '(unknown token)'

class TextFile:

    def __init__(self, filename, text=None):
        self.name = filename
        self._cached_text = text

    @property
    def text(self):
        if self._cached_text is None:
            with open(self.name, 'r') as f:
                self._cached_text = f.read()
        return self._cached_text

class TextPos:

    def __init__(self, line=1, column=1):
        self.line = line
        self.column = column

    def clone(self):
        return TextPos(self.line, self.column)

    def advance(self, text):
        for ch in text:
            if ch == '\n':
                self.line += 1
                self.column = 1
            else:
                self.column += 1

class Scanner:

    def __init__(self, text, text_offset=0, filename=None):
        self._text = text
        self._text_offset = text_offset
        self.file = TextFile(filename, text)
        self._curr_pos = TextPos()

    def _peek_char(self, offset=1):
        i = self._text_offset + offset - 1
        return self._text[i] if i < len(self._text) else EOF

    def _get_char(self):
        if self._text_offset == len(self._text):
            return EOF
        i = self._text_offset
        self._text_offset += 1
        ch = self._text[i]
        self._curr_pos.advance(ch)
        return ch

    def _take_while(self, pred):
        out = ''
        while True:
            ch = self._peek_char()
            if not pred(ch):
                break
            self._get_char()
            out += ch
        return out

    def scan(self):

        while True:
            start_pos = self._curr_pos.clone()
            c0 = self._get_char()
            c1 = self._peek_char()
            if _re_whitespace.match(c0):
                continue
            if c0 == '/' and c1 == '/':
                while True:
                    c2 = self._get_char()
                    if c2 == EOF or c2 == '\n':
                        break
                continue
            break

        if c0 == EOF:
            return Token(END_OF_FILE, self._curr_pos.clone())

        if c0 == ';': return Token(SEMI, start_pos)
        if c0 == '{': return Token(LBRACE, start_pos)
        if c0 == '}': return Token(RBRACE, start_pos)
        if c0 == '(': return Token(LPAREN, start_pos)
        if c0 == ')': return Token(RPAREN, start_pos)
        if c0 == '<': return Token(LESSTHAN, start_pos)
        if c0 == '>': return Token(GREATERTHAN, start_pos)
        if c0 == ':': return Token(COLON, start_pos)
        if c0 == '|': return Token(VBAR, start_pos)
        if c0 == ',': return Token(COMMA, start_pos)
        if c0 == '#': return Token(HASH, start_pos)

        if c0 == '"':
            text = ''
            while True:
                c1 = self._get_char()
                if c1 == '"':
                    break
                text += c1
            return Token(STRING, start_pos, text)

        if _re_ident_start.match(c0):
            name = c0 + self._take_while(lambda ch: _re_ident_part.match(ch))
            return Token(_keywords[name], start_pos) \
                if name in _keywords \
                else Token(IDENTIFIER, start_pos, name)

        raise ScanError(self.file, start_pos, c0)

class Type(Record):
    pass

class ListType(Type):
    element_type: Type

class OptionalType(Type):
    element_type: Type

class NodeType(Type):
    name: str

class VariantType(Type):
    types: List[Type]

class RawType(Type):
    text: str

class AST(Record):
    pass

class Directive(AST):
    pass

INCLUDEMODE_LOCAL  = 0
INCLUDEMODE_SYSTEM = 1

class IncludeDiretive(Directive):
    path: str
    mode: int

    def __str__(self):
        if self.mode == INCLUDEMODE_LOCAL:
            return f"#include \"{self.path}\"\n"
        if self.mode == INCLUDEMODE_SYSTEM:
            return f"#include <{self.path}>\n"

class TypeExpr(AST):

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.type = None

class RefTypeExpr(TypeExpr):
    name: str
    args: List[TypeExpr]

class UnionTypeExpr(TypeExpr):
    types: List[TypeExpr]

class External(AST):
    name: str

class NodeDeclField(AST):
    name: str
    type_expr: TypeExpr

class NodeDecl(AST):
    name: str
    parents: List[str]
    members: List[NodeDeclField]

class Expr(AST):
    pass

class BlockElement(AST):
    pass

class RetStmt(BlockElement):
    value: Expr

class Param(AST):
    type_expr: TypeExpr
    name: Optional[Token]

def pretty_token(token):
    if token.type == END_OF_FILE:
        return 'end-of-file'
    return f"'{token.text}'"

def pretty_token_type(token_type):
    if token_type in _token_type_to_string:
        return f"'{_token_type_to_string[token_type]}'"
    if token_type == IDENTIFIER:
        return 'an identfier'
    if token_type == STRING:
        return 'a string literal'
    if token_type == END_OF_FILE:
        return 'end-of-file'
    return f"(unknown token type {token_type})"

def pretty_alternatives(elements):
    try:
        out = next(elements)
    except StopIteration:
        return 'nothing'
    try:
        prev_element = next(elements)
    except StopIteration:
        return out
    while True:
        try:
            element = next(elements)
        except StopIteration:
            break
        out += ', ' + prev_element
        prev_element = element
    return out + ' or ' + prev_element

class ParseError(RuntimeError):

    def __init__(self, file, actual, expected):
        super().__init__(f"{file.name}:{actual.start_pos.line}:{actual.start_pos.column}: got {pretty_token(actual)} but expected {pretty_alternatives(pretty_token_type(tt) for tt in expected)}")
        self.actual = actual
        self.expected = expected

class Parser:

    def __init__(self, scanner: Scanner):
        self._scanner = scanner
        self._token_buffer = deque[Token]()

    def _peek_token(self, offset=1):
        while len(self._token_buffer) < offset:
            self._token_buffer.append(self._scanner.scan())
        return self._token_buffer[offset-1]

    def _get_token(self):
        if self._token_buffer:
            return self._token_buffer.popleft()
        return self._scanner.scan()

    def _expect_token(self, expected_token_type):
        t0 = self._get_token()
        if t0.type != expected_token_type:
            raise ParseError(self._scanner.file, t0, [ expected_token_type ])
        return t0

    def _parse_prim_type_expr(self):
        t0 = self._get_token()
        if t0.type == LPAREN:
            result = self.parse_type_expr()
            self._expect_token(RPAREN)
            return result
        if t0.type == IDENTIFIER:
            t1 = self._peek_token()
            args = []
            if t1.type == LESSTHAN:
                self._get_token()
                while True:
                    t2 = self._peek_token()
                    if t2.type == GREATERTHAN:
                        self._get_token()
                        break
                    args.append(self.parse_type_expr())
                    t3 = self._get_token()
                    if t3.type == GREATERTHAN:
                        break
                    if t3.type != COMMA:
                        raise ParseError(self._scanner.file, t3, [ COMMA, GREATERTHAN ])
            return RefTypeExpr(t0.value, args)
        raise ParseError(self._scanner.file, t0, [ LPAREN, IDENTIFIER ])

    def parse_type_expr(self):
        return self._parse_prim_type_expr()

    def parse_param(self) -> Param:
        type_expr = self.parse_type_expr()
        t1 = self._peek_token()
        name = None
        if t1.type == IDENTIFIER:
            self._get_token()
            name = t1
        return Param(type_expr, name)

    def parse_node_member(self):
        type_expr = self.parse_type_expr()
        name = self._expect_token(IDENTIFIER)
        self._expect_token(SEMI)
        return NodeDeclField(name.value, type_expr)

    def parse_toplevel(self):
        t0 = self._get_token()
        if t0.type == EXTERNAL_KEYWORD:
            name = self._expect_token(IDENTIFIER)
            self._expect_token(SEMI)
            return External(name.value)
        if t0.type == NODE_KEYWORD:
            name = self._expect_token(IDENTIFIER).value
            parents = []
            t1 = self._peek_token()
            if t1.type == COLON:
                self._get_token()
                while True:
                    parent = self._expect_token(IDENTIFIER).value
                    parents.append(parent)
                    t2 = self._peek_token()
                    if t2.type != COMMA:
                        break
                    self._get_token()
            self._expect_token(LBRACE)
            members = []
            while True:
                t2 = self._peek_token()
                if t2.type == RBRACE:
                    self._get_token()
                    break
                member = self.parse_node_member()
                members.append(member)
            return NodeDecl(name, parents, members)
        if t0.type == HASH:
            name = self._expect_token(IDENTIFIER)
            if name.value == 'include':
                t1 = self._get_token()
                if t1.type == LESSTHAN:
                    assert(not self._token_buffer)
                    path = self._scanner._take_while(lambda ch: ch != '>')
                    self._scanner._get_char()
                    mode = INCLUDEMODE_SYSTEM
                elif t1.type == STRING:
                    mode = INCLUDEMODE_LOCAL
                    path = t1.value
                else:
                    raise ParseError(self._scanner.file, t1, [ STRING, LESSTHAN ])
                return IncludeDiretive(path, mode)
            raise RuntimeError(f"invalid preprocessor directive '{name.value}'")
        raise ParseError(self._scanner.file, t0, [ EXTERNAL_KEYWORD, NODE_KEYWORD, HASH ])

    def parse_grammar(self):
        elements = []
        while True:
            t0 = self._peek_token()
            if t0.type == END_OF_FILE:
                break
            element = self.parse_toplevel()
            elements.append(element)
        return elements

class Writer:

    def __init__(self, text='', path=None):
        self.path = path
        self.text = text
        self._at_blank_line = True
        self._indentation = '  '
        self._indent_level = 0

    def indent(self, count=1):
        self._indent_level += count

    def dedent(self, count=1):
        self._indent_level -= count

    def write_dedent(self, chunk):
        self.dedent()
        self.write(chunk)
        self.indent()

    def write(self, chunk):
        for ch in chunk:
            if ch == '}':
                self.dedent()
            if ch == '\n':
                self._at_blank_line = True
            elif self._at_blank_line and not _re_whitespace.match(ch):
                self.text += self._indentation * self._indent_level
                self._at_blank_line = False
            self.text += ch
            if ch == '{':
                self.indent()

    def save(self, dest_dir):
        dest_path = dest_dir / self.path
        print(f'Writing file {dest_path} ...')
        with open(dest_path, 'w') as f:
            f.write(self.text)

class DiGraph:

    def __init__(self):
        self._out_edges = dict()
        self._in_edges = dict()

    def add_edge(self, a, b):
        if  a not in self._out_edges:
            self._out_edges[a] = set()
        self._out_edges[a].add(b)
        if b not in self._in_edges:
            self._in_edges[b] = set()
        self._in_edges[b].add(a)

    def get_children(self, node):
        if node not in self._out_edges:
            return
        for child in self._out_edges[node]:
            yield child

    def is_child_of(self, a, b):
        stack = [ b ]
        visited = set()
        while stack:
            node = stack.pop()
            if node in visited:
                break
            visited.add(node)
            if node == a:
                return True
            for child in self.get_children(node):
                stack.append(child)
        return False

    def get_ancestors(self, node):
        if node not in self._in_edges:
            return
        for parent in self._in_edges[node]:
            yield parent

    def get_common_ancestor(self, nodes):
        out = nodes[0]
        parents = []
        for node in nodes[1:]:
            if not self.is_child_of(node, out):
                for parent in self.get_ancestors(node):
                    parents.append(parent)
        if not parents:
            return out
        parents.append(out)
        return self.get_common_ancestor(parents)

def main():

    parser = argparse.ArgumentParser()

    parser.add_argument('file', nargs=1, help='The specification file to generate C++ code for')
    parser.add_argument('--namespace', default='', help='What C++ namespace to put generated code under')
    parser.add_argument('--name', default='AST', help='How to name the generated tree')
    parser.add_argument('-I', default='.', help='What path will be used to include generated header files')
    parser.add_argument('--include-root', default='.', help='Where the headers live inside the include directroy')
    parser.add_argument('--enable-serde', action='store_true', help='Also write (de)serialization logic')
    parser.add_argument('--source-root', default='.', help='Where to store generated souce files')
    parser.add_argument('--node-name', default='Node', help='How the root node of the hierachy should be called')
    parser.add_argument('--out-dir', default='.', help='Place the endire folder structure inside this folder')
    parser.add_argument('--dry-run', action='store_true', help='Do not write generated code to the file system')

    args = parser.parse_args()

    variable_casing = to_upper_camel_case
    type_expr_casing = to_upper_camel_case
    filename = args.file[0]
    prefix = ''
    cpp_root_node_name = prefix + args.node_name
    include_dir = Path(args.I)
    include_path = Path(args.include_root or '.')
    full_include_path = include_dir / include_path
    source_path = Path(args.source_root)
    namespace = args.namespace.split('::')
    out_dir = Path(args.out_dir)
    out_name = args.name
    write_serde = args.enable_serde

    with open(filename, 'r') as f:
        text = f.read()

    scanner = Scanner(text, filename=filename)
    parser = Parser(scanner)
    elements = parser.parse_grammar()

    types = dict()
    nodes = list()
    leaf_nodes = list()
    graph = DiGraph()
    parent_to_children = dict()

    for element in elements:
        if isinstance(element, External) \
                or isinstance(element, NodeDecl):
            types[element.name] = element
        if isinstance(element, NodeDecl):
            nodes.append(element)
            for parent in element.parents:
                graph.add_edge(parent, element.name)
                if parent not in parent_to_children:
                    parent_to_children[parent] = set()
                children = parent_to_children[parent]
                children.add(element)

    for node in nodes:
        if node.name not in parent_to_children:
            leaf_nodes.append(node)

    def is_null_type_expr(type_expr):
        return isinstance(type_expr, RefTypeExpr) and type_expr.name == 'null'

    def is_node(name):
        if name in types:
            return isinstance(types[name], NodeDecl)
        if name in parent_to_children:
            return True
        return False

    def get_all_variant_elements(type_expr):
        types = list()
        def loop(ty):
            if isinstance(ty, RefTypeExpr) and ty.name == 'Variant':
                for arg in ty.args:
                    loop(arg)
            else:
                types.append(ty)
        loop(type_expr)
        return types

    def infer_type(type_expr):
        if isinstance(type_expr, RefTypeExpr):
            if type_expr.name == 'List':
                assert(len(type_expr.args) == 1)
                return ListType(infer_type(type_expr.args[0]))
            if type_expr.name == 'Variant':
                types = get_all_variant_elements(type_expr)
                has_null = False
                for i, ty in enumerate(types):
                    if is_null_type_expr(ty):
                        has_null = True
                        del types[i]
                if all(isinstance(ty, RefTypeExpr) and is_node(ty.name) for ty in types):
                    node_name = graph.get_common_ancestor(list(t.name for t in types))
                    out = NodeType(node_name)
                else:
                    out = VariantType(types)
                if has_null:
                    out = OptionalType(out)
                return out
            if type_expr.name == 'Option':
                assert(len(type_expr.args) == 1)
                return OptionalType(infer_type(type_expr.args[0]))
            if is_node(type_expr.name):
                assert(len(type_expr.args) == 0)
                return NodeType(type_expr.name)
            assert(len(type_expr.args) == 0)
            return RawType(type_expr.name)
        raise RuntimeError(f"unhandled type expression {type_expr}")

    for node in nodes:
        for member in node.members:
            member.type_expr.type = infer_type(member.type_expr)

    def is_optional_by_default(ty):
        return isinstance(ty, NodeType)

    def gen_cpp_type_expr(ty):
        if isinstance(ty, NodeType):
            return prefix + ty.name + "*"
        if isinstance(ty, ListType):
            return f"std::vector<{gen_cpp_type_expr(ty.element_type)}>"
        if isinstance(ty, NodeType):
            return 'class ' + ty.name + '*'
        if isinstance(ty, OptionalType):
            cpp_expr = gen_cpp_type_expr(ty.element_type)
            if is_optional_by_default(ty.element_type):
                return cpp_expr
            return f"std::optional<{cpp_expr}>"
        if isinstance(ty, VariantType):
            return f"std::variant<{','.join(gen_cpp_type_expr(t) for t in ty.types)}>"
        if isinstance(ty, RawType):
            return ty.text
        raise RuntimeError(f"unhandled Type {ty}")

    def gen_cpp_dtor(expr, ty):
        if isinstance(ty, NodeType) or (isinstance(ty, OptionalType) and isinstance(ty.element_type, NodeType)):
            return f'{expr}->unref();\n'
        if isinstance(ty, ListType):
            dtor = gen_cpp_dtor('Element', ty.element_type)
            if dtor:
                out = ''
                out += f'for (auto& Element: {expr})'
                out += '{\n'
                out += dtor
                out += '}\n'
                return out
        if isinstance(ty, OptionalType):
            dtor = gen_cpp_dtor('(*' + expr + ')', ty.element_type)
            if dtor:
                out = ''
                out += 'if ('
                out += expr
                out += ') {\n'
                out += dtor
                out += '}\n'
                return out

    def gen_cpp_ctor_params(out, node):
        visited = set()
        queue = deque([ node ])
        first = True
        while queue:
            node = queue.popleft()
            if node.name in visited:
                return
            visited.add(node.name)
            for member in node.members:
                if first:
                    first = False
                else:
                    out.write(', ')
                out.write(gen_cpp_type_expr(member.type_expr.type))
                out.write(' ')
                out.write(variable_casing(member.name))
            for parent in node.parents:
               queue.append(types[parent])

    def gen_cpp_ctor_args(out, orig_node):
        first = True
        if orig_node.parents:
            for parent in orig_node.parents:
                if first:
                    first = False
                else:
                    out.write(', ')
                node = types[parent]
                out.write(prefix + node.name)
                out.write('(')
                out.write(', '.join(variable_casing(member.name) for member in node.members))
                out.write(')')
        else:
            out.write(cpp_root_node_name + '()')
            first = False
        for member in orig_node.members:
            if first:
                first = False
            else:
                out.write(', ')
            out.write(variable_casing(member.name))
            out.write('(')
            out.write(variable_casing(member.name))
            out.write(')')

    node_hdr = Writer(path=full_include_path / (out_name + '.hpp'))
    node_src = Writer(path=source_path / (out_name + '.cc'))

    macro_prefix = '_'.join(namespace).upper() + '_'

    node_hdr_guard_name = f'{macro_prefix}{out_name.upper()}_HPP'
    node_hdr.write(f'#ifndef {node_hdr_guard_name}')
    node_hdr.write('\n')
    node_hdr.write(f'#define {node_hdr_guard_name}')
    node_hdr.write('\n\n')

    for element in elements:
        if isinstance(element, IncludeDiretive):
            node_hdr.write(str(element))

    for name in namespace:
        node_hdr.write("namespace ")
        node_hdr.write(name)
        node_hdr.write(" {\n\n")

    if write_serde:
        node_hdr.write('class Encoder;\n')
        node_hdr.write('class Decoder;\n')
        node_hdr.write('\n')

    for element in elements:
        if isinstance(element, NodeDecl):
            node_hdr.write(f'class {element.name};\n')
    node_hdr.write('\n')

    node_hdr.write("enum class ")
    node_hdr.write(prefix)
    node_hdr.write("NodeType {\n")
    for node in leaf_nodes:
        node_hdr.write(node.name)
        node_hdr.write(",\n")
    node_hdr.write("};\n\n")

    node_hdr.write("class ")
    node_hdr.write(cpp_root_node_name)
    node_hdr.write(" {\n\n")
    node_hdr.write('unsigned RefCount = 0;\n\n')
    node_hdr.write_dedent('public:\n\n')
    node_hdr.write('inline void ref() {\n++RefCount;\n}\n\n')
    node_hdr.write('inline void unref() {\n--RefCount;\nif (RefCount == 0) {\ndelete this;\n}\n}\n\n')
    node_hdr.write(f"{cpp_root_node_name}Type Type;\n\n")
    node_hdr.write('void setParents();\n\n')
    node_hdr.write('SourceFile* getSourceFile();\n\n');
    node_hdr.write('const SourceFile* getSourceFile() const;\n\n');
    if write_serde:
        node_hdr.write('void encode(Encoder& encoder) const;\n\n')
        node_hdr.write('virtual void encode_fields(Encoder& encoder) const = 0;\n');
        node_hdr.write('virtual void decode_fields(Decoder& decoder) = 0;\n\n');
    node_hdr.write(f'virtual ~{cpp_root_node_name}();')
    node_hdr.write('\n')
    node_hdr.write("};\n\n")

    for element in elements:
        if isinstance(element, NodeDecl):
            node = element
            is_leaf = not list(graph.get_children(node.name))
            cpp_node_name = prefix + node.name
            node_hdr.write("class ")
            node_hdr.write(cpp_node_name)
            node_hdr.write(" : public ")
            if node.parents:
                node_hdr.write(', '.join(prefix + parent for parent in node.parents))
            else:
                node_hdr.write(cpp_root_node_name)
            node_hdr.write(" {\n")
            node_hdr.write_dedent('public:\n\n')

            node_hdr.write(cpp_node_name + '(')
            gen_cpp_ctor_params(node_hdr, node)
            node_hdr.write('): ')
            gen_cpp_ctor_args(node_hdr, node)
            node_hdr.write(' {}\n\n')

            if node.members:
                for member in node.members:
                    node_hdr.write(gen_cpp_type_expr(member.type_expr.type))
                    node_hdr.write(" ");
                    node_hdr.write(variable_casing(member.name))
                    node_hdr.write(";\n");
                node_hdr.write('\n')

            if write_serde and is_leaf:
                node_hdr.write('void encode_fields(Encoder& encoder) const override;\n');
                node_hdr.write('void decode_fields(Decoder& decoder) override;\n\n');

            if node.name == 'SourceFile':
                node_hdr.write('TextFile& getTextFile() {\nreturn File;\n}\n\n')
                node_hdr.write('const TextFile& getTextFile() const {\nreturn File;\n}\n\n')

            # elif node.name == 'Token':
            #     node_hdr.write('TextRange getRange() const {\nreturn TextRange(StartLoc, EndLoc);\n}\n\n')

            node_hdr.write('~')
            node_hdr.write(cpp_node_name)
            node_hdr.write('();\n\n')

            node_hdr.write("};\n\n")

    for _ in namespace:
        node_hdr.write("}\n\n")

    node_hdr.write('#endif\n')

    node_src.write('#include "')
    node_src.write(str(include_path / (out_name + '.hpp')))
    node_src.write('"\n')
    node_src.write('\n')

    if write_serde:
        node_src.write('#include "')
        node_src.write(str(include_path / 'Encoder.hpp'))
        node_src.write('"\n')
        node_src.write('\n')

    for name in namespace:
        node_src.write("namespace ")
        node_src.write(name)
        node_src.write(" {\n\n")

    if write_serde:
        node_src.write(f'void {cpp_root_node_name}::encode(Encoder& encoder) const')
        node_src.write(' {\n')
        node_src.write(f'encoder.start_encode_struct("{cpp_root_node_name}");')
        node_src.write('\n')
        node_src.write('encode_fields(encoder);\n')
        node_src.write('encoder.end_encode_struct();\n')
        node_src.write('}\n\n')

    for node in nodes:
        is_leaf = not list(graph.get_children(node.name))
        cpp_node_name = prefix + node.name
        if write_serde and is_leaf:
            node_src.write(f'void {cpp_node_name}::encode_fields(Encoder& encoder) const')
            node_src.write(' { \n')
            for member in node.members:
                node_src.write(f'encoder.encode_field("{member.name}", {member.name});')
                node_src.write('\n')
            node_src.write('}\n\n')
        node_src.write(f'{cpp_node_name}::~{cpp_node_name}()')
        node_src.write('{ \n')
        for member in node.members:
            dtor = gen_cpp_dtor(variable_casing(member.name), member.type_expr.type)
            if dtor:
                node_src.write(dtor)
        node_src.write('}\n\n')

    for _ in namespace:
        node_src.write("}\n\n")

    if args.dry_run:
        print('# ' + str(node_hdr.path))
        print(node_hdr.text)
        print('# ' + str(node_src.path))
        print(node_src.text)
    else:
        out_dir.mkdir(exist_ok=True, parents=True)
        node_hdr.save(out_dir)
        node_src.save(out_dir)

if __name__ == '__main__':
    main()
