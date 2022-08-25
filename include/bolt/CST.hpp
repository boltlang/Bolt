#ifndef BOLT_CST_HPP
#define BOLT_CST_HPP

#include <iterator>
#include <vector>

#include "bolt/Text.hpp"
#include "bolt/Integer.hpp"
#include "bolt/ByteString.hpp"

namespace bolt {

  enum class NodeType {
    Equals,
    Colon,
    Dot,
    DotDot,
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    RArrow,
    LetKeyword,
    MutKeyword,
    PubKeyword,
    TypeKeyword,
    ReturnKeyword,
    ModKeyword,
    StructKeyword,
    Invalid,
    EndOfFile,
    BlockStart,
    BlockEnd,
    LineFoldEnd,
    CustomOperator,
    Assignment,
    Identifier,
    StringLiteral,
    IntegerLiteral,
    QualifiedName,
    ReferenceTypeExpression,
    ArrowTypeExpression,
    BindPattern,
    ReferenceExpression,
    ConstantExpression,
    CallExpression,
    InfixExpression,
    UnaryExpression,
    ExpressionStatement,
    ReturnStatement,
    TypeAssert,
    Param,
    LetBlockBody,
    LetExprBody,
    LetDeclaration,
    StructDeclField,
    StructDecl,
    SourceFile,
  };

  class Token;
  class SourceFile;

  class Node {

    unsigned RefCount = 0;

  public:

    Node* Parent = nullptr;

    inline void ref() {
      ++RefCount;
    }

    inline void unref() {
      --RefCount;
      if (RefCount == 0) {
        delete this;
      }
    }

    virtual void setParents() = 0;
    
    virtual Token* getFirstToken() = 0;
    virtual Token* getLastToken() = 0;

    TextRange getRange();

    const NodeType Type;

    inline Node(NodeType Type):
        Type(Type) {}

    SourceFile* getSourceFile();

    virtual ~Node();

  };

  class Token : public Node {

    TextLoc StartLoc;

  public:

    Token(NodeType Type, TextLoc StartLoc): Node(Type), StartLoc(StartLoc) {}

    virtual std::string getText() const = 0;

    void setParents() override;

    inline Token* getFirstToken() override {
      return this;
    }

    inline Token* getLastToken() override {
      return this;
    }

    inline TextLoc getStartLoc() {
      return StartLoc;
    }

    TextLoc getEndLoc();

    inline size_t getStartLine() {
      return StartLoc.Line;
    }

    inline size_t getStartColumn() {
      return StartLoc.Column;
    }

    inline size_t getEndLine() {
      return getEndLoc().Line;
    }

    inline size_t getEndColumn() {
      return getEndLoc().Column;
    }

    ~Token();

  };

  class Equals : public Token {
  public:

    Equals(TextLoc StartLoc):
      Token(NodeType::Equals, StartLoc) {}

    std::string getText() const override;

    ~Equals();

  };

  class Colon : public Token {
  public:

    Colon(TextLoc StartLoc):
      Token(NodeType::Colon, StartLoc) {}

    std::string getText() const override;

    ~Colon();

  };

  class RArrow : public Token {
  public:

    RArrow(TextLoc StartLoc):
      Token(NodeType::RArrow, StartLoc) {}

    std::string getText() const override;

    ~RArrow();

  };

  class Dot : public Token {
  public:

    Dot(TextLoc StartLoc):
      Token(NodeType::Dot, StartLoc) {}

    std::string getText() const override;

    ~Dot();

  };

  class DotDot : public Token {
  public:

    DotDot(TextLoc StartLoc):
      Token(NodeType::DotDot, StartLoc) {}

    std::string getText() const override;

    ~DotDot();

  };

  class LParen : public Token {
  public:

    LParen(TextLoc StartLoc):
      Token(NodeType::LParen, StartLoc) {}

    std::string getText() const override;

    ~LParen();

  };

  class RParen : public Token {
  public:

    RParen(TextLoc StartLoc):
      Token(NodeType::RParen, StartLoc) {}

    std::string getText() const override;

    ~RParen();

  };

  class LBracket : public Token {
  public:

    LBracket(TextLoc StartLoc):
      Token(NodeType::LBracket, StartLoc) {}

    std::string getText() const override;

    ~LBracket();

  };

  class RBracket : public Token {
  public:

    RBracket(TextLoc StartLoc):
      Token(NodeType::RBracket, StartLoc) {}

    std::string getText() const override;

    ~RBracket();

  };

  class LBrace : public Token {
  public:

    LBrace(TextLoc StartLoc):
      Token(NodeType::LBrace, StartLoc) {}

