#ifndef BOLT_CST_HPP
#define BOLT_CST_HPP

#include <stdint.h>
#include <cmath>
#include <cstdlib>
#include <unordered_map>
#include <variant>
#include <vector>
#include <optional>
#include <filesystem>

#include "zen/config.hpp"
#include "zen/range.hpp"

#include "bolt/Common.hpp"
#include "bolt/Integer.hpp"
#include "bolt/String.hpp"
#include "bolt/ByteString.hpp"
#include "bolt/Type.hpp"

namespace bolt {

class Token;
class SourceFile;
class Scope;
class Pattern;
class Expression;

class TextLoc {
public:

  std::size_t Line = 1;
  std::size_t Column = 1;

  inline bool isEmpty() const noexcept {
    return Line == 0 && Column == 0;
  }

  inline void advance(const ByteString& Text) {
    for (auto Chr: Text) {
      if (Chr == '\n') {
        Line++;
        Column = 1;
      } else {
        Column++;
      }
    }
  }

  inline TextLoc operator+(const ByteString& Text) const {
    TextLoc Out { Line, Column };
    Out.advance(Text);
    return Out;
  }

  static TextLoc empty() {
    return TextLoc { 0, 0 };
  }

};

struct TextRange {
  TextLoc Start;
  TextLoc End;
};

class TextFile {

  ByteString Path;
  ByteString Text;

  std::vector<std::size_t> LineOffsets;

public:

  TextFile(ByteString Path, ByteString Text);

  std::size_t getLine(std::size_t Offset) const;
  std::size_t getColumn(std::size_t Offset) const;
  std::size_t getStartOffsetOfLine(std::size_t Line) const;
  std::size_t getEndOffsetOfLine(std::size_t Line) const;

  std::size_t getLineCount() const;

  ByteString getPath() const;

  ByteString getText() const;

};

enum class NodeKind {

  // Plain tokens
  Assignment,
  At,
  Backslash,
  Colon,
  Comma,
  CustomOperator,
  DoKeyword,
  Dot,
  DotDot,
  Equals,
  Identifier,
  IdentifierAlt,
  LBrace,
  LBracket,
  LParen,
  RArrow,
  RArrowAlt,
  RBrace,
  RBracket,
  RParen,
  Tilde,
  VBar,
  WrappedOperator,

  // Keywords
  ClassKeyword,
  ElifKeyword,
  ElseKeyword,
  EnumKeyword,
  FnKeyword,
  ForeignKeyword,
  IfKeyword,
  InstanceKeyword,
  LetKeyword,
  MatchKeyword,
  ModKeyword,
  MutKeyword,
  PubKeyword,
  ReturnKeyword,
  StructKeyword,
  TypeKeyword,

  // Virtual tokens
  BlockStart,
  BlockEnd,
  LineFoldEnd,
  EndOfFile,
  Invalid,

  // Literal tokens
  StringLiteral,
  IntegerLiteral,

  // Annotations
  ExpressionAnnotation,
  TypeAssertAnnotation,

  // Constraint expressions
  TypeclassConstraintExpression,
  EqualityConstraintExpression,

  RecordTypeExpressionField,

  // Type expressions
  AppTypeExpression,
  ArrowTypeExpression,
  NestedTypeExpression,
  QualifiedTypeExpression,
  RecordTypeExpression,
  ReferenceTypeExpression,
  TupleTypeExpression,
  VarTypeExpression,

  RecordPatternField,

  // Patterns
  BindPattern,
  ListPattern,
  LiteralPattern,
  NamedRecordPattern,
  NamedTuplePattern,
  NestedPattern,
  RecordPattern,
  TuplePattern,

  MatchCase,
  RecordExpressionField,
  IfExpressionPart,

  // Expressions
  BlockExpression,
  CallExpression,
  FunctionExpression,
  IfExpression,
  InfixExpression,
  LiteralExpression,
  MatchExpression,
  MemberExpression,
  NestedExpression,
  PrefixExpression,
  RecordExpression,
  ReferenceExpression,
  ReturnExpression,
  TupleExpression,

  TypeAssert,
  Parameter,

  // Bodies of a let-declaration or function declaration
  LetBlockBody,
  LetExprBody,

  // Function declarations
  PrefixFunctionDeclaration,
  InfixFunctionDeclaration,
  SuffixFunctionDeclaration,
  NamedFunctionDeclaration,

  RecordDeclarationField,
  TupleVariantDeclarationMember,
  RecordVariantDeclarationMember,

  // Other declarations
  ClassDeclaration,
  InstanceDeclaration,
  RecordDeclaration,
  VariableDeclaration,
  VariantDeclaration,

  SourceFile,
};

struct SymbolPath {
  std::vector<ByteString> Modules;
  ByteString Name;
};

template<typename T>
NodeKind getNodeType();

enum NodeFlags {
  NodeFlags_TypeIsSolved = 1,
};

using NodeFlagsMask = unsigned;

class Node;

template<typename T>
bool _is_helper(const Node* N) noexcept;

class Node {

  unsigned RefCount = 1;

  const NodeKind K;

public:

  NodeFlagsMask Flags = 0;
  Node* Parent = nullptr;

  inline void ref() {
    ++RefCount;
  }

  void unref();

  void setParents();

  virtual Token* getFirstToken() const = 0;
  virtual Token* getLastToken() const = 0;

  virtual std::size_t getStartLine() const;
  virtual std::size_t getStartColumn() const;
  virtual std::size_t getEndLine() const;
  virtual std::size_t getEndColumn() const;

  inline NodeKind getKind() const noexcept {
    return K;
  }

  virtual TextRange getRange() const;

  inline Node(NodeKind Type):
      K(Type) {}

  const SourceFile* getSourceFile() const;
  SourceFile* getSourceFile();

  virtual Scope* getScope();

  virtual ~Node() {}

};

enum class SymbolKind {
  Var,
  Class,
  Type,
  Constructor,
};

class Scope {

  Node* Source;
  std::unordered_multimap<ByteString, std::tuple<Node*, SymbolKind>> Mapping;

  void addSymbol(ByteString Name, Node* Decl, SymbolKind Kind);

  void scan(Node* X);
  void scanChild(Node* X);

  void visitPattern(Pattern* P, Node* ToInsert);

public:

  Scope(Node* Source);

  /**
   * Performs a direct lookup in this scope for the given symbol.
   *
   * This method will never traverse to parent scopes and will always return a
   * symbol that belongs to this scope, if any is found.
   *
   * \returns nullptr when no such symbol could be found in this scope.
   */
  Node* lookupDirect(SymbolPath Path, SymbolKind Kind = SymbolKind::Var);

  /**
   * Find the symbol with the given name, either in this scope or in any of
   * the parent ones.
   *
   * \returns nullptr when no such symbol could be found in any of the scopes.
   */
  Node* lookup(SymbolPath Path, SymbolKind Kind = SymbolKind::Var);

  Scope* getParentScope();

};

class Token : public Node {

  TextLoc StartLoc;

public:

  Token(NodeKind Type, TextLoc StartLoc): Node(Type), StartLoc(StartLoc) {}

  virtual std::string getText() const = 0;

  inline Token* getFirstToken() const override {
    ZEN_UNREACHABLE
  }

  inline Token* getLastToken() const override {
    ZEN_UNREACHABLE
  }

  inline TextLoc getStartLoc() const {
    return StartLoc;
  }

  TextLoc getEndLoc() const;

  inline std::size_t getStartLine() const override {
    return StartLoc.Line;
  }

  inline std::size_t getStartColumn() const override {
    return StartLoc.Column;
  }

  inline std::size_t getEndLine() const override {
    return getEndLoc().Line;
  }

  inline std::size_t getEndColumn() const override {
    return getEndLoc().Column;
  }

  TextRange getRange() const override {
    return { getStartLoc(), getEndLoc() };
  }

};

class Equals : public Token {
public:

  inline Equals(TextLoc StartLoc):
    Token(NodeKind::Equals, StartLoc) {}

  std::string getText() const override;

  static constexpr NodeKind Kind = NodeKind::Equals;

};

class VBar : public Token {
public:

  inline VBar(TextLoc StartLoc):
    Token(NodeKind::VBar, StartLoc) {}

  std::string getText() const override;

  static constexpr const NodeKind Kind = NodeKind::VBar;

};

class Colon : public Token {
public:

  inline Colon(TextLoc StartLoc):
    Token(NodeKind::Colon, StartLoc) {}

  std::string getText() const override;

  static constexpr const NodeKind Kind = NodeKind::Colon;

};

class Comma : public Token {
public:

  inline Comma(TextLoc StartLoc):
    Token(NodeKind::Comma, StartLoc) {}

  std::string getText() const override;

  static constexpr const NodeKind Kind = NodeKind::Comma;

};

class Dot : public Token {
public:

  inline Dot(TextLoc StartLoc):
    Token(NodeKind::Dot, StartLoc) {}

  std::string getText() const override;

  static constexpr const NodeKind Kind = NodeKind::Dot;

};

class DotDot : public Token {
public:

  inline DotDot(TextLoc StartLoc):
    Token(NodeKind::DotDot, StartLoc) {}

  std::string getText() const override;

  static constexpr const NodeKind Kind = NodeKind::DotDot;

};

class Tilde : public Token {
public:

