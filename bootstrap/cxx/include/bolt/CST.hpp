#ifndef BOLT_CST_HPP
#define BOLT_CST_HPP

#include <limits>
#include <unordered_map>
#include <variant>
#include <vector>

#include "zen/config.hpp"

#include "bolt/Integer.hpp"
#include "bolt/String.hpp"
#include "bolt/ByteString.hpp"

namespace bolt {

  class Type;
  class InferContext;

  class Token;
  class SourceFile;
  class Scope;
  class Pattern;
  class Expression;
  class Statement;

  class TextLoc {
  public:

    size_t Line = 1;
    size_t Column = 1;

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

    std::vector<size_t> LineOffsets;

  public:

    TextFile(ByteString Path, ByteString Text);

    size_t getLine(size_t Offset) const;
    size_t getColumn(size_t Offset) const;
    size_t getStartOffsetOfLine(size_t Line) const;
    size_t getEndOffsetOfLine(size_t Line) const;

    size_t getLineCount() const;

    ByteString getPath() const;

    ByteString getText() const;

  };

  enum class NodeKind {
    VBar,
    Equals,
    Colon,
    Comma,
    Dot,
    DotDot,
    Tilde,
    At,
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    RArrow,
    RArrowAlt,
    LetKeyword,
    MutKeyword,
    PubKeyword,
    ForeignKeyword,
    TypeKeyword,
    ReturnKeyword,
    ModKeyword,
    StructKeyword,
    EnumKeyword,
    ClassKeyword,
    InstanceKeyword,
    ElifKeyword,
    IfKeyword,
    ElseKeyword,
    MatchKeyword,
    Invalid,
    EndOfFile,
    BlockStart,
    BlockEnd,
    LineFoldEnd,
    CustomOperator,
    Assignment,
    Identifier,
    IdentifierAlt,
    StringLiteral,
    IntegerLiteral,
    ExpressionAnnotation,
    TypeAssertAnnotation,
    TypeclassConstraintExpression,
    EqualityConstraintExpression,
    RecordTypeExpressionField,
    RecordTypeExpression,
    QualifiedTypeExpression,
    ReferenceTypeExpression,
    ArrowTypeExpression,
    AppTypeExpression,
    VarTypeExpression,
    NestedTypeExpression,
    TupleTypeExpression,
    BindPattern,
    LiteralPattern,
    RecordPatternField,
    RecordPattern,
    NamedRecordPattern,
    NamedTuplePattern,
    TuplePattern,
    NestedPattern,
    ListPattern,
    ReferenceExpression,
    MatchCase,
    MatchExpression,
    MemberExpression,
    TupleExpression,
    NestedExpression,
    LiteralExpression,
    CallExpression,
    InfixExpression,
    PrefixExpression,
    RecordExpressionField,
    RecordExpression,
    ExpressionStatement,
    ReturnStatement,
    IfStatement,
    IfStatementPart,
    TypeAssert,
    Parameter,
    LetBlockBody,
    LetExprBody,
    LetDeclaration,
    RecordDeclarationField,
    RecordDeclaration,
    VariantDeclaration,
    TupleVariantDeclarationMember,
    RecordVariantDeclarationMember,
    ClassDeclaration,
    InstanceDeclaration,
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

  class Node {

    unsigned RefCount = 1;

    const NodeKind Kind;

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
      return Kind;
    }

    template<typename T>
    bool is() const noexcept {
      return Kind == getNodeType<T>();
    }

    template<>
    bool is<Expression>() const noexcept {
      return Kind == NodeKind::ReferenceExpression
          || Kind == NodeKind::LiteralExpression
          || Kind == NodeKind::PrefixExpression
          || Kind == NodeKind::InfixExpression
          || Kind == NodeKind::CallExpression
          || Kind == NodeKind::NestedExpression;
    }

    template<typename T>
    T* as() {
      ZEN_ASSERT(is<T>());
      return static_cast<T*>(this);
    }

    virtual TextRange getRange() const;

    inline Node(NodeKind Type):
        Kind(Type) {}

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

    inline size_t getStartLine() const override {
      return StartLoc.Line;
    }

    inline size_t getStartColumn() const override {
      return StartLoc.Column;
    }

    inline size_t getEndLine() const override {
      return getEndLoc().Line;
    }

    inline size_t getEndColumn() const override {
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

    static bool classof(const Node* N) {
      return N->getKind() == NodeKind::Equals;
    }

  };

  class VBar : public Token {
  public:

    inline VBar(TextLoc StartLoc):
      Token(NodeKind::VBar, StartLoc) {}

    std::string getText() const override;

    static bool classof(const Node* N) {
      return N->getKind() == NodeKind::VBar;
    }

  };

  class Colon : public Token {
  public:

    inline Colon(TextLoc StartLoc):
      Token(NodeKind::Colon, StartLoc) {}

    std::string getText() const override;

    static bool classof(const Node* N) {
      return N->getKind() == NodeKind::Colon;
    }

  };

  class Comma : public Token {
  public:

    inline Comma(TextLoc StartLoc):
      Token(NodeKind::Comma, StartLoc) {}

    std::string getText() const override;

    static bool classof(const Node* N) {
      return N->getKind() == NodeKind::Comma;
    }

  };

  class Dot : public Token {
  public:

    inline Dot(TextLoc StartLoc):
      Token(NodeKind::Dot, StartLoc) {}