    std::string getText() const override;

    ~LBrace();

  };

  class RBrace : public Token {
  public:

    RBrace(TextLoc StartLoc):
      Token(NodeType::RBrace, StartLoc) {}

    std::string getText() const override;

    ~RBrace();

  };

  class LetKeyword : public Token {
  public:

    LetKeyword(TextLoc StartLoc):
      Token(NodeType::LetKeyword, StartLoc) {}

    std::string getText() const override;

    ~LetKeyword();

  };

  class MutKeyword : public Token {
  public:

    MutKeyword(TextLoc StartLoc):
      Token(NodeType::MutKeyword, StartLoc) {}

    std::string getText() const override;

    ~MutKeyword();

  };

  class PubKeyword : public Token {
  public:

    PubKeyword(TextLoc StartLoc):
      Token(NodeType::PubKeyword, StartLoc) {}

    std::string getText() const override;

    ~PubKeyword();

  };

  class TypeKeyword : public Token {
  public:

    TypeKeyword(TextLoc StartLoc):
      Token(NodeType::TypeKeyword, StartLoc) {}

    std::string getText() const override;

    ~TypeKeyword();

  };

  class ReturnKeyword : public Token {
  public:

    ReturnKeyword(TextLoc StartLoc):
      Token(NodeType::ReturnKeyword, StartLoc) {}

    std::string getText() const override;

    ~ReturnKeyword();

  };

  class ModKeyword : public Token {
  public:

    ModKeyword(TextLoc StartLoc):
      Token(NodeType::ModKeyword, StartLoc) {}

    std::string getText() const override;

    ~ModKeyword();

  };

  class StructKeyword : public Token {
  public:

    StructKeyword(TextLoc StartLoc):
      Token(NodeType::StructKeyword, StartLoc) {}

    std::string getText() const override;

    ~StructKeyword();

  };

  class Invalid : public Token {
  public:

    Invalid(TextLoc StartLoc):
      Token(NodeType::Invalid, StartLoc) {}

    std::string getText() const override;

    ~Invalid();

  };

  class EndOfFile : public Token {
  public:

    EndOfFile(TextLoc StartLoc):
      Token(NodeType::EndOfFile, StartLoc) {}

    std::string getText() const override;

    ~EndOfFile();

  };

  class BlockStart : public Token {
  public:

    BlockStart(TextLoc StartLoc):
      Token(NodeType::BlockStart, StartLoc) {}

    std::string getText() const override;

    ~BlockStart();

  };

  class BlockEnd : public Token {
  public:

    BlockEnd(TextLoc StartLoc):
      Token(NodeType::BlockEnd, StartLoc) {}

    std::string getText() const override;

    ~BlockEnd();

  };

  class LineFoldEnd : public Token {
  public:

    LineFoldEnd(TextLoc StartLoc):
      Token(NodeType::LineFoldEnd, StartLoc) {}

    std::string getText() const override;

    ~LineFoldEnd();

  };

  class CustomOperator : public Token {
  public:

    ByteString Text;

    CustomOperator(ByteString Text, TextLoc StartLoc):
      Token(NodeType::CustomOperator, StartLoc), Text(Text) {}

    std::string getText() const override;

    ~CustomOperator();

  };

  class Assignment : public Token {
  public:

    ByteString Text;

    Assignment(ByteString Text, TextLoc StartLoc):
      Token(NodeType::Assignment, StartLoc), Text(Text) {}

    std::string getText() const override;

    ~Assignment();

  };

  class Identifier : public Token {
  public:

    ByteString Text;

    Identifier(ByteString Text, TextLoc StartLoc):
      Token(NodeType::Identifier, StartLoc), Text(Text) {}

    std::string getText() const override;

    ~Identifier();

  };

  class StringLiteral : public Token {
  public:

    ByteString Text;

    StringLiteral(ByteString Text, TextLoc StartLoc):
      Token(NodeType::StringLiteral, StartLoc), Text(Text) {}

    std::string getText() const override;

    ~StringLiteral();

  };

  class IntegerLiteral : public Token {
  public:

    Integer Value;

    IntegerLiteral(Integer Value, TextLoc StartLoc):
      Token(NodeType::IntegerLiteral, StartLoc), Value(Value) {}

    std::string getText() const override;

    ~IntegerLiteral();

  };

  class QualifiedName : public Node {
  public:

    std::vector<Identifier*> ModulePath;
    Identifier* Name;

    QualifiedName(
      std::vector<Identifier*> ModulePath,
      Identifier* Name
    ): Node(NodeType::QualifiedName),
       ModulePath(ModulePath),
       Name(Name) {}