  inline Tilde(TextLoc StartLoc):
    Token(NodeKind::Tilde, StartLoc) {}

  std::string getText() const override;

  static constexpr const NodeKind Kind = NodeKind::Tilde;

};

class At : public Token {
public:

  inline At(TextLoc StartLoc):
    Token(NodeKind::At, StartLoc) {}

  std::string getText() const override;

  static constexpr const NodeKind Kind = NodeKind::At;

};

class Backslash : public Token {
public:

  inline Backslash(TextLoc StartLoc):
    Token(NodeKind::Backslash, StartLoc) {}

  std::string getText() const override;

  static constexpr const NodeKind Kind = NodeKind::Backslash;

};

class DoKeyword : public Token {
public:

  inline DoKeyword(TextLoc StartLoc):
    Token(NodeKind::DoKeyword, StartLoc) {}

  std::string getText() const override;

  static constexpr const NodeKind Kind = NodeKind::DoKeyword;

};

class LParen : public Token {
public:

  inline LParen(TextLoc StartLoc):
    Token(NodeKind::LParen, StartLoc) {}

  std::string getText() const override;

  static constexpr const NodeKind Kind = NodeKind::LParen;

};

class RParen : public Token {
public:

  inline RParen(TextLoc StartLoc):
    Token(NodeKind::RParen, StartLoc) {}

  std::string getText() const override;

  static constexpr const NodeKind Kind = NodeKind::RParen;

};

class LBracket : public Token {
public:

  inline LBracket(TextLoc StartLoc):
    Token(NodeKind::LBracket, StartLoc) {}

  std::string getText() const override;

  static constexpr const NodeKind Kind = NodeKind::LBracket;

};

class RBracket : public Token {
public:

  inline RBracket(TextLoc StartLoc):
    Token(NodeKind::RBracket, StartLoc) {}

  std::string getText() const override;

  static constexpr const NodeKind Kind = NodeKind::RBracket;

};

class LBrace : public Token {
public:

  inline LBrace(TextLoc StartLoc):
    Token(NodeKind::LBrace, StartLoc) {}

  std::string getText() const override;

  static constexpr const NodeKind Kind = NodeKind::LBrace;

};

class RBrace : public Token {
public:

  inline RBrace(TextLoc StartLoc):
    Token(NodeKind::RBrace, StartLoc) {}

  std::string getText() const override;

  static constexpr const NodeKind Kind = NodeKind::RBrace;

};

class RArrow : public Token {
public:

  inline RArrow(TextLoc StartLoc):
    Token(NodeKind::RArrow, StartLoc) {}

  std::string getText() const override;

  static constexpr const NodeKind Kind = NodeKind::RArrow;

};

class RArrowAlt : public Token {
public:

  inline RArrowAlt(TextLoc StartLoc):
    Token(NodeKind::RArrowAlt, StartLoc) {}

  std::string getText() const override;

  static constexpr const NodeKind Kind = NodeKind::RArrowAlt;

};

class LetKeyword : public Token {
public:

  inline LetKeyword(TextLoc StartLoc):
    Token(NodeKind::LetKeyword, StartLoc) {}

  std::string getText() const override;

  static constexpr const NodeKind Kind = NodeKind::LetKeyword;

};

class MutKeyword : public Token {
public:

  inline MutKeyword(TextLoc StartLoc):
    Token(NodeKind::MutKeyword, StartLoc) {}

  std::string getText() const override;

  static constexpr const NodeKind Kind = NodeKind::MutKeyword;

};

class PubKeyword : public Token {
public:

  inline PubKeyword(TextLoc StartLoc):
    Token(NodeKind::PubKeyword, StartLoc) {}

  std::string getText() const override;

  static constexpr const NodeKind Kind = NodeKind::PubKeyword;

};

class ForeignKeyword : public Token {
public:

  inline ForeignKeyword(TextLoc StartLoc):
    Token(NodeKind::ForeignKeyword, StartLoc) {}

  std::string getText() const override;

  static constexpr const NodeKind Kind = NodeKind::ForeignKeyword;

};

class TypeKeyword : public Token {
public:

  inline TypeKeyword(TextLoc StartLoc):
    Token(NodeKind::TypeKeyword, StartLoc) {}

  std::string getText() const override;

  static constexpr const NodeKind Kind = NodeKind::TypeKeyword;

};

class ReturnKeyword : public Token {
public:

  inline ReturnKeyword(TextLoc StartLoc):
    Token(NodeKind::ReturnKeyword, StartLoc) {}

  std::string getText() const override;

  static constexpr const NodeKind Kind = NodeKind::ReturnKeyword;

};

class ModKeyword : public Token {
public:

  inline ModKeyword(TextLoc StartLoc):
    Token(NodeKind::ModKeyword, StartLoc) {}

  std::string getText() const override;

  static constexpr const NodeKind Kind = NodeKind::ModKeyword;

};

class StructKeyword : public Token {
public:

  inline StructKeyword(TextLoc StartLoc):
    Token(NodeKind::StructKeyword, StartLoc) {}

  std::string getText() const override;

  static constexpr const NodeKind Kind = NodeKind::StructKeyword;

};

class EnumKeyword : public Token {
public:

  inline EnumKeyword(TextLoc StartLoc):
    Token(NodeKind::EnumKeyword, StartLoc) {}

  std::string getText() const override;

  static constexpr const NodeKind Kind = NodeKind::EnumKeyword;

};

class FnKeyword : public Token {
public:

  inline FnKeyword(TextLoc StartLoc):
    Token(NodeKind::FnKeyword, StartLoc) {}

  std::string getText() const override;

  static constexpr const NodeKind Kind = NodeKind::FnKeyword;

};

class ClassKeyword : public Token {
public:

  inline ClassKeyword(TextLoc StartLoc):
    Token(NodeKind::ClassKeyword, StartLoc) {}

  std::string getText() const override;

  static constexpr const NodeKind Kind = NodeKind::ClassKeyword;

};

class InstanceKeyword : public Token {
public:

  inline InstanceKeyword(TextLoc StartLoc):
    Token(NodeKind::InstanceKeyword, StartLoc) {}

  std::string getText() const override;

  static constexpr const NodeKind Kind = NodeKind::InstanceKeyword;

};

class ElifKeyword : public Token {
public:

  inline ElifKeyword(TextLoc StartLoc):
    Token(NodeKind::ElifKeyword, StartLoc) {}

  std::string getText() const override;

  static constexpr const NodeKind Kind = NodeKind::ElifKeyword;

};

class IfKeyword : public Token {
public:

  inline IfKeyword(TextLoc StartLoc):
    Token(NodeKind::IfKeyword, StartLoc) {}

  std::string getText() const override;

  static constexpr const NodeKind Kind = NodeKind::IfKeyword;

};

class ElseKeyword : public Token {
public:

  inline ElseKeyword(TextLoc StartLoc):
    Token(NodeKind::ElseKeyword, StartLoc) {}

  std::string getText() const override;

  static constexpr const NodeKind Kind = NodeKind::ElseKeyword;

};

class MatchKeyword : public Token {
public:

  inline MatchKeyword(TextLoc StartLoc):
    Token(NodeKind::MatchKeyword, StartLoc) {}

  std::string getText() const override;

  static constexpr const NodeKind Kind = NodeKind::MatchKeyword;

};

class Invalid : public Token {
public:

  inline Invalid(TextLoc StartLoc):
    Token(NodeKind::Invalid, StartLoc) {}

  std::string getText() const override;

  static constexpr const NodeKind Kind = NodeKind::Invalid;

};

class EndOfFile : public Token {
public:

  inline EndOfFile(TextLoc StartLoc):
    Token(NodeKind::EndOfFile, StartLoc) {}

  std::string getText() const override;

  static constexpr const NodeKind Kind = NodeKind::EndOfFile;

};

class BlockStart : public Token {
public:

  inline BlockStart(TextLoc StartLoc):
    Token(NodeKind::BlockStart, StartLoc) {}

  std::string getText() const override;

  static constexpr const NodeKind Kind = NodeKind::BlockStart;

};

class BlockEnd : public Token {
public:

  inline BlockEnd(TextLoc StartLoc):
    Token(NodeKind::BlockEnd, StartLoc) {}

  std::string getText() const override;

  static constexpr const NodeKind Kind = NodeKind::BlockEnd;

};

class LineFoldEnd : public Token {
public:

  inline LineFoldEnd(TextLoc StartLoc):
    Token(NodeKind::LineFoldEnd, StartLoc) {}

  std::string getText() const override;

  static constexpr const NodeKind Kind = NodeKind::LineFoldEnd;

};

class CustomOperator : public Token {
public:

  ByteString Text;

  CustomOperator(ByteString Text, TextLoc StartLoc):
    Token(NodeKind::CustomOperator, StartLoc), Text(Text) {}

  std::string getText() const override;

  std::string getCanonicalText() const;

  static constexpr const NodeKind Kind = NodeKind::CustomOperator;

};

class Assignment : public Token {
public:

  ByteString Text;

  Assignment(ByteString Text, TextLoc StartLoc):
    Token(NodeKind::Assignment, StartLoc), Text(Text) {}

  std::string getText() const override;

  static constexpr const NodeKind Kind = NodeKind::Assignment;

};

class Identifier : public Token {
public:

  ByteString Text;

