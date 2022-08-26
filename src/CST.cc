
#include "zen/config.hpp"

#include "bolt/CST.hpp"

namespace bolt {

  Node* Scope::lookup(SymbolPath Path) {
    auto Curr = this;
    do {
      auto Match = Curr->Mapping.find(Path.Name);
      if (Match != Curr->Mapping.end()) {
        return Match->second;
      }
      Curr = Curr->getParentScope();
    } while (Curr != nullptr);
    return nullptr;
  }

  Scope* Scope::getParentScope() {
    if (Source->Parent == nullptr) {
      return nullptr;
    }
    return Source->Parent->getScope();
  }

  SourceFile* Node::getSourceFile() {
    auto CurrNode = this;
    for (;;) {
      if (CurrNode->Type == NodeType::SourceFile) {
        return static_cast<SourceFile*>(CurrNode);
      }
      CurrNode = CurrNode->Parent;
      ZEN_ASSERT(CurrNode != nullptr);
    } 
  }

  TextRange Node::getRange() {
    return TextRange {
      getFirstToken()->getStartLoc(),
      getLastToken()->getEndLoc(),
    };
  }

  Scope* Node::getScope() {
    return this->Parent->getScope();
  }

  TextLoc Token::getEndLoc() {
    auto EndLoc = StartLoc;
    EndLoc.advance(getText());
    return EndLoc;
  }

  void Token::setParents() {
  }

  void QualifiedName::setParents() {
    for (auto Name: ModulePath) {
      Name->Parent = this;
    }
    Name->Parent = this;
  }

  void ReferenceTypeExpression::setParents() {
    Name->Parent = this;
    Name->setParents();
  }
  
  void ArrowTypeExpression::setParents() {
    for (auto ParamType: ParamTypes) {
      ParamType->Parent = this;
      ParamType->setParents();
    }
    ReturnType->Parent = this;
    ReturnType->setParents();
  }

  void BindPattern::setParents() {
    Name->Parent = this;
  }

  void ReferenceExpression::setParents() {
    Name->Parent = this;
  }

  void NestedExpression::setParents() {
    LParen->Parent = this;
    Inner->Parent = this;
    Inner->setParents();
    RParen->Parent = this;
  }

  void ConstantExpression::setParents() {
    Token->Parent = this;
  }

  void CallExpression::setParents() {
    Function->Parent = this;
    Function->setParents();
    for (auto Arg: Args) {
      Arg->Parent = this;
      Arg->setParents();
    }
  }

  void InfixExpression::setParents() {
    LHS->Parent = this;
    LHS->setParents();
    Operator->Parent = this;
    RHS->Parent = this;
    RHS->setParents();
  }

  void UnaryExpression::setParents() {
    Operator->Parent = this;
    Argument->Parent = this;
    Argument->setParents();
  }

  void ExpressionStatement::setParents() {
    Expression->Parent = this;
    Expression->setParents();
  }

  void ReturnStatement::setParents() {
    ReturnKeyword->Parent = this;
    Expression->Parent = this;
    Expression->setParents();
  }

  void IfStatementPart::setParents() {
    Keyword->Parent = this;
    if (Test) {
      Test->Parent = this;
      Test->setParents();
    }
    BlockStart->Parent = this;
    for (auto Element: Elements) {
      Element->Parent = this;
      Element->setParents();
    }
  }

  void IfStatement::setParents() {
    for (auto Part: Parts) {
      Part->Parent = this;
      Part->setParents();
    }
  }

  void TypeAssert::setParents() {
    Colon->Parent = this;
    TypeExpression->Parent = this;
    TypeExpression->setParents();
  }

  void LetBlockBody::setParents() {
    BlockStart->Parent = this;
    for (auto Element: Elements) {
      Element->Parent = this;
      Element->setParents();
    }
  }

  void LetExprBody::setParents() {
    Equals->Parent = this;
    Expression->Parent = this;
    Expression->setParents();
  }