    Token* getFirstToken() override;
    Token* getLastToken() override;

    void setParents() override;

    ~QualifiedName();

  };

  class TypeExpression : public Node {
  public:

    TypeExpression(NodeType Type): Node(Type) {}

    ~TypeExpression();

  };

  class ReferenceTypeExpression : public TypeExpression {
  public:

    QualifiedName* Name;

    ReferenceTypeExpression(
      QualifiedName* Name
    ): TypeExpression(NodeType::ReferenceTypeExpression),
       Name(Name) {}

    void setParents() override;

    Token* getFirstToken() override;
    Token* getLastToken() override;

    ~ReferenceTypeExpression();

  };

  class ArrowTypeExpression : public TypeExpression {
  public:

    std::vector<TypeExpression*> ParamTypes;
    TypeExpression* ReturnType;

    inline ArrowTypeExpression(
      std::vector<TypeExpression*> ParamTypes,
      TypeExpression* ReturnType
    ): TypeExpression(NodeType::ArrowTypeExpression),
       ParamTypes(ParamTypes),
       ReturnType(ReturnType) {}

    void setParents() override;

    Token* getFirstToken() override;
    Token* getLastToken() override;

    ~ArrowTypeExpression();

  };

  class Pattern : public Node {
  public:

    Pattern(NodeType Type): Node(Type) {}

    ~Pattern();

  };

  class BindPattern : public Pattern {
  public:

    Identifier* Name;

    BindPattern(
      Identifier* Name
    ): Pattern(NodeType::BindPattern),
       Name(Name) {}

    void setParents() override;

    Token* getFirstToken() override;
    Token* getLastToken() override;

    ~BindPattern();

  };

  class Expression : public Node {
  public:

    Expression(NodeType Type): Node(Type) {}

    ~Expression();

  };

  class ReferenceExpression : public Expression {
  public:

    QualifiedName* Name;

    ReferenceExpression(
      QualifiedName* Name
    ): Expression(NodeType::ReferenceExpression),
       Name(Name) {}

    void setParents() override;

    Token* getFirstToken() override;
    Token* getLastToken() override;

    ~ReferenceExpression();

  };

  class ConstantExpression : public Expression {
  public:

    Token* Token;

    ConstantExpression(
      class Token* Token
    ): Expression(NodeType::ConstantExpression),
       Token(Token) {}

    void setParents() override;

    class Token* getFirstToken() override;
    class Token* getLastToken() override;

    ~ConstantExpression();

  };

  class CallExpression : public Expression {
  public:

    Expression* Function;
    std::vector<Expression*> Args;

    CallExpression(
      Expression* Function,
      std::vector<Expression*> Args
    ): Expression(NodeType::CallExpression),
       Function(Function),
       Args(Args) {}

    void setParents() override;

    Token* getFirstToken() override;
    Token* getLastToken() override;

    ~CallExpression();

  };

  class InfixExpression : public Expression {
  public:

    Expression* LHS;
    Token* Operator;
    Expression* RHS;

    InfixExpression(Expression* LHS, Token* Operator, Expression* RHS):
      Expression(NodeType::InfixExpression),
      LHS(LHS),
      Operator(Operator),
      RHS(RHS) {}

    void setParents() override;

    Token* getFirstToken() override;
    Token* getLastToken() override;

    ~InfixExpression();

  };

  class UnaryExpression : public Expression {
  public:

    Token* Operator;
    Expression* Argument;

    UnaryExpression(
      Token* Operator,
      Expression* Argument
    ): Expression(NodeType::UnaryExpression),
       Operator(Operator),
       Argument(Argument) {}

    void setParents() override;

    Token* getFirstToken() override;
    Token* getLastToken() override;

    ~UnaryExpression();

  };

  class Statement : public Node {
  public:

    Statement(NodeType Type): Node(Type) {}

    ~Statement();

  };

  class ExpressionStatement : public Statement {
  public:

    Expression* Expression;

    ExpressionStatement(class Expression* Expression):
      Statement(NodeType::ExpressionStatement), Expression(Expression) {}

    void setParents() override;

    Token* getFirstToken() override;
    Token* getLastToken() override;

    ~ExpressionStatement();

  };

  class ReturnStatement : public Statement {
  public:

    ReturnKeyword* ReturnKeyword;
    Expression* Expression;

    ReturnStatement(
      class ReturnKeyword* ReturnKeyword,
      class Expression* Expression
    ): Statement(NodeType::ReturnStatement),
       ReturnKeyword(ReturnKeyword),
       Expression(Expression) {}

    void setParents() override;

    Token* getFirstToken() override;
    Token* getLastToken() override;