  Identifier(ByteString Text, TextLoc StartLoc = TextLoc::empty()):
    Token(NodeKind::Identifier, StartLoc), Text(Text) {}

  std::string getText() const override;

  ByteString getCanonicalText() const;

  bool isTypeVar() const;

  static constexpr const NodeKind Kind = NodeKind::Identifier;

};

class IdentifierAlt : public Token {
public:

  ByteString Text;

  IdentifierAlt(ByteString Text, TextLoc StartLoc):
    Token(NodeKind::IdentifierAlt, StartLoc), Text(Text) {}

  std::string getText() const override;

  ByteString getCanonicalText() const;

  static constexpr const NodeKind Kind = NodeKind::IdentifierAlt;

};

using LiteralValue = std::variant<ByteString, Integer>;

class Literal : public Token {
public:

  inline Literal(NodeKind Kind, TextLoc StartLoc):
    Token(Kind, StartLoc) {}

  virtual LiteralValue getValue() = 0;

  static bool classof(const Node* N) {
    return N->getKind() == NodeKind::StringLiteral
        || N->getKind() == NodeKind::IntegerLiteral;
  }

};

class StringLiteral : public Literal {
public:

  ByteString Text;

  StringLiteral(ByteString Text, TextLoc StartLoc):
    Literal(NodeKind::StringLiteral, StartLoc), Text(Text) {}

  std::string getText() const override;

  LiteralValue getValue() override;

  static constexpr const NodeKind Kind = NodeKind::StringLiteral;

};

class IntegerLiteral : public Literal {
public:

  Integer V;

  IntegerLiteral(Integer Value, TextLoc StartLoc):
    Literal(NodeKind::IntegerLiteral, StartLoc), V(Value) {}

  std::string getText() const override;

  inline Integer getInteger() const noexcept {
    return V;
  }

  inline int asInt() const {
    ZEN_ASSERT(V >= std::numeric_limits<int>::min() && V <= std::numeric_limits<int>::max());
    return V;
  }

  LiteralValue getValue() override;

  static constexpr const NodeKind Kind = NodeKind::IntegerLiteral;

};

/// Base node for things that can be used as an operator
///
/// This includes the following nodes:
/// - VBar
/// - CustomOperator
class Operator {

  Node* N;

  Operator(Node* N):
    N(N) {}

public:

  Operator() {}

  Operator(VBar* N):
    N(N) {}

  Operator(CustomOperator* N):
    N(N) {}

  static Operator from_raw_node(Node* N) {
    ZEN_ASSERT(isa<Operator>(N));
    return N;
  }

  inline NodeKind getKind() const {
    return N->getKind();
  }

  inline bool isVBar() const {
    return N->getKind() == NodeKind::VBar;
  }

  inline bool isCustomOperator() const {
    return N->getKind() == NodeKind::CustomOperator;
  }

  VBar* asVBar() const {
    return static_cast<VBar*>(N);
  }

  CustomOperator* asCustomOperator() const {
    return static_cast<CustomOperator*>(N);
  }

  operator Node*() const {
    return N;
  }

  /// Get the name that is actually represented by an operator, without all the
  /// syntactic sugar.
  virtual ByteString getCanonicalText() const;

  Token* getFirstToken() const;
  Token* getLastToken() const;

  static bool classof(const Node* N) {
    return N->getKind() == NodeKind::VBar
        || N->getKind() == NodeKind::CustomOperator;
  }

};
class WrappedOperator : public Node {
public:

  class LParen* LParen;
  Operator Op;
  class RParen* RParen;

  WrappedOperator(
    class LParen* LParen,
    Operator Operator,
    class RParen* RParen
  ): Node(NodeKind::WrappedOperator),
     LParen(LParen),
     Op(Operator),
     RParen(RParen) {}

  inline Operator getOperator() const {
    return Op;
  }

  ByteString getCanonicalText() const {
    return Op.getCanonicalText();
  }

  Token* getFirstToken() const override;
  Token* getLastToken() const override;

  static constexpr const NodeKind Kind = NodeKind::WrappedOperator;

};

/// Base node for things that can be used as a symbol
///
/// This includes the following nodes:
/// - WrappedOperator
/// - Identifier
/// - IdentifierAlt
class Symbol {

  Node* N;

  Symbol(Node* N):
    N(N) {}

public:

  Symbol() {}

  Symbol(WrappedOperator* N):
    N(N) {}

  Symbol(Identifier* N):
    N(N) {}

  Symbol(IdentifierAlt* N):
    N(N) {}

  static Symbol from_raw_node(Node* N) {
    ZEN_ASSERT(isa<Symbol>(N));
    return N;
  }

  NodeKind getKind() const {
    return N->getKind();
  }

  bool isWrappedOperator() const {
    return N->getKind() == NodeKind::WrappedOperator;
  }

  bool isIdentifier() const {
    return N->getKind() == NodeKind::Identifier;
  }

  bool isIdentifierAlt() const {
    return N->getKind() == NodeKind::IdentifierAlt;
  }

  IdentifierAlt* asIdentifierAlt() const {
    return cast<IdentifierAlt>(N);
  }

  Identifier* asIdentifier() const {
    return cast<Identifier>(N);
  }

  WrappedOperator* asWrappedOperator() const {
    return cast<WrappedOperator>(N);
  }

  operator Node*() const {
    return N;
  }

  /// Get the name that is actually represented by a symbol, without all the
  /// syntactic sugar.
  ByteString getCanonicalText() const;

  Token* getFirstToken() const;
  Token* getLastToken() const;

  static bool classof(const Node* N) {
    return N->getKind() == NodeKind::Identifier
        || N->getKind() == NodeKind::IdentifierAlt
        || N->getKind() == NodeKind::WrappedOperator;
  }

};


class Annotation : public Node {
public:

  inline Annotation(NodeKind Kind):
    Node(Kind) {}

};

class AnnotationContainer {
public:

  std::vector<Annotation*> Annotations;

  inline AnnotationContainer():
    Annotations({}) {}

  inline AnnotationContainer(std::vector<Annotation*> Annotations):
    Annotations(Annotations) {}

};

class ExpressionAnnotation : public Annotation {
public:

  class At* At;
  class Expression* Expression;

  inline ExpressionAnnotation(
    class At* At,
    class Expression* Expression
  ): Annotation(NodeKind::ExpressionAnnotation),
     At(At),
     Expression(Expression) {}

  Token* getFirstToken() const override;
  Token* getLastToken() const override;

  inline class Expression* getExpression() const noexcept {
    return Expression;
  }

  static constexpr const NodeKind Kind = NodeKind::ExpressionAnnotation;

};

class TypeExpression;

class TypeAssertAnnotation : public Annotation {
public:

  class At* At;
  class Colon* Colon;
  TypeExpression* TE;

  inline TypeAssertAnnotation(
    class At* At,
    class Colon* Colon,
    TypeExpression* TE
  ): Annotation(NodeKind::TypeAssertAnnotation),
     At(At),
     Colon(Colon),
     TE(TE) {}

  Token* getFirstToken() const override;
  Token* getLastToken() const override;

  inline TypeExpression* getTypeExpression() const noexcept {
    return TE;
  }

  static constexpr const NodeKind Kind = NodeKind::TypeAssertAnnotation;

};

class TypedNode : public Node {
protected:

  Type* Ty;

  inline TypedNode(NodeKind Kind):
    Node(Kind) {}

public:

  inline void setType(Type* Ty2) {
    Ty = Ty2;
  }

  inline Type* getType() const noexcept {
    ZEN_ASSERT(Ty != nullptr);
    return Ty;
  }

  static bool classof(const Node* N);

};

class TypeExpression : public TypedNode, AnnotationContainer {
protected:

  inline TypeExpression(NodeKind Kind, std::vector<Annotation*> Annotations = {}):
    TypedNode(Kind), AnnotationContainer(Annotations) {}

public:

  static bool classof(const Node* N) {
    return N->getKind() == NodeKind::ReferenceTypeExpression
        || N->getKind() == NodeKind::AppTypeExpression
        || N->getKind() == NodeKind::NestedTypeExpression
        || N->getKind() == NodeKind::ArrowTypeExpression
        || N->getKind() == NodeKind::VarTypeExpression
        || N->getKind() == NodeKind::TupleTypeExpression
        || N->getKind() == NodeKind::RecordTypeExpression
        || N->getKind() == NodeKind::QualifiedTypeExpression;
  }

};

class ConstraintExpression : public Node {
public:

  inline ConstraintExpression(NodeKind Kind):
    Node(Kind) {}

};

class RecordTypeExpressionField : public Node {
public:

  Identifier* Name;
  class Colon* Colon;
  TypeExpression* TE;

  inline RecordTypeExpressionField(
    Identifier* Name,
    class Colon* Colon,
    TypeExpression* TE
  ): Node(NodeKind::RecordTypeExpressionField),
     Name(Name),
     Colon(Colon),
     TE(TE) {}

  Token* getFirstToken() const override;
  Token* getLastToken() const override;

  static constexpr const NodeKind Kind = NodeKind::RecordTypeExpressionField;

};

class RecordTypeExpression : public TypeExpression {
public:

  class LBrace* LBrace;
  std::vector<std::tuple<RecordTypeExpressionField*, Comma*>> Fields;
  class VBar* VBar;
  TypeExpression* Rest;
  class RBrace* RBrace;

