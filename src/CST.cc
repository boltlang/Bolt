
#include "bolt/CST.hpp"

namespace bolt {

  Node::~Node() { }

  Token::~Token() {
  }

  Equals::~Equals() {
  }

  Colon::~Colon() {
  }

  Dot::~Dot() {
  }

  DotDot::~DotDot() {
  }

  LParen::~LParen() {
  }

  RParen::~RParen() {
  }

  LBracket::~LBracket() {
  }

  RBracket::~RBracket() {
  }

  LBrace::~LBrace() {
  }

  RBrace::~RBrace() {
  }

  LetKeyword::~LetKeyword() {
  }

  MutKeyword::~MutKeyword() {
  }

  PubKeyword::~PubKeyword() {
  }

  TypeKeyword::~TypeKeyword() {
  }

  ReturnKeyword::~ReturnKeyword() {
  }

  ModKeyword::~ModKeyword() {
  }

  StructKeyword::~StructKeyword() {
  }

  Invalid::~Invalid() {
  }

  EndOfFile::~EndOfFile() {
  }

  BlockStart::~BlockStart() {
  }

  BlockEnd::~BlockEnd() {
  }

  LineFoldEnd::~LineFoldEnd() {
  }

  CustomOperator::~CustomOperator() {
  }

  Assignment::~Assignment() {
  }

  Identifier::~Identifier() {
  }

  StringLiteral::~StringLiteral() {
  }

  IntegerLiteral::~IntegerLiteral() {
  }

  QualifiedName::~QualifiedName() {
    for (auto& Element: ModulePath){
      Element->unref();
    }
    Name->unref();
  }

  TypeExpression::~TypeExpression() {
  }

  ReferenceTypeExpression::~ReferenceTypeExpression() {
    Name->unref();
  }

  Pattern::~Pattern() {
  }

  BindPattern::~BindPattern() {
    Name->unref();
  }

  Expression::~Expression() {
  }

  ReferenceExpression::~ReferenceExpression() {
    Name->unref();
  }

  ConstantExpression::~ConstantExpression() {
    Token->unref();
  }

  CallExpression::~CallExpression() {
    Function->unref();
    for (auto& Element: Args){
      Element->unref();
    }
  }

  InfixExpression::~InfixExpression() {
    LHS->unref();
    Operator->unref();
    RHS->unref();
  }

  UnaryExpression::~UnaryExpression() {
    Operator->unref();
    Argument->unref();
  }

  Statement::~Statement() {
  }

  ExpressionStatement::~ExpressionStatement() {
    Expression->unref();
  }

  ReturnStatement::~ReturnStatement() {
    ReturnKeyword->unref();
    Expression->unref();
  }

  TypeAssert::~TypeAssert() {
    Colon->unref();
    TypeExpression->unref();
  }

  Param::~Param() {
    Pattern->unref();
    TypeAssert->unref();
  }

  LetBody::~LetBody() {
  }

  LetBlockBody::~LetBlockBody() {
    BlockStart->unref();
    for (auto& Element: Elements){
      Element->unref();
    }
  }

  LetExprBody::~LetExprBody() {
    Equals->unref();
    Expression->unref();
  }

  LetDeclaration::~LetDeclaration() {
    if (PubKeyword) {
      PubKeyword->unref();
    }
    LetKeywod->unref();
    if (MutKeyword) {
      MutKeyword->unref();
    }
    Pattern->unref();
    for (auto& Element: Params){
      Element->unref();
    }
    if (TypeAssert) {
      TypeAssert->unref();
    }
    if (Body) {
      Body->unref();
    }
  }

  StructDeclField::~StructDeclField() {
    Name->unref();
    Colon->unref();
    TypeExpression->unref();
  }

  StructDecl::~StructDecl() {
    StructKeyword->unref();
    Name->unref();
    Dot->unref();
    for (auto& Element: Fields){
      Element->unref();
    }
  }

  SourceFile::~SourceFile() {
    for (auto& Element: Elements){
      Element->unref();
    }
  }

  std::string Equals::getText() const {
    return "=";
  }

  std::string Colon::getText() const {
    return ":";
  }

  std::string Dot::getText() const {
    return ".";
  }

  std::string LParen::getText() const {
    return "(";
  }

  std::string RParen::getText() const {
    return ")";
  }

  std::string LBracket::getText() const {
    return "[";
  }

  std::string RBracket::getText() const {
    return "]";
  }

  std::string LBrace::getText() const {
    return "{";
  }

  std::string RBrace::getText() const {
    return "}";
  }

  std::string LetKeyword::getText() const {
    return "let";
  }

  std::string MutKeyword::getText() const {
    return "mut";
  }

  std::string PubKeyword::getText() const {
    return "pub";
  }

  std::string TypeKeyword::getText() const {
    return "type";
  }

  std::string ReturnKeyword::getText() const {
    return "return";
  }

  std::string ModKeyword::getText() const {
    return "mod";
  }

  std::string StructKeyword::getText() const {
    return "struct";
  }

  std::string Invalid::getText() const {
    return "";
  }

  std::string EndOfFile::getText() const {
    return "";
  }

  std::string BlockStart::getText() const {
    return ".";
  }

  std::string BlockEnd::getText() const {
    return "";
  }

  std::string LineFoldEnd::getText() const {
    return "";
  }

  std::string CustomOperator::getText() const {
    return Text;
  }
  
  std::string Assignment::getText() const {
    return Text + "=";
  }

  std::string Identifier::getText() const {
    return Text;
  }

  std::string StringLiteral::getText() const {
    return "\"" + Text + "\"";
  }

  std::string IntegerLiteral::getText() const {
    return std::to_string(Value);
  }

  std::string DotDot::getText() const {
    return "..";
  }

}