    std::string getText() const override;

    static bool classof(const Node* N) {
      return N->getKind() == NodeKind::Dot;
    }

  };

  class DotDot : public Token {
  public:

    inline DotDot(TextLoc StartLoc):
      Token(NodeKind::DotDot, StartLoc) {}

    std::string getText() const override;

    static bool classof(const Node* N) {
      return N->getKind() == NodeKind::DotDot;
    }

  };

  class Tilde : public Token {
  public:

    inline Tilde(TextLoc StartLoc):
      Token(NodeKind::Tilde, StartLoc) {}

    std::string getText() const override;

    static bool classof(const Node* N) {
      return N->getKind() == NodeKind::Tilde;
    }

  };

  class At : public Token {
  public:

    inline At(TextLoc StartLoc):
      Token(NodeKind::At, StartLoc) {}

    std::string getText() const override;

    static bool classof(const Node* N) {
      return N->getKind() == NodeKind::At;
    }

  };

  class LParen : public Token {
  public:

    inline LParen(TextLoc StartLoc):
      Token(NodeKind::LParen, StartLoc) {}

    std::string getText() const override;

    static bool classof(const Node* N) {
      return N->getKind() == NodeKind::LParen;
    }

  };

  class RParen : public Token {
  public:

    inline RParen(TextLoc StartLoc):
      Token(NodeKind::RParen, StartLoc) {}

    std::string getText() const override;

    static bool classof(const Node* N) {
      return N->getKind() == NodeKind::RParen;
    }

  };

  class LBracket : public Token {
  public:

    inline LBracket(TextLoc StartLoc):
      Token(NodeKind::LBracket, StartLoc) {}

    std::string getText() const override;

    static bool classof(const Node* N) {
      return N->getKind() == NodeKind::LBracket;
    }

  };

  class RBracket : public Token {
  public:

    inline RBracket(TextLoc StartLoc):
      Token(NodeKind::RBracket, StartLoc) {}

    std::string getText() const override;

    static bool classof(const Node* N) {
      return N->getKind() == NodeKind::RBracket;
    }

  };

  class LBrace : public Token {
  public:

    inline LBrace(TextLoc StartLoc):
      Token(NodeKind::LBrace, StartLoc) {}

    std::string getText() const override;

    static bool classof(const Node* N) {
      return N->getKind() == NodeKind::LBrace;
    }

  };

  class RBrace : public Token {
  public:

    inline RBrace(TextLoc StartLoc):
      Token(NodeKind::RBrace, StartLoc) {}

    std::string getText() const override;

    static bool classof(const Node* N) {
      return N->getKind() == NodeKind::RBrace;
    }

  };

  class RArrow : public Token {
  public:

    inline RArrow(TextLoc StartLoc):
      Token(NodeKind::RArrow, StartLoc) {}

    std::string getText() const override;

    static bool classof(const Node* N) {
      return N->getKind() == NodeKind::RArrow;
    }

  };

  class RArrowAlt : public Token {
  public:

    inline RArrowAlt(TextLoc StartLoc):
      Token(NodeKind::RArrowAlt, StartLoc) {}

    std::string getText() const override;

    static bool classof(const Node* N) {
      return N->getKind() == NodeKind::RArrowAlt;
    }

  };

  class LetKeyword : public Token {
  public:

    inline LetKeyword(TextLoc StartLoc):
      Token(NodeKind::LetKeyword, StartLoc) {}

    std::string getText() const override;

    static bool classof(const Node* N) {
      return N->getKind() == NodeKind::LetKeyword;
    }

  };

  class MutKeyword : public Token {
  public:

    inline MutKeyword(TextLoc StartLoc):
      Token(NodeKind::MutKeyword, StartLoc) {}

    std::string getText() const override;

    static bool classof(const Node* N) {
      return N->getKind() == NodeKind::MutKeyword;
    }

  };

  class PubKeyword : public Token {
  public:

    inline PubKeyword(TextLoc StartLoc):
      Token(NodeKind::PubKeyword, StartLoc) {}

    std::string getText() const override;

    static bool classof(const Node* N) {
      return N->getKind() == NodeKind::PubKeyword;
    }

  };

  class ForeignKeyword : public Token {
  public:

    inline ForeignKeyword(TextLoc StartLoc):
      Token(NodeKind::ForeignKeyword, StartLoc) {}

    std::string getText() const override;

    static bool classof(const Node* N) {
      return N->getKind() == NodeKind::ForeignKeyword;
    }

  };

  class TypeKeyword : public Token {
  public:

    inline TypeKeyword(TextLoc StartLoc):
      Token(NodeKind::TypeKeyword, StartLoc) {}

    std::string getText() const override;

    static bool classof(const Node* N) {
      return N->getKind() == NodeKind::TypeKeyword;
    }

  };

  class ReturnKeyword : public Token {
  public:

    inline ReturnKeyword(TextLoc StartLoc):
      Token(NodeKind::ReturnKeyword, StartLoc) {}

    std::string getText() const override;

    static bool classof(const Node* N) {
      return N->getKind() == NodeKind::ReturnKeyword;
    }

  };

  class ModKeyword : public Token {
  public:

    inline ModKeyword(TextLoc StartLoc):
      Token(NodeKind::ModKeyword, StartLoc) {}

    std::string getText() const override;

    static bool classof(const Node* N) {
      return N->getKind() == NodeKind::ModKeyword;
    }

  };