    ~ReturnStatement();

  };

  class TypeAssert : public Node {
  public:

    Colon* Colon;
    TypeExpression* TypeExpression;

    TypeAssert(
      class Colon* Colon,
      class TypeExpression* TypeExpression
    ): Node(NodeType::TypeAssert),
       Colon(Colon),
       TypeExpression(TypeExpression) {}

    void setParents() override;

    Token* getFirstToken() override;
    Token* getLastToken() override;

    ~TypeAssert();

  };

  class Param : public Node {
  public:

    Param(Pattern* Pattern, TypeAssert* TypeAssert): Node(NodeType::Param), Pattern(Pattern), TypeAssert(TypeAssert) {}

    Pattern* Pattern;
    TypeAssert* TypeAssert;

    void setParents() override;

    Token* getFirstToken() override;
    Token* getLastToken() override;

    ~Param();

  };

  class LetBody : public Node {
  public:

    LetBody(NodeType Type): Node(Type) {}

    ~LetBody();

  };

  class LetBlockBody : public LetBody {
  public:

    BlockStart* BlockStart;
    std::vector<Node*> Elements;

    LetBlockBody(
      class BlockStart* BlockStart,
      std::vector<Node*> Elements
    ): LetBody(NodeType::LetBlockBody),
       BlockStart(BlockStart),
       Elements(Elements) {}

    void setParents() override;

    Token* getFirstToken() override;
    Token* getLastToken() override;

    ~LetBlockBody();

  };

  class LetExprBody : public LetBody {
  public:

    Equals* Equals;
    Expression* Expression;

    LetExprBody(
      class Equals* Equals,
      class Expression* Expression
    ): LetBody(NodeType::LetExprBody),
       Equals(Equals),
       Expression(Expression) {}

    void setParents() override;

    Token* getFirstToken() override;
    Token* getLastToken() override;

    ~LetExprBody();

  };

  class LetDeclaration : public Node {
  public:

    PubKeyword* PubKeyword;
    LetKeyword* LetKeyword;
    MutKeyword* MutKeyword;
    Pattern* Pattern;
    std::vector<Param*> Params;
    TypeAssert* TypeAssert;
    LetBody* Body;

    LetDeclaration(
      class PubKeyword* PubKeyword,
      class LetKeyword* LetKeywod,
      class MutKeyword* MutKeyword,
      class Pattern* Pattern,
      std::vector<Param*> Params,
      class TypeAssert* TypeAssert,
      LetBody* Body
    ): Node(NodeType::LetDeclaration),
       PubKeyword(PubKeyword),
       LetKeyword(LetKeywod),
       MutKeyword(MutKeyword),
       Pattern(Pattern),
       Params(Params),
       TypeAssert(TypeAssert),
       Body(Body) {}

    void setParents() override;

    Token* getFirstToken() override;
    Token* getLastToken() override;

    ~LetDeclaration();

  };

  class StructDeclField : public Node {
  public:

    StructDeclField(
      Identifier* Name,
      Colon* Colon,
      TypeExpression* TypeExpression
    ): Node(NodeType::StructDeclField),
       Name(Name),
       Colon(Colon),
       TypeExpression(TypeExpression) {}

    Identifier* Name;
    Colon* Colon;
    TypeExpression* TypeExpression;

    void setParents() override;

    Token* getFirstToken() override;
    Token* getLastToken() override;

    ~StructDeclField();

  };

  class StructDecl : public Node {
  public:

    PubKeyword* PubKeyword;
    StructKeyword* StructKeyword;
    Identifier* Name;
    BlockStart* BlockStart;
    std::vector<StructDeclField*> Fields;

    StructDecl(
      class PubKeyword* PubKeyword,
      class StructKeyword* StructKeyword,
      Identifier* Name,
      class BlockStart* BlockStart,
      std::vector<StructDeclField*> Fields
    ): Node(NodeType::StructDecl),
       PubKeyword(PubKeyword),
       StructKeyword(StructKeyword),
       Name(Name),
       BlockStart(BlockStart),
       Fields(Fields) {}

    void setParents() override;

    Token* getFirstToken() override;
    Token* getLastToken() override;

    ~StructDecl();

  };

  class SourceFile : public Node {
  public:
    
    TextFile& File;

    std::vector<Node*> Elements;

    SourceFile(TextFile& File, std::vector<Node*> Elements):
      Node(NodeType::SourceFile), File(File), Elements(Elements) {}

    inline TextFile& getTextFile() {
      return File;
    }

    void setParents() override;

    Token* getFirstToken() override;
    Token* getLastToken() override;

    ~SourceFile();

  };

}

#endif