  void Param::setParents() {
    Pattern->Parent = this;
    Pattern->setParents();
    if (TypeAssert) {
      TypeAssert->Parent = this;
      TypeAssert->setParents();
    }
  }

  void LetDeclaration::setParents() {
    if (PubKeyword) {
      PubKeyword->Parent = this;
    }
    LetKeyword->Parent = this;
    if (MutKeyword) {
      MutKeyword->Parent = this;
    }
    Pattern->Parent = this;
    Pattern->setParents();
    for (auto Param: Params) {
      Param->Parent = this;
      Param->setParents();
    }
    if (TypeAssert) {
      TypeAssert->Parent = this;
      TypeAssert->setParents();
    }
    if (Body) {
      Body->Parent = this;
      Body->setParents();
    }
  }

  void StructDeclField::setParents() {
    Name->Parent = this;
    Colon->Parent = this;
    TypeExpression->Parent = this;
    TypeExpression->setParents();
  }

  void StructDecl::setParents() {
    StructKeyword->Parent = this;
    Name->Parent = this;
    BlockStart->Parent = this;
    for (auto Field: Fields) {
      Field->Parent = this;
      Field->setParents();
    }
  }

  void SourceFile::setParents() {
    for (auto Element: Elements) {
      Element->Parent = this;
      Element->setParents();
    }
  }

  Node::~Node() {
  }

  Token::~Token() {
  }

  Equals::~Equals() {
  }

  Colon::~Colon() {
  }