  class StructKeyword : public Token {
  public:

    inline StructKeyword(TextLoc StartLoc):
      Token(NodeKind::StructKeyword, StartLoc) {}

    std::string getText() const override;

    static bool classof(const Node* N) {
      return N->getKind() == NodeKind::StructKeyword;
    }

  };

  class EnumKeyword : public Token {
  public:

    inline EnumKeyword(TextLoc StartLoc):
      Token(NodeKind::EnumKeyword, StartLoc) {}

    std::string getText() const override;

    static bool classof(const Node* N) {
      return N->getKind() == NodeKind::EnumKeyword;
    }

  };

  class ClassKeyword : public Token {
  public:

    inline ClassKeyword(TextLoc StartLoc):
      Token(NodeKind::ClassKeyword, StartLoc) {}

    std::string getText() const override;

    static bool classof(const Node* N) {
      return N->getKind() == NodeKind::ClassKeyword;
    }

  };

  class InstanceKeyword : public Token {
  public:

    inline InstanceKeyword(TextLoc StartLoc):
      Token(NodeKind::InstanceKeyword, StartLoc) {}

    std::string getText() const override;

    static bool classof(const Node* N) {
      return N->getKind() == NodeKind::InstanceKeyword;
    }

  };

  class ElifKeyword : public Token {
  public:

    inline ElifKeyword(TextLoc StartLoc):
      Token(NodeKind::ElifKeyword, StartLoc) {}

    std::string getText() const override;

    static bool classof(const Node* N) {
      return N->getKind() == NodeKind::ElifKeyword;
    }

  };

  class IfKeyword : public Token {
  public:

    inline IfKeyword(TextLoc StartLoc):
      Token(NodeKind::IfKeyword, StartLoc) {}

    std::string getText() const override;

    static bool classof(const Node* N) {
      return N->getKind() == NodeKind::IfKeyword;
    }

  };

  class ElseKeyword : public Token {
  public:

    inline ElseKeyword(TextLoc StartLoc):
      Token(NodeKind::ElseKeyword, StartLoc) {}

    std::string getText() const override;

    static bool classof(const Node* N) {
      return N->getKind() == NodeKind::ElseKeyword;
    }

  };

  class MatchKeyword : public Token {
  public:

    inline MatchKeyword(TextLoc StartLoc):
      Token(NodeKind::MatchKeyword, StartLoc) {}

    std::string getText() const override;

    static bool classof(const Node* N) {
      return N->getKind() == NodeKind::MatchKeyword;
    }

  };

  class Invalid : public Token {
  public:

    inline Invalid(TextLoc StartLoc):
      Token(NodeKind::Invalid, StartLoc) {}

    std::string getText() const override;

    static bool classof(const Node* N) {
      return N->getKind() == NodeKind::Invalid;
    }

  };

  class EndOfFile : public Token {
  public:

    inline EndOfFile(TextLoc StartLoc):
      Token(NodeKind::EndOfFile, StartLoc) {}

    std::string getText() const override;

    static bool classof(const Node* N) {
      return N->getKind() == NodeKind::EndOfFile;
    }

  };

  class BlockStart : public Token {
  public:

    inline BlockStart(TextLoc StartLoc):
      Token(NodeKind::BlockStart, StartLoc) {}

    std::string getText() const override;

    static bool classof(const Node* N) {
      return N->getKind() == NodeKind::BlockStart;
    }

  };

  class BlockEnd : public Token {
  public:

    inline BlockEnd(TextLoc StartLoc):
      Token(NodeKind::BlockEnd, StartLoc) {}

    std::string getText() const override;

    static bool classof(const Node* N) {
      return N->getKind() == NodeKind::BlockEnd;
    }

  };

  class LineFoldEnd : public Token {
  public:

    inline LineFoldEnd(TextLoc StartLoc):
      Token(NodeKind::LineFoldEnd, StartLoc) {}

    std::string getText() const override;

    static bool classof(const Node* N) {
      return N->getKind() == NodeKind::LineFoldEnd;
    }

  };

  class CustomOperator : public Token {
  public:

    ByteString Text;

    CustomOperator(ByteString Text, TextLoc StartLoc):
      Token(NodeKind::CustomOperator, StartLoc), Text(Text) {}

    std::string getText() const override;

    static bool classof(const Node* N) {
      return N->getKind() == NodeKind::CustomOperator;
    }

  };

  class Assignment : public Token {
  public:

    ByteString Text;

    Assignment(ByteString Text, TextLoc StartLoc):
      Token(NodeKind::Assignment, StartLoc), Text(Text) {}

    std::string getText() const override;

    static bool classof(const Node* N) {
      return N->getKind() == NodeKind::Assignment;
    }

  };

  class Symbol : public Token {
  public:

    inline Symbol(NodeKind Kind, TextLoc StartLoc):
      Token(Kind, StartLoc) {}

    virtual ByteString getCanonicalText() = 0;

    static bool classof(const Node* N) {
      return N->getKind() == NodeKind::Identifier
          || N->getKind() == NodeKind::IdentifierAlt;
    }

  };

  class Identifier : public Symbol {
  public:

    ByteString Text;

    Identifier(ByteString Text, TextLoc StartLoc = TextLoc::empty()):
      Symbol(NodeKind::Identifier, StartLoc), Text(Text) {}

    ByteString getCanonicalText() override;

    std::string getText() const override;

