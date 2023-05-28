
#include "zen/config.hpp"

#include "bolt/CST.hpp"
#include "bolt/CSTVisitor.hpp"

namespace bolt {

  TextFile::TextFile(ByteString Path, ByteString Text):
    Path(Path), Text(Text) {
      LineOffsets.push_back(0);
      for (size_t I = 0; I < Text.size(); I++) {
        auto Chr = Text[I];
        if (Chr == '\n') {
          LineOffsets.push_back(I+1);
        }
      }
      LineOffsets.push_back(Text.size());
    }

  size_t TextFile::getLineCount() const {
    return LineOffsets.size();
  }

  size_t TextFile::getStartOffset(size_t Line) const {
    return LineOffsets[Line-1];
  }

  size_t TextFile::getLine(size_t Offset) const {
    ZEN_ASSERT(Offset < Text.size());
    for (size_t I = 0; I < LineOffsets.size(); ++I) {
      if (LineOffsets[I] > Offset) {
        return I;
      }
    }
    ZEN_UNREACHABLE
  }

  size_t TextFile::getColumn(size_t Offset) const {
    auto Line = getLine(Offset);
    auto StartOffset = getStartOffset(Line);
    return Offset - StartOffset + 1 ;
  }

  ByteString TextFile::getPath() const {
    return Path;
  }

  ByteString TextFile::getText() const {
    return Text;
  }

  Scope::Scope(Node* Source):
    Source(Source) {
      scan(Source);
    }