  RArrow::~RArrow() {
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

  IfKeyword::~IfKeyword() {
  }

  ElifKeyword::~ElifKeyword() {
  }

  ElseKeyword::~ElseKeyword() {
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

  ArrowTypeExpression::~ArrowTypeExpression() {
    for (auto ParamType: ParamTypes) {
      ParamType->unref();
    }
    ReturnType->unref();
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

  NestedExpression::~NestedExpression() {
    LParen->unref();
    Inner->unref();
    RParen->unref();
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

  IfStatementPart::~IfStatementPart() {
    Keyword->unref();
    if (Test) {
      Test->unref();
    }
    BlockStart->unref();
    for (auto Element: Elements) {
      Element->unref();
    }
  }

  IfStatement::~IfStatement() {
    for (auto Part: Parts) {
      Part->unref();
    }
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
    LetKeyword->unref();
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
    BlockStart->unref();
    for (auto& Element: Fields){
      Element->unref();
    }
  }

  SourceFile::~SourceFile() {
    for (auto& Element: Elements){
      Element->unref();
    }
  }

  Token* QualifiedName::getFirstToken() {
    if (ModulePath.size()) {
      return ModulePath.front();
    }
    return Name;
  }

  Token* QualifiedName::getLastToken() {
    return Name;
  }

  Token* ReferenceTypeExpression::getFirstToken() {
    return Name->getFirstToken();
  }

  Token* ReferenceTypeExpression::getLastToken() {
    return Name->getFirstToken();
  }

  Token* ArrowTypeExpression::getFirstToken() {
    if (ParamTypes.size()) {
      return ParamTypes.front()->getFirstToken();
    }
    return ReturnType->getFirstToken();
  }

  Token* ArrowTypeExpression::getLastToken() {
    return ReturnType->getLastToken();
  }

  Token* BindPattern::getFirstToken() {
    return Name;
  }

  Token* BindPattern::getLastToken() {
    return Name;
  }

  Token* ReferenceExpression::getFirstToken() {
    return Name->getFirstToken();
  }

  Token* ReferenceExpression::getLastToken() {
    return Name->getLastToken();
  }

  Token* NestedExpression::getFirstToken() {
    return LParen;
  }

  Token* NestedExpression::getLastToken() {
    return RParen;
  }

  Token* ConstantExpression::getFirstToken() {
    return Token;
  }

  Token* ConstantExpression::getLastToken() {
    return Token;
  }

  Token* CallExpression::getFirstToken() {
    return Function->getFirstToken();
  }

  Token* CallExpression::getLastToken() {
    if (Args.size()) {
      return Args.back()->getLastToken();
    }
    return Function->getLastToken();
  }

  Token* InfixExpression::getFirstToken() {
    return LHS->getFirstToken();
  }

  Token* InfixExpression::getLastToken() {
    return RHS->getLastToken();
  }

  Token* UnaryExpression::getFirstToken() {
    return Operator;
  }

  Token* UnaryExpression::getLastToken() {
    return Argument->getLastToken();
  }
  
  Token* ExpressionStatement::getFirstToken() {
    return Expression->getFirstToken();
  }

  Token* ExpressionStatement::getLastToken() {
    return Expression->getLastToken();
  }

  Token* ReturnStatement::getFirstToken() {
    return ReturnKeyword;
  }

  Token* ReturnStatement::getLastToken() {
    if (Expression) {
      return Expression->getLastToken();
    }
    return ReturnKeyword;
  }

  Token* IfStatementPart::getFirstToken() {
    return Keyword;
  }

  Token* IfStatementPart::getLastToken() {
    if (Elements.size()) {
      return Elements.back()->getLastToken();
    }
    return BlockStart;
  }

  Token* IfStatement::getFirstToken() {
    ZEN_ASSERT(Parts.size());
    return Parts.front()->getFirstToken();
  }

  Token* IfStatement::getLastToken() {
    ZEN_ASSERT(Parts.size());
    return Parts.back()->getLastToken();
  }

  Token* TypeAssert::getFirstToken() {
    return Colon;
  }

  Token* TypeAssert::getLastToken() {
    return TypeExpression->getLastToken();
  }

  Token* Param::getFirstToken() {
    return Pattern->getFirstToken();
  }

  Token* Param::getLastToken() {
    if (TypeAssert) {
      return TypeAssert->getLastToken();
    }
    return Pattern->getLastToken();
  }

  Token* LetBlockBody::getFirstToken() {
    return BlockStart;
  }

  Token* LetBlockBody::getLastToken() {
    if (Elements.size()) {
      return Elements.back()->getLastToken();
    }
    return BlockStart;
  }

  Token* LetExprBody::getFirstToken() {
    return Equals;
  }

  Token* LetExprBody::getLastToken() {
    return Expression->getLastToken();
  }

  Token* LetDeclaration::getFirstToken() {
    if (PubKeyword) {
      return PubKeyword;
    }
    return LetKeyword;
  }

  Token* LetDeclaration::getLastToken() {
    if (Body) {
      return Body->getLastToken();
    }
    if (TypeAssert) {
      return TypeAssert->getLastToken();
    }
    if (Params.size()) {
      return Params.back()->getLastToken();
    }
    return Pattern->getLastToken();
  }

  Token* StructDeclField::getFirstToken() {
    return Name;
  }

  Token* StructDeclField::getLastToken() {
    return TypeExpression->getLastToken();
  }

  Token* StructDecl::getFirstToken() {
    if (PubKeyword) {
      return PubKeyword;
    }
    return StructKeyword;
  }

  Token* StructDecl::getLastToken() {
    if (Fields.size()) {
      Fields.back()->getLastToken();
    }
    return BlockStart;
  }

  Token* SourceFile::getFirstToken() {
    if (Elements.size()) {
      return Elements.front()->getFirstToken();
    }
    return nullptr;
  }

  Token* SourceFile::getLastToken() {
    if (Elements.size()) {
      return Elements.back()->getLastToken();
    }
    return nullptr;
  }

  std::string Equals::getText() const {
    return "=";
  }

  std::string Colon::getText() const {
    return ":";
  }

  std::string RArrow::getText() const {
    return "->";
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

  std::string IfKeyword::getText() const {
    return "if";
  }

  std::string ElseKeyword::getText() const {
    return "else";
  }

  std::string ElifKeyword::getText() const {
    return "elif";
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

  SymbolPath QualifiedName::getSymbolPath() const { 
    std::vector<ByteString> ModuleNames;
    for (auto Ident: ModulePath) {
      ModuleNames.push_back(Ident->Text);
    }
    return SymbolPath { ModuleNames, Name->Text };
  }

}