    bool isTypeVar() const;

    static bool classof(const Node* N) {
      return N->getKind() == NodeKind::Identifier;
    }

  };

  class IdentifierAlt : public Symbol {
  public:

    ByteString Text;

    IdentifierAlt(ByteString Text, TextLoc StartLoc):
      Symbol(NodeKind::IdentifierAlt, StartLoc), Text(Text) {}

    ByteString getCanonicalText() override;

    std::string getText() const override;

    static bool classof(const Node* N) {
      return N->getKind() == NodeKind::IdentifierAlt;
    }

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

    static bool classof(const Node* N) {
      return N->getKind() == NodeKind::StringLiteral;
    }

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

    static bool classof(const Node* N) {
      return N->getKind() == NodeKind::IntegerLiteral;
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
  };

  class ExpressionAnnotation : public Annotation {
  public:

    At* At;
    Expression* Expression;

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

  };

  class TypeExpression;

  class TypeAssertAnnotation : public Annotation {
  public:

    At* At;
    Colon* Colon;
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

  };

  class TypeExpression : public TypedNode, AnnotationContainer {
  protected:

    inline TypeExpression(NodeKind Kind, std::vector<Annotation*> Annotations = {}):
      TypedNode(Kind), AnnotationContainer(Annotations) {}

  };

  class ConstraintExpression : public Node {
  public:

    inline ConstraintExpression(NodeKind Kind):
      Node(Kind) {}

  };

  class RecordTypeExpressionField : public Node {
  public:

    Identifier* Name;
    Colon* Colon;
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

  };

  class RecordTypeExpression : public TypeExpression {
  public:

    LBrace* LBrace;
    std::vector<std::tuple<RecordTypeExpressionField*, Comma*>> Fields;
    VBar* VBar;
    TypeExpression* Rest;
    RBrace* RBrace;

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

    static bool classof(const Node* N) {
      return N->getKind() == NodeKind::TypeclassConstraintExpression;
    }

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

    static bool classof(const Node* N) {
      return N->getKind() == NodeKind::EqualityConstraintExpression;
    }

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

    static bool classof(const Node* N) {
      return N->getKind() == NodeKind::QualifiedTypeExpression;
    }

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

  };

  class VarTypeExpression : public TypeExpression {
  public:

    Identifier* Name;

    inline VarTypeExpression(Identifier* Name):
      TypeExpression(NodeKind::VarTypeExpression), Name(Name) {}

    Token* getFirstToken() const override;
    Token* getLastToken() const override;

  };

  class NestedTypeExpression : public TypeExpression {
  public:

    LParen* LParen;
    TypeExpression* TE;
    RParen* RParen;

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

  };

  class TupleTypeExpression : public TypeExpression {
  public:

    LParen* LParen;
    std::vector<std::tuple<TypeExpression*, Comma*>> Elements;
    RParen* RParen;

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

  };

  class Pattern : public Node {
  protected:

    inline Pattern(NodeKind Type):
      Node(Type) {}

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

    static bool classof(const Node* N) {
      return N->getKind() == NodeKind::BindPattern;
    }

  };

  class LiteralPattern : public Pattern {
  public:

    class Literal* Literal;

    LiteralPattern(class Literal* Literal):
      Pattern(NodeKind::LiteralPattern),
      Literal(Literal) {}

    Token* getFirstToken() const override;
    Token* getLastToken() const override;

    static bool classof(const Node* N) {
      return N->getKind() == NodeKind::LiteralPattern;
    }

  };

  class RecordPatternField : public Node {
  public:

    DotDot* DotDot;
    Identifier* Name;
    Equals* Equals;
    Pattern* Pattern;

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

  };

  class RecordPattern : public Pattern {
  public:

    LBrace* LBrace;
    std::vector<std::tuple<RecordPatternField*, Comma*>> Fields;
    RBrace* RBrace;

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

  };

  class NamedRecordPattern : public Pattern {
  public:

    std::vector<std::tuple<IdentifierAlt*, Dot*>> ModulePath;
    IdentifierAlt* Name;
    LBrace* LBrace;
    std::vector<std::tuple<RecordPatternField*, Comma*>> Fields;
    RBrace* RBrace;

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

  };

  class TuplePattern : public Pattern {
  public:

    LParen* LParen;
    std::vector<std::tuple<Pattern*, Comma*>> Elements;
    RParen* RParen;
  
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

  };

  class Expression : public TypedNode, public AnnotationContainer {
  protected:

    inline Expression(NodeKind Kind, std::vector<Annotation*> Annotations = {}):
      TypedNode(Kind), AnnotationContainer(Annotations) {}

  };

  class ReferenceExpression : public Expression {
  public:

    std::vector<std::tuple<IdentifierAlt*, Dot*>> ModulePath;
    Symbol* Name;

    inline ReferenceExpression(
      std::vector<std::tuple<IdentifierAlt*, Dot*>> ModulePath,
      Symbol* Name
    ): Expression(NodeKind::ReferenceExpression),
       ModulePath(ModulePath),
       Name(Name) {}

    inline ReferenceExpression(
      std::vector<Annotation*> Annotations,
      std::vector<std::tuple<IdentifierAlt*, Dot*>> ModulePath,
      Symbol* Name
    ): Expression(NodeKind::ReferenceExpression, Annotations),
       ModulePath(ModulePath),
       Name(Name) {}