  void Scope::addSymbol(ByteString Name, Node* Decl, SymbolKind Kind) {
    Mapping.emplace(Name, std::make_tuple(Decl, Kind));
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
        addSymbol(Decl->Name->getCanonicalText(), Decl, SymbolKind::Class);
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
        visitPattern(Decl->Pattern, Decl);
        break;
      }
      case NodeKind::RecordDeclaration:
      {
        auto Decl = static_cast<RecordDeclaration*>(X);
        addSymbol(Decl->Name->getCanonicalText(), Decl, SymbolKind::Type);
        break;
      }
      case NodeKind::VariantDeclaration:
      {
        auto Decl = static_cast<VariantDeclaration*>(X);
        addSymbol(Decl->Name->getCanonicalText(), Decl, SymbolKind::Type);
        break;
      }
      default:
        ZEN_UNREACHABLE
    }
  }

  void Scope::visitPattern(Pattern* X, Node* Decl) {
    switch (X->getKind()) {
      case NodeKind::BindPattern:
      {
        auto Y = static_cast<BindPattern*>(X);
        addSymbol(Y->Name->Text, Decl, SymbolKind::Var);
        break;
      }
      case NodeKind::NamedPattern:
      {
        auto Y = static_cast<NamedPattern*>(X);
        for (auto P: Y->Patterns) {
          visitPattern(P, Decl);
        }
        break;
      }
      case NodeKind::NestedPattern:
      {
        auto Y = static_cast<NestedPattern*>(X);
        visitPattern(Y->P, Decl);
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

  const SourceFile* Node::getSourceFile() const {
    const  Node* CurrNode = this;
    for (;;) {
      if (CurrNode->Kind == NodeKind::SourceFile) {
        return static_cast<const SourceFile*>(CurrNode);
      }
      CurrNode = CurrNode->Parent;
      ZEN_ASSERT(CurrNode != nullptr);
    }
  }
  SourceFile* Node::getSourceFile() {
    Node* CurrNode = this;
    for (;;) {
      if (CurrNode->Kind == NodeKind::SourceFile) {
        return static_cast<SourceFile*>(CurrNode);
      }
      CurrNode = CurrNode->Parent;
      ZEN_ASSERT(CurrNode != nullptr);
    }
  }

  std::size_t Node::getStartLine() const {
    return getFirstToken()->getStartLine();
  }

  std::size_t Node::getStartColumn() const {
    return getFirstToken()->getStartColumn();
  }

  std::size_t Node::getEndLine() const {
    return getLastToken()->getEndLine();
  }

  std::size_t Node::getEndColumn() const {
    return getLastToken()->getEndColumn();
  }

  TextRange Node::getRange() const {
    return TextRange {
      getFirstToken()->getStartLoc(),
      getLastToken()->getEndLoc(),
    };
  }

  Scope* Node::getScope() {
    return Parent->getScope();
  }

  TextLoc Token::getEndLoc() const {
    auto Loc = StartLoc;
    Loc.advance(getText());
    return Loc;
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

  Token* TypeclassConstraintExpression::getFirstToken() const {
    return Name;
  }

  Token* TypeclassConstraintExpression::getLastToken() const {
    if (!TEs.empty()) {
      return TEs.back()->getLastToken();
    }
    return Name;
  }

  Token* EqualityConstraintExpression::getFirstToken() const {
    return Left->getFirstToken();
  }

  Token* EqualityConstraintExpression::getLastToken() const {
    return Left->getLastToken();
  }

  Token* QualifiedTypeExpression::getFirstToken() const {
    if (!Constraints.empty()) {
      return std::get<0>(Constraints.front())->getFirstToken();
    }
    return TE->getFirstToken();
  }

  Token* QualifiedTypeExpression::getLastToken() const {
    return TE->getLastToken();
  }

  Token* ReferenceTypeExpression::getFirstToken() const {
    if (!ModulePath.empty()) {
      return std::get<0>(ModulePath.front());
    }
    return Name;
  }

  Token* ReferenceTypeExpression::getLastToken() const {
    return Name;
  }

  Token* ArrowTypeExpression::getFirstToken() const {
    if (ParamTypes.size()) {
      return ParamTypes.front()->getFirstToken();
    }
    return ReturnType->getFirstToken();
  }

  Token* ArrowTypeExpression::getLastToken() const {
    return ReturnType->getLastToken();
  }

  Token* AppTypeExpression::getFirstToken() const {
    return Op->getFirstToken();
  }

  Token* AppTypeExpression::getLastToken() const {
    if (Args.size()) {
      return Args.back()->getLastToken();
    }
    return Op->getLastToken();
  }

  Token* VarTypeExpression::getLastToken() const {
    return Name;
  }

  Token* VarTypeExpression::getFirstToken() const {
    return Name;
  }

  Token* NestedTypeExpression::getLastToken() const {
    return LParen;
  }

  Token* NestedTypeExpression::getFirstToken() const {
    return RParen;
  }

  Token* TupleTypeExpression::getLastToken() const {
    return LParen;
  }

  Token* TupleTypeExpression::getFirstToken() const {
    return RParen;
  }

  Token* BindPattern::getFirstToken() const {
    return Name;
  }

  Token* BindPattern::getLastToken() const {
    return Name;
  }

  Token* LiteralPattern::getFirstToken() const {
    return Literal;
  }

  Token* LiteralPattern::getLastToken() const {
    return Literal;
  }

  Token* NamedPattern::getFirstToken() const {
    return Name;
  }

  Token* NamedPattern::getLastToken() const {
    if (Patterns.size()) {
      return Patterns.back()->getLastToken();
    }
    return Name;
  }

  Token* NestedPattern::getFirstToken() const {
    return LParen;
  }

  Token* NestedPattern::getLastToken() const {
    return RParen;
  }

  Token* ReferenceExpression::getFirstToken() const {
    if (!ModulePath.empty()) {
      return std::get<0>(ModulePath.front());
    }
    return Name;
  }

  Token* ReferenceExpression::getLastToken() const {
    return Name;
  }

  Token* MatchCase::getFirstToken() const {
    return Pattern->getFirstToken();
  }

  Token* MatchCase::getLastToken() const {
    return Expression->getLastToken();
  }

  Token* MatchExpression::getFirstToken() const {
    return MatchKeyword;
  }

  Token* MatchExpression::getLastToken() const {
    if (!Cases.empty()) {
      return Cases.back()->getLastToken();
    }
    return BlockStart;
  }

  Token* MemberExpression::getFirstToken() const {
    return E->getFirstToken();
  }

  Token* MemberExpression::getLastToken() const {
    return Name;
  }

  Token* TupleExpression::getFirstToken() const {
    return LParen;
  }

  Token* TupleExpression::getLastToken() const {
    return RParen;
  }

  Token* NestedExpression::getFirstToken() const {
    return LParen;
  }

  Token* NestedExpression::getLastToken() const {
    return RParen;
  }

  Token* ConstantExpression::getFirstToken() const {
    return Token;
  }

  Token* ConstantExpression::getLastToken() const {
    return Token;
  }

  Token* CallExpression::getFirstToken() const {
    return Function->getFirstToken();
  }

  Token* CallExpression::getLastToken() const {
    if (Args.size()) {
      return Args.back()->getLastToken();
    }
    return Function->getLastToken();
  }

  Token* InfixExpression::getFirstToken() const {
    return LHS->getFirstToken();
  }

  Token* InfixExpression::getLastToken() const {
    return RHS->getLastToken();
  }

  Token* PrefixExpression::getFirstToken() const {
    return Operator;
  }

  Token* PrefixExpression::getLastToken() const {
    return Argument->getLastToken();
  }
  
  Token* ExpressionStatement::getFirstToken() const {
    return Expression->getFirstToken();
  }

  Token* ExpressionStatement::getLastToken() const {
    return Expression->getLastToken();
  }

  Token* ReturnStatement::getFirstToken() const {
    return ReturnKeyword;
  }

  Token* ReturnStatement::getLastToken() const {
    if (Expression) {
      return Expression->getLastToken();
    }
    return ReturnKeyword;
  }

  Token* IfStatementPart::getFirstToken() const {
    return Keyword;
  }

  Token* IfStatementPart::getLastToken() const {
    if (Elements.size()) {
      return Elements.back()->getLastToken();
    }
    return BlockStart;
  }

  Token* IfStatement::getFirstToken() const {
    ZEN_ASSERT(Parts.size());
    return Parts.front()->getFirstToken();
  }

  Token* IfStatement::getLastToken() const {
    ZEN_ASSERT(Parts.size());
    return Parts.back()->getLastToken();
  }

  Token* TypeAssert::getFirstToken() const {
    return Colon;
  }

  Token* TypeAssert::getLastToken() const {
    return TypeExpression->getLastToken();
  }

  Token* Parameter::getFirstToken() const {
    return Pattern->getFirstToken();
  }

  Token* Parameter::getLastToken() const {
    if (TypeAssert) {
      return TypeAssert->getLastToken();
    }
    return Pattern->getLastToken();
  }

  Token* LetBlockBody::getFirstToken() const {
    return BlockStart;
  }

  Token* LetBlockBody::getLastToken() const {
    if (Elements.size()) {
      return Elements.back()->getLastToken();
    }
    return BlockStart;
  }

  Token* LetExprBody::getFirstToken() const {
    return Equals;
  }

  Token* LetExprBody::getLastToken() const {
    return Expression->getLastToken();
  }

  Token* LetDeclaration::getFirstToken() const {
    if (PubKeyword) {
      return PubKeyword;
    }
    return LetKeyword;
  }

  Token* LetDeclaration::getLastToken() const {
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

  Token* RecordDeclarationField::getFirstToken() const {
    return Name;
  }

  Token* RecordDeclarationField::getLastToken() const {
    return TypeExpression->getLastToken();
  }

  Token* RecordDeclaration::getFirstToken() const {
    if (PubKeyword) {
      return PubKeyword;
    }
    return StructKeyword;
  }

  Token* RecordDeclaration::getLastToken() const {
    if (Fields.size()) {
      return Fields.back()->getLastToken();
    }
    return BlockStart;
  }

  Token* VariantDeclaration::getFirstToken() const {
    if (PubKeyword) {
      return PubKeyword;
    }
    return EnumKeyword;
  }

  Token* VariantDeclaration::getLastToken() const {
    if (Members.size()) {
      return Members.back()->getLastToken();
    }
    return BlockStart;
  }

  Token* TupleVariantDeclarationMember::getFirstToken() const {
    return Name;
  }

  Token* TupleVariantDeclarationMember::getLastToken() const {
    if (Elements.size()) {
      return Elements.back()->getLastToken();
    }
    return Name;
  }

  Token* RecordVariantDeclarationMember::getFirstToken() const {
    return Name;
  }

  Token* RecordVariantDeclarationMember::getLastToken() const {
    if (Fields.size()) {
      return Fields.back()->getLastToken();
    }
    return BlockStart;
  }

  Token* InstanceDeclaration::getFirstToken() const {
    return InstanceKeyword;
  }

  Token* InstanceDeclaration::getLastToken() const {
    if (!Elements.empty()) {
      return Elements.back()->getLastToken();
    }
    return BlockStart;
  }

  Token* ClassDeclaration::getFirstToken() const {
    if (PubKeyword != nullptr) {
      return PubKeyword;
    }
    return ClassKeyword;
  }

  Token* ClassDeclaration::getLastToken() const {
    if (!Elements.empty()) {
      return Elements.back()->getLastToken();
    }
    return BlockStart;
  }

  Token* SourceFile::getFirstToken() const {
    if (Elements.size()) {
      return Elements.front()->getFirstToken();
    }
    return nullptr;
  }

  Token* SourceFile::getLastToken() const {
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

  std::string EnumKeyword::getText() const {
    return "enum";
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