  inline RecordTypeExpression(
    class LBrace* LBrace,
    std::vector<std::tuple<RecordTypeExpressionField*, Comma*>> Fields,
    class VBar* VBar,
    TypeExpression* Rest,
    class RBrace* RBrace
  ): TypeExpression(NodeKind::RecordTypeExpression),
     LBrace(LBrace),
     Fields(Fields),
     VBar(VBar),
     Rest(Rest),
     RBrace(RBrace) {}

  Token* getFirstToken() const override;
  Token* getLastToken() const override;

  static constexpr const NodeKind Kind = NodeKind::RecordTypeExpression;

};

class VarTypeExpression;

class TypeclassConstraintExpression : public ConstraintExpression {
public:

  IdentifierAlt* Name;
  std::vector<VarTypeExpression*> TEs;

  TypeclassConstraintExpression(
    IdentifierAlt* Name,
    std::vector<VarTypeExpression*> TEs
  ): ConstraintExpression(NodeKind::TypeclassConstraintExpression),
     Name(Name),
     TEs(TEs) {}

  Token* getFirstToken() const override;
  Token* getLastToken() const override;

  static constexpr const NodeKind Kind = NodeKind::TypeclassConstraintExpression;

};

class EqualityConstraintExpression : public ConstraintExpression {
public:

  TypeExpression* Left;
  class Tilde* Tilde;
  TypeExpression* Right;

  inline EqualityConstraintExpression(
    TypeExpression* Left,
    class Tilde* Tilde,
    TypeExpression* Right
  ): ConstraintExpression(NodeKind::EqualityConstraintExpression),
     Left(Left),
     Tilde(Tilde),
     Right(Right) {}

  Token* getFirstToken() const override;
  Token* getLastToken() const override;

  static constexpr const NodeKind Kind = NodeKind::EqualityConstraintExpression;

};

class QualifiedTypeExpression : public TypeExpression {
public:

  std::vector<std::tuple<ConstraintExpression*, Comma*>> Constraints;
  class RArrowAlt* RArrowAlt;
  TypeExpression* TE;

  QualifiedTypeExpression(
    std::vector<std::tuple<ConstraintExpression*, Comma*>> Constraints,
    class RArrowAlt* RArrowAlt,
    TypeExpression* TE
  ): TypeExpression(NodeKind::QualifiedTypeExpression),
     Constraints(Constraints),
     RArrowAlt(RArrowAlt),
     TE(TE) {}

  Token* getFirstToken() const override;
  Token* getLastToken() const override;

  static constexpr const NodeKind Kind = NodeKind::QualifiedTypeExpression;

};

class ReferenceTypeExpression : public TypeExpression {
public:

  std::vector<std::tuple<IdentifierAlt*, Dot*>> ModulePath;
  IdentifierAlt* Name;

  ReferenceTypeExpression(
    std::vector<std::tuple<IdentifierAlt*, Dot*>> ModulePath,
    IdentifierAlt* Name
  ): TypeExpression(NodeKind::ReferenceTypeExpression),
     ModulePath(ModulePath),
     Name(Name) {}

  Token* getFirstToken() const override;
  Token* getLastToken() const override;

  SymbolPath getSymbolPath() const;

  static constexpr const NodeKind Kind = NodeKind::ReferenceTypeExpression;

};

class ArrowTypeExpression : public TypeExpression {
public:

  std::vector<TypeExpression*> ParamTypes;
  TypeExpression* ReturnType;

  inline ArrowTypeExpression(
    std::vector<TypeExpression*> ParamTypes,
    TypeExpression* ReturnType
  ): TypeExpression(NodeKind::ArrowTypeExpression),
     ParamTypes(ParamTypes),
     ReturnType(ReturnType) {}

  Token* getFirstToken() const override;
  Token* getLastToken() const override;

  static constexpr const NodeKind Kind = NodeKind::ArrowTypeExpression;

};

class AppTypeExpression : public TypeExpression {
public:

  TypeExpression* Op;
  std::vector<TypeExpression*> Args;

  inline AppTypeExpression(
    TypeExpression* Op,
    std::vector<TypeExpression*> Args
  ): TypeExpression(NodeKind::AppTypeExpression),
     Op(Op),
     Args(Args) {}

  Token* getFirstToken() const override;
  Token* getLastToken() const override;

  static constexpr const NodeKind Kind = NodeKind::AppTypeExpression;

};

class VarTypeExpression : public TypeExpression {
public:

  Identifier* Name;

  inline VarTypeExpression(Identifier* Name):
    TypeExpression(NodeKind::VarTypeExpression), Name(Name) {}

  Token* getFirstToken() const override;
  Token* getLastToken() const override;

  static constexpr const NodeKind Kind = NodeKind::VarTypeExpression;

};

class NestedTypeExpression : public TypeExpression {
public:

  class LParen* LParen;
  TypeExpression* TE;
  class RParen* RParen;

  inline NestedTypeExpression(
    class LParen* LParen,
    TypeExpression* TE,
    class RParen* RParen
  ): TypeExpression(NodeKind::NestedTypeExpression),
     LParen(LParen),
     TE(TE),
     RParen(RParen) {}

  Token* getFirstToken() const override;
  Token* getLastToken() const override;

  static constexpr const NodeKind Kind = NodeKind::NestedTypeExpression;

};

class TupleTypeExpression : public TypeExpression {
public:

  class LParen* LParen;
  std::vector<std::tuple<TypeExpression*, Comma*>> Elements;
  class RParen* RParen;

  inline TupleTypeExpression(
    class LParen* LParen,
    std::vector<std::tuple<TypeExpression*, Comma*>> Elements,
    class RParen* RParen
  ): TypeExpression(NodeKind::TupleTypeExpression),
     LParen(LParen),
     Elements(Elements),
     RParen(RParen) {}

  Token* getFirstToken() const override;
  Token* getLastToken() const override;

  static constexpr const NodeKind Kind = NodeKind::TupleTypeExpression;

};


class Pattern : public Node {
protected:

  inline Pattern(NodeKind Type):
    Node(Type) {}

  static bool classof(const Node* N) {
    return N->getKind() == NodeKind::BindPattern
        || N->getKind() == NodeKind::ListPattern
        || N->getKind() == NodeKind::LiteralPattern
        || N->getKind() == NodeKind::NamedRecordPattern
        || N->getKind() == NodeKind::NamedTuplePattern
        || N->getKind() == NodeKind::NestedPattern
        || N->getKind() == NodeKind::RecordPattern
        || N->getKind() == NodeKind::TuplePattern;
  }

};

class BindPattern : public Pattern {
public:

  Identifier* Name;

  BindPattern(
    Identifier* Name
  ): Pattern(NodeKind::BindPattern),
     Name(Name) {}

  Token* getFirstToken() const override;
  Token* getLastToken() const override;

  static constexpr const NodeKind Kind = NodeKind::BindPattern;

};

class LiteralPattern : public Pattern {
public:

  class Literal* Literal;

  LiteralPattern(class Literal* Literal):
    Pattern(NodeKind::LiteralPattern),
    Literal(Literal) {}

  Token* getFirstToken() const override;
  Token* getLastToken() const override;

  static constexpr const NodeKind Kind = NodeKind::LiteralPattern;

};

class RecordPatternField : public Node {
public:

  class DotDot* DotDot;
  Identifier* Name;
  class Equals* Equals;
  class Pattern* Pattern;

  inline RecordPatternField(
    class DotDot* DotDot,
    Identifier* Name,
    class Equals* Equals,
    class Pattern* Pattern
  ): Node(NodeKind::RecordPatternField),
     DotDot(DotDot),
     Name(Name),
     Equals(Equals),
     Pattern(Pattern) {}

  inline RecordPatternField(
    Identifier* Name,
    class Equals* Equals,
    class Pattern* Pattern
  ): RecordPatternField(nullptr, Name, Equals, Pattern) {}

  inline RecordPatternField(
    class DotDot* DotDot
  ): RecordPatternField(DotDot, nullptr, nullptr, nullptr) {}

  inline RecordPatternField(
    class DotDot* DotDot,
    class Pattern* Pattern
  ): RecordPatternField(DotDot, nullptr, nullptr, Pattern) {}

  inline RecordPatternField(
    Identifier* Name
  ): RecordPatternField(nullptr, Name, nullptr, nullptr) {}

  Token* getFirstToken() const override;
  Token* getLastToken() const override;

  static constexpr const NodeKind Kind = NodeKind::RecordPatternField;

};

class RecordPattern : public Pattern {
public:

  class LBrace* LBrace;
  std::vector<std::tuple<RecordPatternField*, Comma*>> Fields;
  class RBrace* RBrace;

  inline RecordPattern(
    class LBrace* LBrace,
    std::vector<std::tuple<RecordPatternField*, Comma*>> Fields,
    class RBrace* RBrace
  ): Pattern(NodeKind::RecordPattern),
     LBrace(LBrace),
     Fields(Fields),
     RBrace(RBrace) {}

  Token* getFirstToken() const override;
  Token* getLastToken() const override;

  static constexpr const NodeKind Kind = NodeKind::RecordPattern;

};

class NamedRecordPattern : public Pattern {
public:

  std::vector<std::tuple<IdentifierAlt*, Dot*>> ModulePath;
  IdentifierAlt* Name;
  class LBrace* LBrace;
  std::vector<std::tuple<RecordPatternField*, Comma*>> Fields;
  class RBrace* RBrace;

