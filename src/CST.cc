
#include "zen/config.hpp"

#include "bolt/CST.hpp"
#include "bolt/CSTVisitor.hpp"

namespace bolt {

  Scope::Scope(Node* Source):
    Source(Source) {
      scan(Source);
    }

  void Scope::scan(Node* X) {
    switch (X->getKind()) {
      case NodeKind::ExpressionStatement:
      case NodeKind::ReturnStatement:
      case NodeKind::IfStatement:
        break;
      case NodeKind::SourceFile:
      {
        auto File = static_cast<SourceFile*>(X);
        for (auto Element: File->Elements) {
          scan(Element);
        }
        break;
      }
      case NodeKind::ClassDeclaration:
      {
        auto Decl = static_cast<ClassDeclaration*>(X);
        Mapping.emplace(Decl->Name->getCanonicalText(), std::make_tuple(Decl, SymbolKind::Class));
        for (auto Element: Decl->Elements) {
          scan(Element);
        }
        break;
      }
      case NodeKind::InstanceDeclaration:
        // FIXME is this right?
        break;
      case NodeKind::LetDeclaration:
      {
        auto Decl = static_cast<LetDeclaration*>(X);
        addBindings(Decl->Pattern, Decl);
        break;
      }
      default:
        ZEN_UNREACHABLE
    }
  }

  void Scope::addBindings(Pattern* X, Node* ToInsert) {
    switch (X->getKind()) {
      case NodeKind::BindPattern:
      {
        auto Y = static_cast<BindPattern*>(X);
        Mapping.emplace(Y->Name->Text, std::make_tuple(ToInsert, SymbolKind::Var));
        break;
      }
      case NodeKind::LiteralPattern:
        break;
      default:
        ZEN_UNREACHABLE
    }
  }

  Node* Scope::lookupDirect(SymbolPath Path, SymbolKind Kind) {
    ZEN_ASSERT(Path.Modules.empty());
    auto Match = Mapping.find(Path.Name);
    if (Match != Mapping.end() && std::get<1>(Match->second) == Kind) {
      return std::get<0>(Match->second);
    }
    return nullptr;
  }