    inline ByteString getNameAsString() const noexcept {
      return Name->getCanonicalText();
    }

    Token* getFirstToken() const override;
    Token* getLastToken() const override;

    SymbolPath getSymbolPath() const;

  };

  class MatchCase : public Node { 
  public:

    InferContext* Ctx;

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

  };

  class MatchExpression : public Expression {
  public:

    class MatchKeyword* MatchKeyword;
    Expression* Value;
    class BlockStart* BlockStart;
    std::vector<MatchCase*> Cases;

    inline MatchExpression(
      class MatchKeyword* MatchKeyword,
      Expression* Value,
      class BlockStart* BlockStart,
      std::vector<MatchCase*> Cases
    ): Expression(NodeKind::MatchExpression),
       MatchKeyword(MatchKeyword),
       Value(Value),
       BlockStart(BlockStart),
       Cases(Cases) {}

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

    Token* getFirstToken() const override;
    Token* getLastToken() const override;

  };

  class MemberExpression : public Expression {
  public:

    Expression* E;
    Dot* Dot;
    Token* Name;

    inline MemberExpression(
      class Expression* E,
      class Dot* Dot,
      Token* Name
    ): Expression(NodeKind::MemberExpression),
       E(E),
       Dot(Dot),
       Name(Name) {}

    inline MemberExpression(
      std::vector<Annotation*> Annotations,
      class Expression* E,
      class Dot* Dot,
      Token* Name
    ): Expression(NodeKind::MemberExpression, Annotations),
       E(E),
       Dot(Dot),
       Name(Name) {}

    Token* getFirstToken() const override;
    Token* getLastToken() const override;

    inline Expression* getExpression() const {
      return E;
    }

  };

  class TupleExpression : public Expression {
  public:

    class LParen* LParen;
    std::vector<std::tuple<Expression*, Comma*>> Elements;
    class RParen* RParen;

    inline TupleExpression(
      class LParen* LParen,
      std::vector<std::tuple<Expression*, Comma*>> Elements,
      class RParen* RParen
    ): Expression(NodeKind::TupleExpression),
       LParen(LParen),
       Elements(Elements),
       RParen(RParen) {}

    inline TupleExpression(
      std::vector<Annotation*> Annotations,
      class LParen* LParen,
      std::vector<std::tuple<Expression*, Comma*>> Elements,
      class RParen* RParen
    ): Expression(NodeKind::TupleExpression, Annotations),
       LParen(LParen),
       Elements(Elements),
       RParen(RParen) {}

    Token* getFirstToken() const override;
    Token* getLastToken() const override;

  };

  class NestedExpression : public Expression {
  public:

    class LParen* LParen;
    Expression* Inner;
    class RParen* RParen;

    inline NestedExpression(
      class LParen* LParen,
      Expression* Inner,
      class RParen* RParen
    ): Expression(NodeKind::NestedExpression),
       LParen(LParen),
       Inner(Inner),
       RParen(RParen) {}

    inline NestedExpression(
      std::vector<Annotation*> Annotations,
      class LParen* LParen,
      Expression* Inner,
      class RParen* RParen
    ): Expression(NodeKind::NestedExpression, Annotations),
       LParen(LParen),
       Inner(Inner),
       RParen(RParen) {}

    Token* getFirstToken() const override;
    Token* getLastToken() const override;

  };

  class LiteralExpression : public Expression {
  public:

    Literal* Token;

    LiteralExpression(
      Literal* Token
    ): Expression(NodeKind::LiteralExpression),
       Token(Token) {}

    LiteralExpression(
      std::vector<Annotation*> Annotations,
      Literal* Token
    ): Expression(NodeKind::LiteralExpression, Annotations),
       Token(Token) {}

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

  };

  class CallExpression : public Expression {
  public:

    Expression* Function;
    std::vector<Expression*> Args;

    inline CallExpression(
      Expression* Function,
      std::vector<Expression*> Args
    ): Expression(NodeKind::CallExpression),
       Function(Function),
       Args(Args) {}

    inline CallExpression(
      std::vector<Annotation*> Annotations,
      Expression* Function,
      std::vector<Expression*> Args
    ): Expression(NodeKind::CallExpression, Annotations),
       Function(Function),
       Args(Args) {}

    Token* getFirstToken() const override;
    Token* getLastToken() const override;

  };

  class InfixExpression : public Expression {
  public:

    Expression* Left;
    Token* Operator;
    Expression* Right;

    inline InfixExpression(
      Expression* Left,
      Token* Operator,
      Expression* Right
    ): Expression(NodeKind::InfixExpression),
       Left(Left),
       Operator(Operator),
       Right(Right) {}

    inline InfixExpression(
      std::vector<Annotation*> Annotations,
      Expression* Left,
      Token* Operator,
      Expression* Right
    ): Expression(NodeKind::InfixExpression, Annotations),
       Left(Left),
       Operator(Operator),
       Right(Right) {}

    Token* getFirstToken() const override;
    Token* getLastToken() const override;

  };

  class PrefixExpression : public Expression {
  public:

    Token* Operator;
    Expression* Argument;

    PrefixExpression(
      Token* Operator,
      Expression* Argument
    ): Expression(NodeKind::PrefixExpression),
       Operator(Operator),
       Argument(Argument) {}

    PrefixExpression(
      std::vector<Annotation*> Annotations,
      Token* Operator,
      Expression* Argument
    ): Expression(NodeKind::PrefixExpression, Annotations),
       Operator(Operator),
       Argument(Argument) {}