  inline NamedRecordPattern(
    std::vector<std::tuple<IdentifierAlt*, Dot*>> ModulePath,
    IdentifierAlt* Name,
    class LBrace* LBrace,
    std::vector<std::tuple<RecordPatternField*, Comma*>> Fields,
    class RBrace* RBrace
  ): Pattern(NodeKind::NamedRecordPattern),
     ModulePath(ModulePath),
     Name(Name),
     LBrace(LBrace),
     Fields(Fields),
     RBrace(RBrace) {}

  Token* getFirstToken() const override;
  Token* getLastToken() const override;

  static constexpr const NodeKind Kind = NodeKind::NamedRecordPattern;

};

class NamedTuplePattern : public Pattern {
public:

  IdentifierAlt* Name;
  std::vector<Pattern*> Patterns;

  inline NamedTuplePattern(
    IdentifierAlt* Name,
    std::vector<Pattern*> Patterns
  ): Pattern(NodeKind::NamedTuplePattern),
     Name(Name),
     Patterns(Patterns) {}

  Token* getFirstToken() const override;
  Token* getLastToken() const override;

  static constexpr const NodeKind Kind = NodeKind::NamedTuplePattern;

};

class TuplePattern : public Pattern {
public:

  class LParen* LParen;
  std::vector<std::tuple<Pattern*, Comma*>> Elements;
  class RParen* RParen;

  inline TuplePattern(
    class LParen* LParen,
    std::vector<std::tuple<Pattern*, Comma*>> Elements,
    class RParen* RParen
  ): Pattern(NodeKind::TuplePattern),
     LParen(LParen),
     Elements(Elements),
     RParen(RParen) {}

  Token* getFirstToken() const override;
  Token* getLastToken() const override;

  static constexpr const NodeKind Kind = NodeKind::TuplePattern;

};

class NestedPattern : public Pattern {
public:

  class LParen* LParen;
  Pattern* P;
  class RParen* RParen;

  inline NestedPattern(
    class LParen* LParen,
    Pattern* P,
    class RParen* RParen
  ): Pattern(NodeKind::NestedPattern),
     LParen(LParen),
     P(P),
     RParen(RParen) {}

  Token* getFirstToken() const override;
  Token* getLastToken() const override;

  static constexpr const NodeKind Kind = NodeKind::NestedPattern;

};

class ListPattern : public Pattern {
public:

  class LBracket* LBracket;
  std::vector<std::tuple<Pattern*, Comma*>> Elements;
  class RBracket* RBracket;

  inline ListPattern(
    class LBracket* LBracket,
    std::vector<std::tuple<Pattern*, Comma*>> Elements,
    class RBracket* RBracket
  ): Pattern(NodeKind::ListPattern),
     LBracket(LBracket),
     Elements(Elements),
     RBracket(RBracket) {}

  Token* getFirstToken() const override;
  Token* getLastToken() const override;

  static constexpr const NodeKind Kind = NodeKind::ListPattern;

};

class Expression : public TypedNode, public AnnotationContainer {
protected:

  inline Expression(NodeKind Kind, std::vector<Annotation*> Annotations = {}):
    TypedNode(Kind), AnnotationContainer(Annotations) {}

public:

  static bool classof(const Node* N) {
    return N->getKind() == NodeKind::ReferenceExpression
        || N->getKind() == NodeKind::NestedExpression
        || N->getKind() == NodeKind::CallExpression
        || N->getKind() == NodeKind::FunctionExpression
        || N->getKind() == NodeKind::TupleExpression
        || N->getKind() == NodeKind::InfixExpression
        || N->getKind() == NodeKind::RecordExpression
        || N->getKind() == NodeKind::MatchExpression
        || N->getKind() == NodeKind::BlockExpression
        || N->getKind() == NodeKind::MemberExpression
        || N->getKind() == NodeKind::LiteralExpression
        || N->getKind() == NodeKind::BlockExpression
        || N->getKind() == NodeKind::IfExpression
        || N->getKind() == NodeKind::ReturnExpression
        || N->getKind() == NodeKind::PrefixExpression;
  }

};

class ReferenceExpression : public Expression {
public:

  std::vector<std::tuple<IdentifierAlt*, Dot*>> ModulePath;
  Symbol Name;

  inline ReferenceExpression(
    std::vector<Annotation*> Annotations,
    std::vector<std::tuple<IdentifierAlt*, Dot*>> ModulePath,
    Symbol Name
  ): Expression(NodeKind::ReferenceExpression, Annotations),
     ModulePath(ModulePath),
     Name(Name) {}

  inline ReferenceExpression(
    std::vector<std::tuple<IdentifierAlt*, Dot*>> ModulePath,
    Symbol Name
  ): ReferenceExpression({}, ModulePath, Name) {}

  inline ByteString getNameAsString() const noexcept {
    return Name.getCanonicalText();
  }

  Token* getFirstToken() const override;
  Token* getLastToken() const override;

  SymbolPath getSymbolPath() const;

  static constexpr const NodeKind Kind = NodeKind::ReferenceExpression;

};

class MatchCase : public Node { 

  Scope* TheScope = nullptr;

public:

  class Pattern* Pattern;
  class RArrowAlt* RArrowAlt;
  class Expression* Expression;

  inline MatchCase(
    class Pattern* Pattern,
    class RArrowAlt* RArrowAlt,
    class Expression* Expression
  ): Node(NodeKind::MatchCase),
     Pattern(Pattern),
     RArrowAlt(RArrowAlt),
     Expression(Expression) {}

  Token* getFirstToken() const override;
  Token* getLastToken() const override;

  inline Scope* getScope() override {
    if (TheScope == nullptr) {
      TheScope = new Scope(this);
    }
    return TheScope;
  }

  static constexpr const NodeKind Kind = NodeKind::MatchCase; 

};

class MatchExpression : public Expression {
public:

  class MatchKeyword* MatchKeyword;
  Expression* Value;
  class BlockStart* BlockStart;
  std::vector<MatchCase*> Cases;
     
  inline MatchExpression(
    std::vector<Annotation*> Annotations,
    class MatchKeyword* MatchKeyword,
    Expression* Value,
    class BlockStart* BlockStart,
    std::vector<MatchCase*> Cases
  ): Expression(NodeKind::MatchExpression, Annotations),
     MatchKeyword(MatchKeyword),
     Value(Value),
     BlockStart(BlockStart),
     Cases(Cases) {}

  inline MatchExpression(
    class MatchKeyword* MatchKeyword,
    Expression* Value,
    class BlockStart* BlockStart,
    std::vector<MatchCase*> Cases
  ): MatchExpression({}, MatchKeyword, Value, BlockStart, Cases) {}

  Token* getFirstToken() const override;
  Token* getLastToken() const override;

  inline bool hasValue() const {
    return Value;
  }

  Expression* getValue() const {
    return Value;
  }

  static constexpr const NodeKind Kind = NodeKind::MatchExpression;

};

class BlockExpression : public Expression {
public:

  class DoKeyword* DoKeyword;
  class BlockStart* BlockStart;
  std::vector<Node*> Elements;

  inline BlockExpression(
    std::vector<Annotation*> Annotations,
    class DoKeyword* DoKeyword,
    class BlockStart* BlockStart,
    std::vector<Node*> Elements
  ): Expression(NodeKind::BlockExpression, Annotations),
     DoKeyword(DoKeyword),
     BlockStart(BlockStart),
     Elements(Elements) {}

  inline BlockExpression(
    class DoKeyword* DoKeyword,
    class BlockStart* BlockStart,
    std::vector<Node*> Elements
  ): BlockExpression({}, DoKeyword, BlockStart, Elements) {}

  Token* getFirstToken() const override;
  Token* getLastToken() const override;

  static constexpr const NodeKind Kind = NodeKind::BlockExpression;

};

class MemberExpression : public Expression {
public:

  Expression* E;
  class Dot* Dot;
  Token* Name;

  inline MemberExpression(
    std::vector<Annotation*> Annotations,
    class Expression* E,
    class Dot* Dot,
    Token* Name
  ): Expression(NodeKind::MemberExpression, Annotations),
     E(E),
     Dot(Dot),
     Name(Name) {}

  inline MemberExpression(
    Expression* E,
    class Dot* Dot,
    Token* Name
  ): MemberExpression({}, E, Dot, Name) {}

  Token* getFirstToken() const override;
  Token* getLastToken() const override;

  inline Expression* getExpression() const {
    return E;
  }

  static constexpr const NodeKind Kind = NodeKind::MemberExpression;

};

class TupleExpression : public Expression {
public:

  class LParen* LParen;
  std::vector<std::tuple<Expression*, Comma*>> Elements;
  class RParen* RParen;

  inline TupleExpression(
    std::vector<Annotation*> Annotations,
    class LParen* LParen,
    std::vector<std::tuple<Expression*, Comma*>> Elements,
    class RParen* RParen
  ): Expression(NodeKind::TupleExpression, Annotations),
     LParen(LParen),
     Elements(Elements),
     RParen(RParen) {}

  inline TupleExpression(
    class LParen* LParen,
    std::vector<std::tuple<Expression*, Comma*>> Elements,
    class RParen* RParen
  ): TupleExpression({}, LParen, Elements, RParen) {}