  Node* Scope::lookup(SymbolPath Path, SymbolKind Kind) {
    ZEN_ASSERT(Path.Modules.empty());
    auto Curr = this;
    do {
      auto Found= Curr->lookupDirect(Path, Kind);
      if (Found) {
        return Found;
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
      if (CurrNode->Kind == NodeKind::SourceFile) {
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
    return Parent->getScope();
  }

  /* ClassScope& Node::getClassScope() { */
  /*   return Parent->getClassScope(); */
  /* } */

  TextLoc Token::getEndLoc() {
    auto EndLoc = StartLoc;
    EndLoc.advance(getText());
    return EndLoc;
  }

  void Node::setParents() {

    struct SetParentsVisitor : public CSTVisitor<SetParentsVisitor> {

      std::vector<Node*> Parents { nullptr };

      void visit(Node* N) {
        N->Parent = Parents.back();
        Parents.push_back(N);
        visitEachChild(N);
        Parents.pop_back();
      }

    };

    SetParentsVisitor V;
    V.visit(this);

  }

  Node::~Node() {

    struct UnrefVisitor : public CSTVisitor<UnrefVisitor> {

      void visit(Node* N) {
        N->unref();
        visitEachChild(N);
      }

    };

    UnrefVisitor V;
    V.visitEachChild(this);

  }

  bool Identifier::isTypeVar() const {
    for (auto C: Text) {
      if (!((C >= 97 && C <= 122) || C == '_')) {
        return false;
      }
    }
    return true;
  }

  Token* TypeclassConstraintExpression::getFirstToken() {
    return Name;
  }

  Token* TypeclassConstraintExpression::getLastToken() {
    if (!TEs.empty()) {
      return TEs.back()->getLastToken();
    }
    return Name;
  }

  Token* EqualityConstraintExpression::getFirstToken() {
    return Left->getFirstToken();
  }

  Token* EqualityConstraintExpression::getLastToken() {
    return Left->getLastToken();
  }

  Token* QualifiedTypeExpression::getFirstToken() {
    if (!Constraints.empty()) {
      return std::get<0>(Constraints.front())->getFirstToken();
    }
    return TE->getFirstToken();
  }

  Token* QualifiedTypeExpression::getLastToken() {
    return TE->getLastToken();
  }

  Token* ReferenceTypeExpression::getFirstToken() {
    if (!ModulePath.empty()) {
      return std::get<0>(ModulePath.front());
    }
    return Name;
  }

  Token* ReferenceTypeExpression::getLastToken() {
    return Name;
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

  Token* VarTypeExpression::getLastToken() {
    return Name;
  }

  Token* VarTypeExpression::getFirstToken() {
    return Name;
  }

  Token* BindPattern::getFirstToken() {
    return Name;
  }

  Token* BindPattern::getLastToken() {
    return Name;
  }

  Token* LiteralPattern::getFirstToken() {
    return Literal;
  }

  Token* LiteralPattern::getLastToken() {
    return Literal;
  }

  Token* ReferenceExpression::getFirstToken() {
    if (!ModulePath.empty()) {
      return std::get<0>(ModulePath.front());
    }
    return Name;
  }

  Token* ReferenceExpression::getLastToken() {
    return Name;
  }

  Token* MatchCase::getFirstToken() {
    return Pattern->getFirstToken();
  }

  Token* MatchCase::getLastToken() {
    return Expression->getLastToken();
  }

  Token* MatchExpression::getFirstToken() {
    return MatchKeyword;
  }

  Token* MatchExpression::getLastToken() {
    if (!Cases.empty()) {
      return Cases.back()->getLastToken();
    }
    return BlockStart;
  }

  Token* MemberExpression::getFirstToken() {
    return E->getFirstToken();
  }

  Token* MemberExpression::getLastToken() {
    return Name;
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

  Token* PrefixExpression::getFirstToken() {
    return Operator;
  }

  Token* PrefixExpression::getLastToken() {
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

  Token* Parameter::getFirstToken() {
    return Pattern->getFirstToken();
  }

  Token* Parameter::getLastToken() {
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

  Token* StructDeclarationField::getFirstToken() {
    return Name;
  }

  Token* StructDeclarationField::getLastToken() {
    return TypeExpression->getLastToken();
  }

  Token* StructDeclaration::getFirstToken() {
    if (PubKeyword) {
      return PubKeyword;
    }
    return StructKeyword;
  }

  Token* StructDeclaration::getLastToken() {
    if (Fields.size()) {
      Fields.back()->getLastToken();
    }
    return BlockStart;
  }

  Token* InstanceDeclaration::getFirstToken() {
    return InstanceKeyword;
  }

  Token* InstanceDeclaration::getLastToken() {
    if (!Elements.empty()) {
      return Elements.back()->getLastToken();
    }
    return BlockStart;
  }

  Token* ClassDeclaration::getFirstToken() {
    if (PubKeyword != nullptr) {
      return PubKeyword;
    }
    return ClassKeyword;
  }

  Token* ClassDeclaration::getLastToken() {
    if (!Elements.empty()) {
      return Elements.back()->getLastToken();
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

  std::string Comma::getText() const {
    return ",";
  }

  std::string RArrow::getText() const {
    return "->";
  }

  std::string RArrowAlt::getText() const {
    return "=>";
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

  std::string MatchKeyword::getText() const {
    return "match";
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

  std::string IdentifierAlt::getText() const {
    return Text;
  }

  std::string StringLiteral::getText() const {
    return "\"" + Text + "\"";
  }

  std::string IntegerLiteral::getText() const {
    return std::to_string(V);
  }

  std::string DotDot::getText() const {
    return "..";
  }

  std::string Tilde::getText() const {
    return "~";
  }

  std::string ClassKeyword::getText() const {
    return "class";
  }

  std::string InstanceKeyword::getText() const {
    return "instance";
  }

  ByteString Identifier::getCanonicalText() {
    return Text;
  }

  ByteString IdentifierAlt::getCanonicalText() {
    return Text;
  }

  Value StringLiteral::getValue() {
    return Text;
  }

  Value IntegerLiteral::getValue() {
    return V;
  }

  SymbolPath ReferenceExpression::getSymbolPath() const {
    std::vector<ByteString> ModuleNames;
    for (auto [Name, Dot]: ModulePath) {
      ModuleNames.push_back(Name->getCanonicalText());
    }
    return SymbolPath { ModuleNames, Name->getCanonicalText() };
  }

}