    Token* getFirstToken() const override;
    Token* getLastToken() const override;

  };

  class RecordExpressionField : public Node {
  public:

    Identifier* Name;
    Equals* Equals;
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

    inline Expression* getExpression() const {
      return E;
    }

  };

  class RecordExpression : public Expression {
  public:

    LBrace* LBrace;
    std::vector<std::tuple<RecordExpressionField*, Comma*>> Fields;
    RBrace* RBrace;

    inline RecordExpression(
      class LBrace* LBrace,
      std::vector<std::tuple<RecordExpressionField*, Comma*>> Fields,
      class RBrace* RBrace
    ): Expression(NodeKind::RecordExpression),
       LBrace(LBrace),
       Fields(Fields),
       RBrace(RBrace) {}

    inline RecordExpression(
      std::vector<Annotation*> Annotations,
      class LBrace* LBrace,
      std::vector<std::tuple<RecordExpressionField*, Comma*>> Fields,
      class RBrace* RBrace
    ): Expression(NodeKind::RecordExpression, Annotations),
       LBrace(LBrace),
       Fields(Fields),
       RBrace(RBrace) {}

    Token* getFirstToken() const override;
    Token* getLastToken() const override;

  };

  class Statement : public Node, public AnnotationContainer {
  protected:

    inline Statement(NodeKind Type, std::vector<Annotation*> Annotations = {}):
      Node(Type), AnnotationContainer(Annotations) {}

  };

  class ExpressionStatement : public Statement {
  public:

    class Expression* Expression;

    ExpressionStatement(class Expression* Expression):
      Statement(NodeKind::ExpressionStatement), Expression(Expression) {}

    ExpressionStatement(
      std::vector<Annotation*> Annotations,
      class Expression* Expression
    ): Statement(NodeKind::ExpressionStatement, Annotations),
       Expression(Expression) {}

    Token* getFirstToken() const override;
    Token* getLastToken() const override;

  };

  class IfStatementPart : public Node, public AnnotationContainer {
  public:

    Token* Keyword;
    Expression* Test;
    class BlockStart* BlockStart;
    std::vector<Node*> Elements;

    inline IfStatementPart(
      Token* Keyword,
      Expression* Test,
      class BlockStart* BlockStart,
      std::vector<Node*> Elements
    ): Node(NodeKind::IfStatementPart),
       Keyword(Keyword),
       Test(Test),
       BlockStart(BlockStart),
       Elements(Elements) {}

    inline IfStatementPart(
      std::vector<Annotation*> Annotations,
      Token* Keyword,
      Expression* Test,
      class BlockStart* BlockStart,
      std::vector<Node*> Elements
    ): Node(NodeKind::IfStatementPart),
       AnnotationContainer(Annotations),
       Keyword(Keyword),
       Test(Test),
       BlockStart(BlockStart),
       Elements(Elements) {}

    Token* getFirstToken() const override;
    Token* getLastToken() const override;

  };

  class IfStatement : public Statement {
  public:
 
    std::vector<IfStatementPart*> Parts;

    inline IfStatement(std::vector<IfStatementPart*> Parts):
      Statement(NodeKind::IfStatement), Parts(Parts) {}

    Token* getFirstToken() const override;
    Token* getLastToken() const override;

  };

  class ReturnStatement : public Statement {
  public:

    class ReturnKeyword* ReturnKeyword;
    class Expression* Expression;

    ReturnStatement(
      class ReturnKeyword* ReturnKeyword,
      class Expression* Expression
    ): Statement(NodeKind::ReturnStatement),
       ReturnKeyword(ReturnKeyword),
       Expression(Expression) {}

    ReturnStatement(
      std::vector<Annotation*> Annotations,
      class ReturnKeyword* ReturnKeyword,
      class Expression* Expression
    ): Statement(NodeKind::ReturnStatement, Annotations),
       ReturnKeyword(ReturnKeyword),
       Expression(Expression) {}

    Token* getFirstToken() const override;
    Token* getLastToken() const override;

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

  };

  class LetDeclaration : public TypedNode, public AnnotationContainer {

    Scope* TheScope = nullptr;

  public:

    bool IsCycleActive = false;
    bool Visited = false;
    InferContext* Ctx;

    class PubKeyword* PubKeyword;
    class ForeignKeyword* ForeignKeyword;
    class LetKeyword* LetKeyword;
    class MutKeyword* MutKeyword;
    class Pattern* Pattern;
    std::vector<Parameter*> Params;
    class TypeAssert* TypeAssert;
    LetBody* Body;

    LetDeclaration(
      class PubKeyword* PubKeyword,
      class ForeignKeyword* ForeignKeyword,
      class LetKeyword* LetKeyword,
      class MutKeyword* MutKeyword,
      class Pattern* Pattern,
      std::vector<Parameter*> Params,
      class TypeAssert* TypeAssert,
      LetBody* Body
    ): TypedNode(NodeKind::LetDeclaration),
       PubKeyword(PubKeyword),
       ForeignKeyword(ForeignKeyword),
       LetKeyword(LetKeyword),
       MutKeyword(MutKeyword),
       Pattern(Pattern),
       Params(Params),
       TypeAssert(TypeAssert),
       Body(Body) {}