  Token* getFirstToken() const override;
  Token* getLastToken() const override;

  static constexpr const NodeKind Kind = NodeKind::TupleExpression;

};

class NestedExpression : public Expression {
public:

  class LParen* LParen;
  Expression* Inner;
  class RParen* RParen;

  inline NestedExpression(
    std::vector<Annotation*> Annotations,
    class LParen* LParen,
    Expression* Inner,
    class RParen* RParen
  ): Expression(NodeKind::NestedExpression, Annotations),
     LParen(LParen),
     Inner(Inner),
     RParen(RParen) {}


  inline NestedExpression(
    class LParen* LParen,
    Expression* Inner,
    class RParen* RParen
  ): NestedExpression({}, LParen, Inner, RParen) {}

  Token* getFirstToken() const override;
  Token* getLastToken() const override;

  static constexpr const NodeKind Kind = NodeKind::NestedExpression;

};

class LiteralExpression : public Expression {
public:

  Literal* Token;

  inline LiteralExpression(
    std::vector<Annotation*> Annotations,
    Literal* Token
  ): Expression(NodeKind::LiteralExpression, Annotations),
     Token(Token) {}

  inline LiteralExpression(Literal* Token):
    LiteralExpression({}, Token) {}

  inline ByteString getAsText() {
    ZEN_ASSERT(Token->getKind() == NodeKind::StringLiteral);
    return static_cast<StringLiteral*>(Token)->Text;
  }

  inline int getAsInt() {
    ZEN_ASSERT(Token->getKind() == NodeKind::IntegerLiteral);
    return static_cast<IntegerLiteral*>(Token)->asInt();
  }

  class Token* getFirstToken() const override;
  class Token* getLastToken() const override;

  static constexpr const NodeKind Kind = NodeKind::LiteralExpression;

};

class CallExpression : public Expression {
public:

  Expression* Function;
  std::vector<Expression*> Args;

  inline CallExpression(
    std::vector<Annotation*> Annotations,
    Expression* Function,
    std::vector<Expression*> Args
  ): Expression(NodeKind::CallExpression, Annotations),
     Function(Function),
     Args(Args) {}

  inline CallExpression(
    Expression* Function,
    std::vector<Expression*> Args
  ): CallExpression({}, Function, Args) {}

  Token* getFirstToken() const override;
  Token* getLastToken() const override;

  static constexpr const NodeKind Kind = NodeKind::CallExpression;

};

class FunctionExpression : public Expression {

public:

  Backslash* Backslash;
  std::vector<Pattern*> Params;
  RArrow* RArrow;
  Expression* E;

  inline FunctionExpression(
    std::vector<Annotation*> Annotations,
    class Backslash* Backslash,
    std::vector<Pattern*> Params,
    class RArrow* RArrow,
    class Expression* E
  ): Expression(NodeKind::FunctionExpression, Annotations),
     Backslash(Backslash),
     Params(Params),
     RArrow(RArrow),
     E(E) {}

  inline FunctionExpression(
    class Backslash* Backslash,
    std::vector<Pattern*> Params,
    class RArrow* RArrow,
    class Expression* Expression
  ): FunctionExpression({}, Backslash, Params, RArrow, Expression) {}

  std::size_t countParams() {
    return Params.size();
  }

  auto getParameters() {
    return zen::make_iterator_range(Params);
  }

  Expression* getExpression() const {
    return E;
  }

  Token* getFirstToken() const override;
  Token* getLastToken() const override;

  static constexpr const NodeKind Kind = NodeKind::FunctionExpression;

};

class InfixExpression : public Expression {
public:

  Expression* Left;
  Operator Operator;
  Expression* Right;

  inline InfixExpression(
    std::vector<Annotation*> Annotations,
    Expression* Left,
    class Operator Operator,
    Expression* Right
  ): Expression(NodeKind::InfixExpression, Annotations),
     Left(Left),
     Operator(Operator),
     Right(Right) {}

  inline InfixExpression(
    Expression* Left,
    class Operator Operator,
    Expression* Right
  ): InfixExpression({}, Left, Operator, Right) {}

  Token* getFirstToken() const override;
  Token* getLastToken() const override;

  static constexpr const NodeKind Kind = NodeKind::InfixExpression;

};

class PrefixExpression : public Expression {
public:

  Token* Operator;
  Expression* Argument;

  inline PrefixExpression(
    std::vector<Annotation*> Annotations,
    Token* Operator,
    Expression* Argument
  ): Expression(NodeKind::PrefixExpression, Annotations),
     Operator(Operator),
     Argument(Argument) {}

  inline PrefixExpression(
    Token* Operator,
    Expression* Argument
  ): PrefixExpression({}, Operator, Argument) {}

  Token* getFirstToken() const override;
  Token* getLastToken() const override;

  static constexpr const NodeKind Kind = NodeKind::PrefixExpression;

};

class RecordExpressionField : public Node {
public:

  Identifier* Name;
  class Equals* Equals;
  Expression* E;

  inline RecordExpressionField(
    Identifier* Name,
    class Equals* Equals,
    Expression* E
  ): Node(NodeKind::RecordExpressionField),
     Name(Name),
     Equals(Equals),
     E(E) {}

  Token* getFirstToken() const override;
  Token* getLastToken() const override;

  bool hasExpression() const {
    return E; 
  }

  inline Expression* getExpression() const {
    ZEN_ASSERT(E != nullptr);
    return E;
  }

  static constexpr const NodeKind Kind = NodeKind::RecordExpressionField;

};

class RecordExpression : public Expression {
public:

  class LBrace* LBrace;
  std::vector<std::tuple<RecordExpressionField*, Comma*>> Fields;
  class RBrace* RBrace;

  inline RecordExpression(
    std::vector<Annotation*> Annotations,
    class LBrace* LBrace,
    std::vector<std::tuple<RecordExpressionField*, Comma*>> Fields,
    class RBrace* RBrace
  ): Expression(NodeKind::RecordExpression, Annotations),
     LBrace(LBrace),
     Fields(Fields),
     RBrace(RBrace) {}

  inline RecordExpression(
    class LBrace* LBrace,
    std::vector<std::tuple<RecordExpressionField*, Comma*>> Fields,
    class RBrace* RBrace
  ): RecordExpression({}, LBrace, Fields, RBrace) {}

  Token* getFirstToken() const override;
  Token* getLastToken() const override;

  static constexpr const NodeKind Kind = NodeKind::RecordExpression;

};

class IfExpressionPart : public Node, public AnnotationContainer {
public:

  Token* Keyword;
  Expression* Test;
  class BlockStart* BlockStart;
  std::vector<Node*> Elements;

  inline IfExpressionPart(
    std::vector<Annotation*> Annotations,
    Token* Keyword,
    Expression* Test,
    class BlockStart* BlockStart,
    std::vector<Node*> Elements
  ): Node(NodeKind::IfExpressionPart),
     AnnotationContainer(Annotations),
     Keyword(Keyword),
     Test(Test),
     BlockStart(BlockStart),
     Elements(Elements) {}

  inline IfExpressionPart(
    Token* Keyword,
    Expression* Test,
    class BlockStart* BlockStart,
    std::vector<Node*> Elements
  ): IfExpressionPart({}, Keyword, Test, BlockStart, Elements) {}

  Token* getFirstToken() const override;
  Token* getLastToken() const override;

  static constexpr const NodeKind Kind = NodeKind::IfExpressionPart;

};

class IfExpression : public Expression {
public:

  std::vector<IfExpressionPart*> Parts;

  inline IfExpression(
    std::vector<Annotation*> Annotations,
    std::vector<IfExpressionPart*> Parts
  ): Expression(NodeKind::IfExpression, Annotations),
     Parts(Parts) {}


  inline IfExpression(std::vector<IfExpressionPart*> Parts):
    IfExpression({}, Parts) {}

  Token* getFirstToken() const override;
  Token* getLastToken() const override;

  static constexpr const NodeKind Kind = NodeKind::IfExpression;

};

class ReturnExpression : public Expression {
public:

  class ReturnKeyword* ReturnKeyword;
  Expression* E;

  inline ReturnExpression(
    std::vector<Annotation*> Annotations,
    class ReturnKeyword* ReturnKeyword,
    Expression* E
  ): Expression(NodeKind::ReturnExpression, Annotations),
     ReturnKeyword(ReturnKeyword),
     E(E) {}

  inline ReturnExpression(
    class ReturnKeyword* ReturnKeyword,
    class Expression* Expression
  ): ReturnExpression({}, ReturnKeyword, Expression) {}

  Token* getFirstToken() const override;
  Token* getLastToken() const override;

  bool hasExpression() const {
    return E;
  }

  Expression* getExpression() {
    return E;
  }

  static constexpr const NodeKind Kind = NodeKind::ReturnExpression;

};

class TypeAssert : public Node {
public:

  class Colon* Colon;
  class TypeExpression* TypeExpression;

  TypeAssert(
    class Colon* Colon,
    class TypeExpression* TypeExpression
  ): Node(NodeKind::TypeAssert),
     Colon(Colon),
     TypeExpression(TypeExpression) {}

  Token* getFirstToken() const override;
  Token* getLastToken() const override;

  static constexpr const NodeKind Kind = NodeKind::TypeAssert;

};

class Parameter : public Node {
public:

  Parameter(
    class Pattern* Pattern,
    class TypeAssert* TypeAssert
  ): Node(NodeKind::Parameter),
     Pattern(Pattern),
     TypeAssert(TypeAssert) {}

  class Pattern* Pattern;
  class TypeAssert* TypeAssert;

  Token* getFirstToken() const override;
  Token* getLastToken() const override;

  static constexpr const NodeKind Kind = NodeKind::Parameter;

};

class LetBody : public Node {
public:

  LetBody(NodeKind Type): Node(Type) {}

};

class LetBlockBody : public LetBody {
public:

  class BlockStart* BlockStart;
  std::vector<Node*> Elements;

  LetBlockBody(
    class BlockStart* BlockStart,
    std::vector<Node*> Elements
  ): LetBody(NodeKind::LetBlockBody),
     BlockStart(BlockStart),
     Elements(Elements) {}

  Token* getFirstToken() const override;
  Token* getLastToken() const override;

  static constexpr const NodeKind Kind = NodeKind::LetBlockBody;

};

class LetExprBody : public LetBody {
public:

  class Equals* Equals;
  class Expression* Expression;

  LetExprBody(
    class Equals* Equals,
    class Expression* Expression
  ): LetBody(NodeKind::LetExprBody),
     Equals(Equals),
     Expression(Expression) {}

  Token* getFirstToken() const override;
  Token* getLastToken() const override;

  static constexpr const NodeKind Kind = NodeKind::LetExprBody;

};

class Declaration : public TypedNode, public AnnotationContainer {

  std::optional<TypeScheme> Scm;

protected:


  inline Declaration(NodeKind Kind, std::vector<Annotation*> Annotations = {}):
    TypedNode(Kind), AnnotationContainer(Annotations) {}

public:

  static bool classof(const Node* N) {
    return N->getKind() == NodeKind::VariantDeclaration
        || N->getKind() == NodeKind::RecordDeclaration
        || N->getKind() == NodeKind::VariantDeclaration
        || N->getKind() == NodeKind::PrefixFunctionDeclaration
        || N->getKind() == NodeKind::InfixFunctionDeclaration
        || N->getKind() == NodeKind::SuffixFunctionDeclaration
        || N->getKind() == NodeKind::NamedFunctionDeclaration;
  }

  const TypeScheme& getScheme() const {
    ZEN_ASSERT(Scm.has_value());
    return *Scm;
  }

  bool hasScheme() const {
    return Scm.has_value();
  }

  void setScheme(TypeScheme NewScm) {
    Scm = NewScm;
  }

};

class FunctionDeclaration : public Declaration {

  Scope* TheScope = nullptr;

public:

  bool IsCycleActive = false;
  bool Visited = false;

  FunctionDeclaration(NodeKind Kind, std::vector<Annotation*> Annotations = {}):
    Declaration(Kind, Annotations) {}

  virtual bool isPublic() const = 0;

  virtual bool isForeign() const = 0;

  virtual ByteString  getNameAsString() const = 0;

  virtual std::vector<Parameter*> getParams() const = 0;

  virtual TypeAssert* getTypeAssert() const = 0;

  bool hasTypeAssert() const {
    return getTypeAssert();
  }

  virtual LetBody* getBody() const = 0;

  bool hasBody() const {
    return getBody();
  }

  inline Scope* getScope() override {
    if (TheScope == nullptr) {
      TheScope = new Scope(this);
    }
    return TheScope;
  }

  bool isInstance() const noexcept {
    return Parent->getKind() == NodeKind::InstanceDeclaration;
  }

  bool isClass() const noexcept {
    return Parent->getKind() == NodeKind::ClassDeclaration;
  }

  static bool classof(const Node* N) {
    return N->getKind() == NodeKind::PrefixFunctionDeclaration
        || N->getKind() == NodeKind::InfixFunctionDeclaration
        || N->getKind() == NodeKind::SuffixFunctionDeclaration
        || N->getKind() == NodeKind::NamedFunctionDeclaration;
  }

};

class PrefixFunctionDeclaration : public FunctionDeclaration {
public:

  class PubKeyword* PubKeyword;
  class ForeignKeyword* ForeignKeyword;
  class FnKeyword* FnKeyword;
  class Operator Name;
  Parameter* Param;
  class TypeAssert* TypeAssert;
  LetBody* Body;

  PrefixFunctionDeclaration(
    class std::vector<Annotation*> Annotations,
    class PubKeyword* PubKeyword,
    class ForeignKeyword* ForeignKeyword,
    class FnKeyword* FnKeyword,
    Operator Name,
    Parameter* Param,
    class TypeAssert* TypeAssert,
    LetBody* Body
  ): FunctionDeclaration(NodeKind::PrefixFunctionDeclaration, Annotations),
     PubKeyword(PubKeyword),
     ForeignKeyword(ForeignKeyword),
     FnKeyword(FnKeyword),
     Name(Name),
     Param(Param),
     TypeAssert(TypeAssert),
     Body(Body) {}

  bool isPublic() const override {
    return PubKeyword != nullptr;
  }

  bool isForeign() const override {
    return ForeignKeyword != nullptr;
  }

  ByteString getNameAsString() const override {
    return Name.getCanonicalText();
  }

  std::vector<Parameter*> getParams() const override {
    return { Param };
  }

  class TypeAssert* getTypeAssert() const override {
    return TypeAssert;
  }

  LetBody* getBody() const override {
    return Body;
  }

  Token* getFirstToken() const override;
  Token* getLastToken() const override;

  static constexpr const NodeKind Kind = NodeKind::PrefixFunctionDeclaration;

};

class SuffixFunctionDeclaration : public FunctionDeclaration {
public:

  class PubKeyword* PubKeyword;
  class ForeignKeyword* ForeignKeyword;
  class FnKeyword* FnKeyword;
  Parameter* Param;
  class Operator Name;
  class TypeAssert* TypeAssert;
  LetBody* Body;

  SuffixFunctionDeclaration(
    class std::vector<Annotation*> Annotations,
    class PubKeyword* PubKeyword,
    class ForeignKeyword* ForeignKeyword,
    class FnKeyword* FnKeyword,
    Parameter* Param,
    Operator Name,
    class TypeAssert* TypeAssert,
    LetBody* Body
  ): FunctionDeclaration(NodeKind::SuffixFunctionDeclaration, Annotations),
     PubKeyword(PubKeyword),
     ForeignKeyword(ForeignKeyword),
     FnKeyword(FnKeyword),
     Name(Name),
     Param(Param),
     TypeAssert(TypeAssert),
     Body(Body) {}

  bool isPublic() const override {
    return PubKeyword != nullptr;
  }

  bool isForeign() const override {
    return ForeignKeyword != nullptr;
  }

  ByteString getNameAsString() const override {
    return Name.getCanonicalText();
  }

  std::vector<Parameter*> getParams() const override {
    return { Param };
  }

  class TypeAssert* getTypeAssert() const override {
    return TypeAssert;
  }

  LetBody* getBody() const override {
    return Body;
  }

  Token* getFirstToken() const override;
  Token* getLastToken() const override;

  static constexpr const NodeKind Kind = NodeKind::SuffixFunctionDeclaration;

};

class InfixFunctionDeclaration : public FunctionDeclaration {
public:

  class PubKeyword* PubKeyword;
  class ForeignKeyword* ForeignKeyword;
  class FnKeyword* FnKeyword;
  Parameter* Left;
  class Operator Name;
  Parameter* Right;
  class TypeAssert* TypeAssert;
  LetBody* Body;

  InfixFunctionDeclaration(
    class std::vector<Annotation*> Annotations,
    class PubKeyword* PubKeyword,
    class ForeignKeyword* ForeignKeyword,
    class FnKeyword* FnKeyword,
    Parameter* Left,
    class Operator Name,
    Parameter* Right,
    class TypeAssert* TypeAssert,
    LetBody* Body
  ): FunctionDeclaration(NodeKind::InfixFunctionDeclaration, Annotations),
     PubKeyword(PubKeyword),
     ForeignKeyword(ForeignKeyword),
     FnKeyword(FnKeyword),
     Left(Left),
     Name(Name),
     Right(Right),
     TypeAssert(TypeAssert),
     Body(Body) {}

  bool isPublic() const override {
    return PubKeyword != nullptr;
  }

  bool isForeign() const override {
    return ForeignKeyword != nullptr;
  }

  ByteString getNameAsString() const override {
    return Name.getCanonicalText();
  }

  std::vector<Parameter*> getParams() const override {
    return { Left, Right };
  }

  class TypeAssert* getTypeAssert() const override {
    return TypeAssert;
  }

  LetBody* getBody() const override {
    return Body;
  }

  Token* getFirstToken() const override;
  Token* getLastToken() const override;

  static constexpr const NodeKind Kind = NodeKind::InfixFunctionDeclaration;

};

class NamedFunctionDeclaration : public FunctionDeclaration {
public:

  class PubKeyword* PubKeyword;
  class ForeignKeyword* ForeignKeyword;
  class FnKeyword* FnKeyword;
  class Symbol Name;
  std::vector<Parameter*> Params;
  class TypeAssert* TypeAssert;
  LetBody* Body;

  NamedFunctionDeclaration(
    class std::vector<Annotation*> Annotations,
    class PubKeyword* PubKeyword,
    class ForeignKeyword* ForeignKeyword,
    class FnKeyword* FnKeyword,
    class Symbol Name,
    std::vector<Parameter*> Params,
    class TypeAssert* TypeAssert,
    LetBody* Body
  ): FunctionDeclaration(NodeKind::NamedFunctionDeclaration, Annotations),
     PubKeyword(PubKeyword),
     ForeignKeyword(ForeignKeyword),
     FnKeyword(FnKeyword),
     Name(Name),
     Params(Params),
     TypeAssert(TypeAssert),
     Body(Body) {}

  bool isPublic() const override {
    return PubKeyword != nullptr;
  }

  bool isForeign() const override {
    return ForeignKeyword != nullptr;
  }

  ByteString getNameAsString() const override {
    return Name.getCanonicalText();
  }

  std::vector<Parameter*> getParams() const override {
    return Params;
  }

  class TypeAssert* getTypeAssert() const override {
    return TypeAssert;
  }

  LetBody* getBody() const override {
    return Body;
  }
  Token* getFirstToken() const override;
  Token* getLastToken() const override;

  static constexpr const NodeKind Kind = NodeKind::NamedFunctionDeclaration;

};

class VariableDeclaration : public Declaration {
public:

  class PubKeyword* PubKeyword;
  class LetKeyword* LetKeyword;
  class MutKeyword* MutKeyword;
  class Pattern* Pattern;
  class TypeAssert* TypeAssert;
  LetBody* Body;

  VariableDeclaration(
    class std::vector<Annotation*> Annotations,
    class PubKeyword* PubKeyword,
    class LetKeyword* LetKeyword,
    class MutKeyword* MutKeyword,
    class Pattern* Pattern,
    class TypeAssert* TypeAssert,
    LetBody* Body
  ): Declaration(NodeKind::VariableDeclaration, Annotations),
     PubKeyword(PubKeyword),
     LetKeyword(LetKeyword),
     MutKeyword(MutKeyword),
     Pattern(Pattern),
     TypeAssert(TypeAssert),
     Body(Body) {}

  Symbol getName() const noexcept {
    ZEN_ASSERT(Pattern->getKind() == NodeKind::BindPattern);
    return static_cast<BindPattern*>(Pattern)->Name;
  }

  ByteString getNameAsString() const noexcept {
    return getName().getCanonicalText();
  }

  Token* getFirstToken() const override;
  Token* getLastToken() const override;

  bool hasExpression() const {
    return Body;
  }

  Expression* getExpression() {
    ZEN_ASSERT(Body->getKind() == NodeKind::LetExprBody);
    return static_cast<LetExprBody*>(Body)->Expression;
  }

  static constexpr const NodeKind Kind = NodeKind::VariableDeclaration;

};

class InstanceDeclaration : public Node {
public:

  class InstanceKeyword* InstanceKeyword;
  IdentifierAlt* Name;
  std::vector<TypeExpression*> TypeExps;
  class BlockStart* BlockStart;
  std::vector<Node*> Elements;

  InstanceDeclaration(
    class InstanceKeyword* InstanceKeyword,
    IdentifierAlt* Name,
    std::vector<TypeExpression*> TypeExps,
    class BlockStart* BlockStart,
    std::vector<Node*> Elements
  ): Node(NodeKind::InstanceDeclaration),
     InstanceKeyword(InstanceKeyword),
     Name(Name),
     TypeExps(TypeExps),
     BlockStart(BlockStart),
     Elements(Elements) {}

  Token* getFirstToken() const override;
  Token* getLastToken() const override;

  static constexpr const NodeKind Kind = NodeKind::InstanceDeclaration;

};

class ClassDeclaration : public Node {
public:

  class PubKeyword* PubKeyword;
  class ClassKeyword* ClassKeyword;
  IdentifierAlt* Name;
  std::vector<VarTypeExpression*> TypeVars;
  class BlockStart* BlockStart;
  std::vector<Node*> Elements;

  ClassDeclaration(
    class PubKeyword* PubKeyword,
    class ClassKeyword* ClassKeyword,
    IdentifierAlt* Name,
    std::vector<VarTypeExpression*> TypeVars,
    class BlockStart* BlockStart,
    std::vector<Node*> Elements
  ): Node(NodeKind::ClassDeclaration),
     PubKeyword(PubKeyword),
     ClassKeyword(ClassKeyword),
     Name(Name),
     TypeVars(TypeVars),
     BlockStart(BlockStart),
     Elements(Elements) {}

  Token* getFirstToken() const override;
  Token* getLastToken() const override;

  static constexpr const NodeKind Kind = NodeKind::ClassDeclaration;

};

class RecordDeclarationField : public Node {
public:

  RecordDeclarationField(
    Identifier* Name,
    class Colon* Colon,
    class TypeExpression* TypeExpression
  ): Node(NodeKind::RecordDeclarationField),
     Name(Name),
     Colon(Colon),
     TypeExpression(TypeExpression) {}

  Identifier* Name;
  class Colon* Colon;
  class TypeExpression* TypeExpression;

  Token* getFirstToken() const override;
  Token* getLastToken() const override;

  static constexpr const NodeKind Kind = NodeKind::RecordDeclarationField;

};

class RecordDeclaration : public Declaration {
public:

  class PubKeyword* PubKeyword;
  class StructKeyword* StructKeyword;
  IdentifierAlt* Name;
  std::vector<VarTypeExpression*> Vars;
  class BlockStart* BlockStart;
  std::vector<RecordDeclarationField*> Fields;

  RecordDeclaration(
    class PubKeyword* PubKeyword,
    class StructKeyword* StructKeyword,
    IdentifierAlt* Name,
    std::vector<VarTypeExpression*> Vars,
    class BlockStart* BlockStart,
    std::vector<RecordDeclarationField*> Fields
  ): Declaration(NodeKind::RecordDeclaration),
     PubKeyword(PubKeyword),
     StructKeyword(StructKeyword),
     Name(Name),
     Vars(Vars),
     BlockStart(BlockStart),
     Fields(Fields) {}

  Token* getFirstToken() const override;
  Token* getLastToken() const override;

  static constexpr const NodeKind Kind = NodeKind::RecordDeclaration;

};

class VariantDeclarationMember : public Node {
public:

  inline VariantDeclarationMember(NodeKind Kind):
    Node(Kind) {}

};

class TupleVariantDeclarationMember : public VariantDeclarationMember {
public:

  IdentifierAlt* Name;
  std::vector<TypeExpression*> Elements;

  inline TupleVariantDeclarationMember(
    IdentifierAlt* Name,
    std::vector<TypeExpression*> Elements
  ): VariantDeclarationMember(NodeKind::TupleVariantDeclarationMember),
     Name(Name),
     Elements(Elements) {}

  Token* getFirstToken() const override;
  Token* getLastToken() const override;

  static constexpr const NodeKind Kind = NodeKind::TupleVariantDeclarationMember;

};

class RecordVariantDeclarationMember : public VariantDeclarationMember {
public:

  IdentifierAlt* Name;
  class BlockStart* BlockStart;
  std::vector<RecordDeclarationField*> Fields;

  inline RecordVariantDeclarationMember(
    IdentifierAlt* Name,
    class BlockStart* BlockStart,
    std::vector<RecordDeclarationField*> Fields
  ): VariantDeclarationMember(NodeKind::RecordVariantDeclarationMember),
     Name(Name),
     BlockStart(BlockStart),
     Fields(Fields) {}

  Token* getFirstToken() const override;
  Token* getLastToken() const override;

  static constexpr const NodeKind Kind = NodeKind::RecordVariantDeclarationMember;

};

class VariantDeclaration : public Declaration {
public:

  class PubKeyword* PubKeyword;
  class EnumKeyword* EnumKeyword;
  class IdentifierAlt* Name;
  std::vector<VarTypeExpression*> TVs;
  class BlockStart* BlockStart;
  std::vector<VariantDeclarationMember*> Members;

  inline VariantDeclaration(
    class PubKeyword* PubKeyword,
    class EnumKeyword* EnumKeyword,
    class IdentifierAlt* Name,
    std::vector<VarTypeExpression*> TVs,
    class BlockStart* BlockStart,
    std::vector<VariantDeclarationMember*> Members
  ): Declaration(NodeKind::VariantDeclaration),
     PubKeyword(PubKeyword),
     EnumKeyword(EnumKeyword),
     Name(Name),
     TVs(TVs),
     BlockStart(BlockStart),
     Members(Members) {}

  Token* getFirstToken() const override;
  Token* getLastToken() const override;

  static constexpr const NodeKind Kind = NodeKind::VariantDeclaration;

};

class SourceFile : public Node {

  Scope* TheScope = nullptr;

public:

  TextFile File;

  std::vector<Node*> Elements;

  SourceFile(TextFile& File, std::vector<Node*> Elements):
    Node(NodeKind::SourceFile), File(File), Elements(Elements) {}

  inline TextFile& getTextFile() {
    return File;
  }

  inline const TextFile& getTextFile() const {
    return File;
  }

  inline std::filesystem::path getFilePath() {
    return File.getPath();
  }

  Token* getFirstToken() const override;
  Token* getLastToken() const override;

  inline Scope* getScope() override {
    if (TheScope == nullptr) {
      TheScope = new Scope(this);
    }
    return TheScope;
  }

  static constexpr const NodeKind Kind = NodeKind::SourceFile;

};

}

#endif