    LetDeclaration(
      std::vector<Annotation*> Annotations,
      class PubKeyword* PubKeyword,
      class ForeignKeyword* ForeignKeyword,
      class LetKeyword* LetKeyword,
      class MutKeyword* MutKeyword,
      class Pattern* Pattern,
      std::vector<Parameter*> Params,
      class TypeAssert* TypeAssert,
      LetBody* Body
    ): TypedNode(NodeKind::LetDeclaration),
       AnnotationContainer(Annotations),
       PubKeyword(PubKeyword),
       ForeignKeyword(ForeignKeyword),
       LetKeyword(LetKeyword),
       MutKeyword(MutKeyword),
       Pattern(Pattern),
       Params(Params),
       TypeAssert(TypeAssert),
       Body(Body) {}

    inline Scope* getScope() override {
      if (isFunction()) {
        if (TheScope == nullptr) {
          TheScope = new Scope(this);
        }
        return TheScope;
      }
      return Parent->getScope();
    }

    bool isInstance() const noexcept {
      return Parent->getKind() == NodeKind::InstanceDeclaration;
    }

    bool isClass() const noexcept {
      return Parent->getKind() == NodeKind::ClassDeclaration;
    }

    bool isSignature() const noexcept {
      return ForeignKeyword;
    }

    bool isVariable() const noexcept {
      // Variables in classes and instances are never possible, so we reflect this by excluding them here.
      return !isSignature() && !isClass() && !isInstance() && Params.empty() && (Pattern->getKind() != NodeKind::BindPattern || !Body);
    }

    bool isFunction() const noexcept {
      return !isSignature() && !isVariable();
    }

    Identifier* getName() const noexcept {
      ZEN_ASSERT(Pattern->getKind() == NodeKind::BindPattern);
      return static_cast<BindPattern*>(Pattern)->Name;
    }

    ByteString getNameAsString() const noexcept {
      return getName()->getCanonicalText();
    }

    Token* getFirstToken() const override;
    Token* getLastToken() const override;

    static bool classof(const Node* N) {
      return N->getKind() == NodeKind::LetDeclaration;
    }

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

    static bool classof(const Node* N) {
      return N->getKind() == NodeKind::InstanceDeclaration;
    }

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

    static bool classof(const Node* N) {
      return N->getKind() == NodeKind::ClassDeclaration;
    }

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

  };

  class RecordDeclaration : public Node {
  public:

    InferContext* Ctx;

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
    ): Node(NodeKind::RecordDeclaration),
       PubKeyword(PubKeyword),
       StructKeyword(StructKeyword),
       Name(Name),
       Vars(Vars),
       BlockStart(BlockStart),
       Fields(Fields) {}

    Token* getFirstToken() const override;
    Token* getLastToken() const override;

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

  };

  class RecordVariantDeclarationMember : public VariantDeclarationMember {
  public:

    IdentifierAlt* Name;
    BlockStart* BlockStart;
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

  };

  class VariantDeclaration : public Node {
  public:

    InferContext* Ctx;

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
    ): Node(NodeKind::VariantDeclaration),
       PubKeyword(PubKeyword),
       EnumKeyword(EnumKeyword),
       Name(Name),
       TVs(TVs),
       BlockStart(BlockStart),
       Members(Members) {}

    Token* getFirstToken() const override;
    Token* getLastToken() const override;

  };

  class SourceFile : public Node {

    Scope* TheScope = nullptr;

  public:

    TextFile File;
    InferContext* Ctx;

    std::vector<Node*> Elements;

    SourceFile(TextFile& File, std::vector<Node*> Elements):
      Node(NodeKind::SourceFile), File(File), Elements(Elements) {}

    inline TextFile& getTextFile() {
      return File;
    }

    inline const TextFile& getTextFile() const {
      return File;
    }

    Token* getFirstToken() const override;
    Token* getLastToken() const override;

    inline Scope* getScope() override {
      if (TheScope == nullptr) {
        TheScope = new Scope(this);
      }
      return TheScope;
    }

    static bool classof(const Node* N) {
      return N->getKind() == NodeKind::SourceFile;
    }

  };

  template<> inline NodeKind getNodeType<Equals>() { return NodeKind::Equals; }
  template<> inline NodeKind getNodeType<Colon>() { return NodeKind::Colon; }
  template<> inline NodeKind getNodeType<Dot>() { return NodeKind::Dot; }
  template<> inline NodeKind getNodeType<DotDot>() { return NodeKind::DotDot; }
  template<> inline NodeKind getNodeType<Tilde>() { return NodeKind::Tilde; }
  template<> inline NodeKind getNodeType<LParen>() { return NodeKind::LParen; }
  template<> inline NodeKind getNodeType<RParen>() { return NodeKind::RParen; }
  template<> inline NodeKind getNodeType<LBracket>() { return NodeKind::LBracket; }
  template<> inline NodeKind getNodeType<RBracket>() { return NodeKind::RBracket; }
  template<> inline NodeKind getNodeType<LBrace>() { return NodeKind::LBrace; }
  template<> inline NodeKind getNodeType<RBrace>() { return NodeKind::RBrace; }
  template<> inline NodeKind getNodeType<RArrow>() { return NodeKind::RArrow; }
  template<> inline NodeKind getNodeType<RArrowAlt>() { return NodeKind::RArrowAlt; }
  template<> inline NodeKind getNodeType<LetKeyword>() { return NodeKind::LetKeyword; }
  template<> inline NodeKind getNodeType<ForeignKeyword>() { return NodeKind::ForeignKeyword; }
  template<> inline NodeKind getNodeType<MutKeyword>() { return NodeKind::MutKeyword; }
  template<> inline NodeKind getNodeType<PubKeyword>() { return NodeKind::PubKeyword; }
  template<> inline NodeKind getNodeType<TypeKeyword>() { return NodeKind::TypeKeyword; }
  template<> inline NodeKind getNodeType<ReturnKeyword>() { return NodeKind::ReturnKeyword; }
  template<> inline NodeKind getNodeType<ModKeyword>() { return NodeKind::ModKeyword; }
  template<> inline NodeKind getNodeType<StructKeyword>() { return NodeKind::StructKeyword; }
  template<> inline NodeKind getNodeType<EnumKeyword>() { return NodeKind::EnumKeyword; }
  template<> inline NodeKind getNodeType<ClassKeyword>() { return NodeKind::ClassKeyword; }
  template<> inline NodeKind getNodeType<InstanceKeyword>() { return NodeKind::InstanceKeyword; }
  template<> inline NodeKind getNodeType<ElifKeyword>() { return NodeKind::ElifKeyword; }
  template<> inline NodeKind getNodeType<IfKeyword>() { return NodeKind::IfKeyword; }
  template<> inline NodeKind getNodeType<MatchKeyword>() { return NodeKind::MatchKeyword; }
  template<> inline NodeKind getNodeType<ElseKeyword>() { return NodeKind::ElseKeyword; }
  template<> inline NodeKind getNodeType<Invalid>() { return NodeKind::Invalid; }
  template<> inline NodeKind getNodeType<EndOfFile>() { return NodeKind::EndOfFile; }
  template<> inline NodeKind getNodeType<BlockStart>() { return NodeKind::BlockStart; }
  template<> inline NodeKind getNodeType<BlockEnd>() { return NodeKind::BlockEnd; }
  template<> inline NodeKind getNodeType<LineFoldEnd>() { return NodeKind::LineFoldEnd; }
  template<> inline NodeKind getNodeType<CustomOperator>() { return NodeKind::CustomOperator; }
  template<> inline NodeKind getNodeType<Assignment>() { return NodeKind::Assignment; }
  template<> inline NodeKind getNodeType<Identifier>() { return NodeKind::Identifier; }
  template<> inline NodeKind getNodeType<IdentifierAlt>() { return NodeKind::IdentifierAlt; }
  template<> inline NodeKind getNodeType<StringLiteral>() { return NodeKind::StringLiteral; }
  template<> inline NodeKind getNodeType<IntegerLiteral>() { return NodeKind::IntegerLiteral; }
  template<> inline NodeKind getNodeType<QualifiedTypeExpression>() { return NodeKind::QualifiedTypeExpression; }
  template<> inline NodeKind getNodeType<ReferenceTypeExpression>() { return NodeKind::ReferenceTypeExpression; }
  template<> inline NodeKind getNodeType<ArrowTypeExpression>() { return NodeKind::ArrowTypeExpression; }
  template<> inline NodeKind getNodeType<BindPattern>() { return NodeKind::BindPattern; }
  template<> inline NodeKind getNodeType<ReferenceExpression>() { return NodeKind::ReferenceExpression; }
  template<> inline NodeKind getNodeType<NestedExpression>() { return NodeKind::NestedExpression; }
  template<> inline NodeKind getNodeType<LiteralExpression>() { return NodeKind::LiteralExpression; }
  template<> inline NodeKind getNodeType<CallExpression>() { return NodeKind::CallExpression; }
  template<> inline NodeKind getNodeType<InfixExpression>() { return NodeKind::InfixExpression; }
  template<> inline NodeKind getNodeType<PrefixExpression>() { return NodeKind::PrefixExpression; }
  template<> inline NodeKind getNodeType<ExpressionStatement>() { return NodeKind::ExpressionStatement; }
  template<> inline NodeKind getNodeType<ReturnStatement>() { return NodeKind::ReturnStatement; }
  template<> inline NodeKind getNodeType<IfStatement>() { return NodeKind::IfStatement; }
  template<> inline NodeKind getNodeType<IfStatementPart>() { return NodeKind::IfStatementPart; }
  template<> inline NodeKind getNodeType<TypeAssert>() { return NodeKind::TypeAssert; }
  template<> inline NodeKind getNodeType<Parameter>() { return NodeKind::Parameter; }
  template<> inline NodeKind getNodeType<LetBlockBody>() { return NodeKind::LetBlockBody; }
  template<> inline NodeKind getNodeType<LetExprBody>() { return NodeKind::LetExprBody; }
  template<> inline NodeKind getNodeType<LetDeclaration>() { return NodeKind::LetDeclaration; }
  template<> inline NodeKind getNodeType<RecordDeclarationField>() { return NodeKind::RecordDeclarationField; }
  template<> inline NodeKind getNodeType<RecordDeclaration>() { return NodeKind::RecordDeclaration; }
  template<> inline NodeKind getNodeType<ClassDeclaration>() { return NodeKind::ClassDeclaration; }
  template<> inline NodeKind getNodeType<InstanceDeclaration>() { return NodeKind::InstanceDeclaration; }
  template<> inline NodeKind getNodeType<SourceFile>() { return NodeKind::SourceFile; }

}

#endif
