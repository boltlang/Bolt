
#pragma once

#include "bolt/CST.hpp"

namespace bolt {

template<typename D, typename R = void>
class CSTVisitor {
public:

  void visit(Node* N) {

#define BOLT_GEN_CASE(name) \
    case NodeKind::name: \
      return static_cast<D*>(this)->visit ## name(static_cast<name*>(N));

#define BOLT_VISIT(node) static_cast<D*>(this)->visit(node)
#define BOLT_VISIT_SYMBOL(node) static_cast<D*>(this)->dispatchSymbol(node)
#define BOLT_VISIT_OPERATOR(node) static_cast<D*>(this)->dispatchOperator(node)

    switch (N->getKind()) {
      BOLT_GEN_CASE(AppTypeExpression)
      BOLT_GEN_CASE(ArrowTypeExpression)
      BOLT_GEN_CASE(Assignment)
      BOLT_GEN_CASE(At)
      BOLT_GEN_CASE(Backslash)
      BOLT_GEN_CASE(BindPattern)
      BOLT_GEN_CASE(BlockEnd)
      BOLT_GEN_CASE(BlockExpression)
      BOLT_GEN_CASE(BlockStart)
      BOLT_GEN_CASE(CallExpression)
      BOLT_GEN_CASE(ClassDeclaration)
      BOLT_GEN_CASE(ClassKeyword)
      BOLT_GEN_CASE(Colon)
      BOLT_GEN_CASE(Comma)
      BOLT_GEN_CASE(CustomOperator)
      BOLT_GEN_CASE(DoKeyword)
      BOLT_GEN_CASE(Dot)
      BOLT_GEN_CASE(DotDot)
      BOLT_GEN_CASE(ElifKeyword)
      BOLT_GEN_CASE(ElseKeyword)
      BOLT_GEN_CASE(EndOfFile)
      BOLT_GEN_CASE(EnumKeyword)
      BOLT_GEN_CASE(EqualityConstraintExpression)
      BOLT_GEN_CASE(Equals)
      BOLT_GEN_CASE(ExpressionAnnotation)
      BOLT_GEN_CASE(FnKeyword)
      BOLT_GEN_CASE(ForeignKeyword)
      BOLT_GEN_CASE(FunctionExpression)
      BOLT_GEN_CASE(Identifier)
      BOLT_GEN_CASE(IdentifierAlt)
      BOLT_GEN_CASE(IfExpression)
      BOLT_GEN_CASE(IfExpressionPart)
      BOLT_GEN_CASE(IfKeyword)
      BOLT_GEN_CASE(InfixExpression)
      BOLT_GEN_CASE(InfixFunctionDeclaration)
      BOLT_GEN_CASE(InstanceDeclaration)
      BOLT_GEN_CASE(InstanceKeyword)
      BOLT_GEN_CASE(IntegerLiteral)
      BOLT_GEN_CASE(Invalid)
      BOLT_GEN_CASE(LBrace)
      BOLT_GEN_CASE(LBracket)
      BOLT_GEN_CASE(LParen)
      BOLT_GEN_CASE(LetBlockBody)
      BOLT_GEN_CASE(LetExprBody)
      BOLT_GEN_CASE(LetKeyword)
      BOLT_GEN_CASE(LineFoldEnd)
      BOLT_GEN_CASE(ListPattern)
      BOLT_GEN_CASE(LiteralExpression)
      BOLT_GEN_CASE(LiteralPattern)
      BOLT_GEN_CASE(MatchCase)
      BOLT_GEN_CASE(MatchExpression)
      BOLT_GEN_CASE(MatchKeyword)
      BOLT_GEN_CASE(MemberExpression)
      BOLT_GEN_CASE(ModKeyword)
      BOLT_GEN_CASE(MutKeyword)
      BOLT_GEN_CASE(NamedFunctionDeclaration)
      BOLT_GEN_CASE(NamedRecordPattern)
      BOLT_GEN_CASE(NamedTuplePattern)
      BOLT_GEN_CASE(NestedExpression)
      BOLT_GEN_CASE(NestedPattern)
      BOLT_GEN_CASE(NestedTypeExpression)
      BOLT_GEN_CASE(Parameter)
      BOLT_GEN_CASE(PrefixExpression)
      BOLT_GEN_CASE(PrefixFunctionDeclaration)
      BOLT_GEN_CASE(PubKeyword)
      BOLT_GEN_CASE(QualifiedTypeExpression)
      BOLT_GEN_CASE(RArrow)
      BOLT_GEN_CASE(RArrowAlt)
      BOLT_GEN_CASE(RBrace)
      BOLT_GEN_CASE(RBracket)
      BOLT_GEN_CASE(RParen)
      BOLT_GEN_CASE(RecordDeclaration)
      BOLT_GEN_CASE(RecordDeclarationField)
      BOLT_GEN_CASE(RecordExpression)
      BOLT_GEN_CASE(RecordExpressionField)
      BOLT_GEN_CASE(RecordPattern)
      BOLT_GEN_CASE(RecordPatternField)
      BOLT_GEN_CASE(RecordTypeExpression)
      BOLT_GEN_CASE(RecordTypeExpressionField)
      BOLT_GEN_CASE(RecordVariantDeclarationMember)
      BOLT_GEN_CASE(ReferenceExpression)
      BOLT_GEN_CASE(ReferenceTypeExpression)
      BOLT_GEN_CASE(ReturnExpression)
      BOLT_GEN_CASE(ReturnKeyword)
      BOLT_GEN_CASE(SourceFile)
      BOLT_GEN_CASE(StringLiteral)
      BOLT_GEN_CASE(StructKeyword)
      BOLT_GEN_CASE(SuffixFunctionDeclaration)
      BOLT_GEN_CASE(Tilde)
      BOLT_GEN_CASE(TupleExpression)
      BOLT_GEN_CASE(TuplePattern)
      BOLT_GEN_CASE(TupleTypeExpression)
      BOLT_GEN_CASE(TupleVariantDeclarationMember)
      BOLT_GEN_CASE(TypeAssert)
      BOLT_GEN_CASE(TypeAssertAnnotation)
      BOLT_GEN_CASE(TypeKeyword)
      BOLT_GEN_CASE(TypeclassConstraintExpression)
      BOLT_GEN_CASE(VBar)
      BOLT_GEN_CASE(VarTypeExpression)
      BOLT_GEN_CASE(VariableDeclaration)
      BOLT_GEN_CASE(VariantDeclaration)
      BOLT_GEN_CASE(WrappedOperator)
    }
  }

  void dispatchSymbol(const Symbol& S) {
    switch (S.getKind()) {
      case NodeKind::Identifier:
        BOLT_VISIT(S.asIdentifier());
        break;
      case NodeKind::IdentifierAlt:
        BOLT_VISIT(S.asIdentifierAlt());
        break;
      case NodeKind::WrappedOperator:
        BOLT_VISIT(S.asWrappedOperator());
        break;
      default:
        ZEN_UNREACHABLE
    }
  }

  void dispatchOperator(const Operator& O) {
    switch (O.getKind()) {
      case NodeKind::VBar:
        BOLT_VISIT(O.asVBar());
        break;
      case NodeKind::CustomOperator:
        BOLT_VISIT(O.asCustomOperator());
        break;
      default:
        ZEN_UNREACHABLE
    }
  }

protected:

  void visitNode(Node* N) {
    visitEachChild(N);
  }

  void visitToken(Token* N) {
    static_cast<D*>(this)->visitNode(N);
  }

  void visitVBar(VBar* N) {
    static_cast<D*>(this)->visitToken(N);
  }

  void visitEquals(Equals* N) {
    static_cast<D*>(this)->visitToken(N);
  }

  void visitColon(Colon* N) {
    static_cast<D*>(this)->visitToken(N);
  }

  void visitComma(Comma* N) {
    static_cast<D*>(this)->visitToken(N);
  }

  void visitDot(Dot* N) {
    static_cast<D*>(this)->visitToken(N);
  }

  void visitDotDot(DotDot* N) {
    static_cast<D*>(this)->visitToken(N);
  }

  void visitTilde(Tilde* N) {
    static_cast<D*>(this)->visitToken(N);
  }

  void visitAt(At* N) {
    static_cast<D*>(this)->visitToken(N);
  }

  void visitBackslash(Backslash* N) {
    static_cast<D*>(this)->visitToken(N);
  }

  void visitDoKeyword(DoKeyword* N) {
    static_cast<D*>(this)->visitToken(N);
  }

  void visitLParen(LParen* N) {
    static_cast<D*>(this)->visitToken(N);
  }

  void visitRParen(RParen* N) {
    static_cast<D*>(this)->visitToken(N);
  }

  void visitLBracket(LBracket* N) {
    static_cast<D*>(this)->visitToken(N);
  }

  void visitRBracket(RBracket* N) {
    static_cast<D*>(this)->visitToken(N);
  }

  void visitLBrace(LBrace* N) {
    static_cast<D*>(this)->visitToken(N);
  }

  void visitRBrace(RBrace* N) {
    static_cast<D*>(this)->visitToken(N);
  }

  void visitRArrow(RArrow* N) {
    static_cast<D*>(this)->visitToken(N);
  }

  void visitRArrowAlt(RArrowAlt* N) {
    static_cast<D*>(this)->visitToken(N);
  }

  void visitLetKeyword(LetKeyword* N) {
    static_cast<D*>(this)->visitToken(N);
  }

  void visitForeignKeyword(ForeignKeyword* N) {
    static_cast<D*>(this)->visitToken(N);
  }

  void visitMutKeyword(MutKeyword* N) {
    static_cast<D*>(this)->visitToken(N);
  }

  void visitPubKeyword(PubKeyword* N) {
    static_cast<D*>(this)->visitToken(N);
  }

  void visitTypeKeyword(TypeKeyword* N) {
    static_cast<D*>(this)->visitToken(N);
  }

  void visitReturnKeyword(ReturnKeyword* N) {
    static_cast<D*>(this)->visitToken(N);
  }

  void visitModKeyword(ModKeyword* N) {
    static_cast<D*>(this)->visitToken(N);
  }

  void visitStructKeyword(StructKeyword* N) {
    static_cast<D*>(this)->visitToken(N);
  }

  void visitEnumKeyword(EnumKeyword* N) {
    static_cast<D*>(this)->visitToken(N);
  }

  void visitFnKeyword(FnKeyword* N) {
    static_cast<D*>(this)->visitToken(N);
  }

  void visitClassKeyword(ClassKeyword* N) {
    static_cast<D*>(this)->visitToken(N);
  }

  void visitInstanceKeyword(InstanceKeyword* N) {
    static_cast<D*>(this)->visitToken(N);
  }

  void visitElifKeyword(ElifKeyword* N) {
    static_cast<D*>(this)->visitToken(N);
  }

  void visitIfKeyword(IfKeyword* N) {
    static_cast<D*>(this)->visitToken(N);
  }

  void visitElseKeyword(ElseKeyword* N) {
    static_cast<D*>(this)->visitToken(N);
  }

  void visitMatchKeyword(MatchKeyword* N) {
    static_cast<D*>(this)->visitToken(N);
  }

  void visitInvalid(Invalid* N) {
    static_cast<D*>(this)->visitToken(N);
  }

  void visitEndOfFile(EndOfFile* N) {
    static_cast<D*>(this)->visitToken(N);
  }

  void visitBlockStart(BlockStart* N) {
    static_cast<D*>(this)->visitToken(N);
  }

  void visitBlockEnd(BlockEnd* N) {
    static_cast<D*>(this)->visitToken(N);
  }

  void visitLineFoldEnd(LineFoldEnd* N) {
    static_cast<D*>(this)->visitToken(N);
  }

  void visitCustomOperator(CustomOperator* N) {
    static_cast<D*>(this)->visitToken(N);
  }

  void visitAssignment(Assignment* N) {
    static_cast<D*>(this)->visitToken(N);
  }

  void visitIdentifier(Identifier* N) {
    static_cast<D*>(this)->visitToken(N);
  }

  void visitIdentifierAlt(IdentifierAlt* N) {
    static_cast<D*>(this)->visitToken(N);
  }

  void visitStringLiteral(StringLiteral* N) {
    static_cast<D*>(this)->visitToken(N);
  }

  void visitIntegerLiteral(IntegerLiteral* N) {
    static_cast<D*>(this)->visitToken(N);
  }

  void visitAnnotation(Annotation* N) {
    static_cast<D*>(this)->visitNode(N);
  }

  void visitTypeAssertAnnotation(TypeAssertAnnotation* N) {
    static_cast<D*>(this)->visitAnnotation(N);
  }

  void visitExpressionAnnotation(ExpressionAnnotation* N) {
    static_cast<D*>(this)->visitAnnotation(N);
  }

  void visitConstraintExpression(ConstraintExpression* N) {
    static_cast<D*>(this)->visitNode(N);
  }

  void visitTypeclassConstraintExpression(TypeclassConstraintExpression* N) {
    static_cast<D*>(this)->visitConstraintExpression(N);
  }

  void visitEqualityConstraintExpression(EqualityConstraintExpression* N) {
    static_cast<D*>(this)->visitConstraintExpression(N);
  }

  void visitTypeExpression(TypeExpression* N) {
    static_cast<D*>(this)->visitNode(N);
  }

  void visitRecordTypeExpressionField(RecordTypeExpressionField * N) {
    static_cast<D*>(this)->visitNode(N);
  }

  void visitRecordTypeExpression(RecordTypeExpression* N) {
    static_cast<D*>(this)->visitTypeExpression(N);
  }

  void visitQualifiedTypeExpression(QualifiedTypeExpression* N) {
    static_cast<D*>(this)->visitTypeExpression(N);
  }

  void visitReferenceTypeExpression(ReferenceTypeExpression* N) {
    static_cast<D*>(this)->visitTypeExpression(N);
  }

  void visitArrowTypeExpression(ArrowTypeExpression* N) {
    static_cast<D*>(this)->visitTypeExpression(N);
  }

  void visitAppTypeExpression(AppTypeExpression* N) {
    static_cast<D*>(this)->visitTypeExpression(N);
  }

  void visitVarTypeExpression(VarTypeExpression* N) {
    static_cast<D*>(this)->visitTypeExpression(N);
  }

  void visitNestedTypeExpression(NestedTypeExpression* N) {
    static_cast<D*>(this)->visitTypeExpression(N);
  }

  void visitTupleTypeExpression(TupleTypeExpression* N) {
    static_cast<D*>(this)->visitTypeExpression(N);
  }

  void visitWrappedOperator(WrappedOperator* N) {
    static_cast<D*>(this)->visitNode(N);
  }

  void visitPattern(Pattern* N) {
    static_cast<D*>(this)->visitNode(N);
  }

  void visitBindPattern(BindPattern* N) {
    static_cast<D*>(this)->visitPattern(N);
  }

  void visitLiteralPattern(LiteralPattern* N) {
    static_cast<D*>(this)->visitPattern(N);
  }

  void visitRecordPatternField(RecordPatternField* N) {
    static_cast<D*>(this)->visitNode(N);
  }

  void visitRecordPattern(RecordPattern* N) {
    static_cast<D*>(this)->visitPattern(N);
  }

  void visitNamedRecordPattern(NamedRecordPattern* N) {
    static_cast<D*>(this)->visitPattern(N);
  }

  void visitNamedTuplePattern(NamedTuplePattern* N) {
    static_cast<D*>(this)->visitPattern(N);
  }

  void visitTuplePattern(TuplePattern* N) {
    static_cast<D*>(this)->visitPattern(N);
  }

  void visitNestedPattern(NestedPattern* N) {
    static_cast<D*>(this)->visitPattern(N);
  }

  void visitListPattern(ListPattern* N) {
    static_cast<D*>(this)->visitPattern(N);
  }

  void visitExpression(Expression* N) {
    static_cast<D*>(this)->visitNode(N);
  }

  void visitReferenceExpression(ReferenceExpression* N) {
    static_cast<D*>(this)->visitExpression(N);
  }

  void visitMatchCase(MatchCase* N) {
    static_cast<D*>(this)->visitNode(N);
  }

  void visitMatchExpression(MatchExpression* N) {
    static_cast<D*>(this)->visitExpression(N);
  }

  void visitBlockExpression(BlockExpression* N) {
    static_cast<D*>(this)->visitExpression(N);
  }

  void visitMemberExpression(MemberExpression* N) {
    static_cast<D*>(this)->visitExpression(N);
  }

  void visitTupleExpression(TupleExpression* N) {
    static_cast<D*>(this)->visitExpression(N);
  }

  void visitNestedExpression(NestedExpression* N) {
    static_cast<D*>(this)->visitExpression(N);
  }

  void visitLiteralExpression(LiteralExpression* N) {
    static_cast<D*>(this)->visitExpression(N);
  }

  void visitCallExpression(CallExpression* N) {
    static_cast<D*>(this)->visitExpression(N);
  }

  void visitFunctionExpression(FunctionExpression* N) {
    static_cast<D*>(this)->visitExpression(N);
  }

  void visitInfixExpression(InfixExpression* N) {
    static_cast<D*>(this)->visitExpression(N);
  }

  void visitPrefixExpression(PrefixExpression* N) {
    static_cast<D*>(this)->visitExpression(N);
  }

  void visitRecordExpressionField(RecordExpressionField* N) {
    static_cast<D*>(this)->visitNode(N);
  }

  void visitRecordExpression(RecordExpression* N) {
    static_cast<D*>(this)->visitExpression(N);
  }

  void visitReturnExpression(ReturnExpression* N) {
    static_cast<D*>(this)->visitExpression(N);
  }

  void visitIfExpression(IfExpression* N) {
    static_cast<D*>(this)->visitExpression(N);
  }

  void visitIfExpressionPart(IfExpressionPart* N) {
    static_cast<D*>(this)->visitNode(N);
  }

  void visitTypeAssert(TypeAssert* N) {
    static_cast<D*>(this)->visitNode(N);
  }

  void visitParameter(Parameter* N) {
    static_cast<D*>(this)->visitNode(N);
  }

  void visitLetBody(LetBody* N) {
    static_cast<D*>(this)->visitNode(N);
  }

  void visitLetBlockBody(LetBlockBody* N) {
    static_cast<D*>(this)->visitLetBody(N);
  }

  void visitLetExprBody(LetExprBody* N) {
    static_cast<D*>(this)->visitLetBody(N);
  }

  void visitFunctionDeclaration(FunctionDeclaration* N) {
    static_cast<D*>(this)->visitNode(N);
  }

  void visitPrefixFunctionDeclaration(PrefixFunctionDeclaration* N) {
    static_cast<D*>(this)->visitFunctionDeclaration(N);
  }

  void visitInfixFunctionDeclaration(InfixFunctionDeclaration* N) {
    static_cast<D*>(this)->visitFunctionDeclaration(N);
  }

  void visitSuffixFunctionDeclaration(SuffixFunctionDeclaration* N) {
    static_cast<D*>(this)->visitFunctionDeclaration(N);
  }

  void visitNamedFunctionDeclaration(NamedFunctionDeclaration* N) {
    static_cast<D*>(this)->visitFunctionDeclaration(N);
  }

  void visitVariableDeclaration(VariableDeclaration* N) {
    static_cast<D*>(this)->visitNode(N);
  }

  void visitRecordDeclarationField(RecordDeclarationField* N) {
    static_cast<D*>(this)->visitNode(N);
  }

  void visitRecordDeclaration(RecordDeclaration* N) {
    static_cast<D*>(this)->visitNode(N);
  }

  void visitVariantDeclaration(VariantDeclaration* N) {
    static_cast<D*>(this)->visitNode(N);
  }

  void visitVariantDeclarationMember(VariantDeclarationMember* N) {
    static_cast<D*>(this)->visitNode(N);
  }

  void visitTupleVariantDeclarationMember(TupleVariantDeclarationMember* N) {
    static_cast<D*>(this)->visitVariantDeclarationMember(N);
  }

  void visitRecordVariantDeclarationMember(RecordVariantDeclarationMember* N) {
    static_cast<D*>(this)->visitVariantDeclarationMember(N);
  }

  void visitClassDeclaration(ClassDeclaration* N) {
    static_cast<D*>(this)->visitNode(N);
  }

  void visitInstanceDeclaration(InstanceDeclaration* N) {
    static_cast<D*>(this)->visitNode(N);
  }

  void visitSourceFile(SourceFile* N) {
    static_cast<D*>(this)->visitNode(N);
  }

public:

  void visitEachChild(Node* N) {

#define BOLT_GEN_CHILD_CASE(name) \
    case NodeKind::name: \
      visitEachChildImpl(static_cast<name*>(N)); \
      break;

    switch (N->getKind()) {
      BOLT_GEN_CHILD_CASE(AppTypeExpression)
      BOLT_GEN_CHILD_CASE(ArrowTypeExpression)
      BOLT_GEN_CHILD_CASE(Assignment)
      BOLT_GEN_CHILD_CASE(At)
      BOLT_GEN_CHILD_CASE(Backslash)
      BOLT_GEN_CHILD_CASE(BindPattern)
      BOLT_GEN_CHILD_CASE(BlockEnd)
      BOLT_GEN_CHILD_CASE(BlockExpression)
      BOLT_GEN_CHILD_CASE(BlockStart)
      BOLT_GEN_CHILD_CASE(CallExpression)
      BOLT_GEN_CHILD_CASE(ClassDeclaration)
      BOLT_GEN_CHILD_CASE(ClassKeyword)
      BOLT_GEN_CHILD_CASE(Colon)
      BOLT_GEN_CHILD_CASE(Comma)
      BOLT_GEN_CHILD_CASE(CustomOperator)
      BOLT_GEN_CHILD_CASE(DoKeyword)
      BOLT_GEN_CHILD_CASE(Dot)
      BOLT_GEN_CHILD_CASE(DotDot)
      BOLT_GEN_CHILD_CASE(ElifKeyword)
      BOLT_GEN_CHILD_CASE(ElseKeyword)
      BOLT_GEN_CHILD_CASE(EndOfFile)
      BOLT_GEN_CHILD_CASE(EnumKeyword)
      BOLT_GEN_CHILD_CASE(EqualityConstraintExpression)
      BOLT_GEN_CHILD_CASE(Equals)
      BOLT_GEN_CHILD_CASE(ExpressionAnnotation)
      BOLT_GEN_CHILD_CASE(FnKeyword)
      BOLT_GEN_CHILD_CASE(ForeignKeyword)
      BOLT_GEN_CHILD_CASE(FunctionExpression)
      BOLT_GEN_CHILD_CASE(Identifier)
      BOLT_GEN_CHILD_CASE(IdentifierAlt)
      BOLT_GEN_CHILD_CASE(IfExpression)
      BOLT_GEN_CHILD_CASE(IfExpressionPart)
      BOLT_GEN_CHILD_CASE(IfKeyword)
      BOLT_GEN_CHILD_CASE(InfixExpression)
      BOLT_GEN_CHILD_CASE(InfixFunctionDeclaration)
      BOLT_GEN_CHILD_CASE(InstanceDeclaration)
      BOLT_GEN_CHILD_CASE(InstanceKeyword)
      BOLT_GEN_CHILD_CASE(IntegerLiteral)
      BOLT_GEN_CHILD_CASE(Invalid)
      BOLT_GEN_CHILD_CASE(LBrace)
      BOLT_GEN_CHILD_CASE(LBracket)
      BOLT_GEN_CHILD_CASE(LParen)
      BOLT_GEN_CHILD_CASE(LetBlockBody)
      BOLT_GEN_CHILD_CASE(LetExprBody)
      BOLT_GEN_CHILD_CASE(LetKeyword)
      BOLT_GEN_CHILD_CASE(LineFoldEnd)
      BOLT_GEN_CHILD_CASE(ListPattern)
      BOLT_GEN_CHILD_CASE(LiteralExpression)
      BOLT_GEN_CHILD_CASE(LiteralPattern)
      BOLT_GEN_CHILD_CASE(MatchCase)
      BOLT_GEN_CHILD_CASE(MatchExpression)
      BOLT_GEN_CHILD_CASE(MatchKeyword)
      BOLT_GEN_CHILD_CASE(MemberExpression)
      BOLT_GEN_CHILD_CASE(ModKeyword)
      BOLT_GEN_CHILD_CASE(MutKeyword)
      BOLT_GEN_CHILD_CASE(NamedFunctionDeclaration)
      BOLT_GEN_CHILD_CASE(NamedRecordPattern)
      BOLT_GEN_CHILD_CASE(NamedTuplePattern)
      BOLT_GEN_CHILD_CASE(NestedExpression)
      BOLT_GEN_CHILD_CASE(NestedPattern)
      BOLT_GEN_CHILD_CASE(NestedTypeExpression)
      BOLT_GEN_CHILD_CASE(Parameter)
      BOLT_GEN_CHILD_CASE(PrefixExpression)
      BOLT_GEN_CHILD_CASE(PrefixFunctionDeclaration)
      BOLT_GEN_CHILD_CASE(PubKeyword)
      BOLT_GEN_CHILD_CASE(QualifiedTypeExpression)
      BOLT_GEN_CHILD_CASE(RArrow)
      BOLT_GEN_CHILD_CASE(RArrowAlt)
      BOLT_GEN_CHILD_CASE(RBrace)
      BOLT_GEN_CHILD_CASE(RBracket)
      BOLT_GEN_CHILD_CASE(RParen)
      BOLT_GEN_CHILD_CASE(RecordDeclaration)
      BOLT_GEN_CHILD_CASE(RecordDeclarationField)
      BOLT_GEN_CHILD_CASE(RecordExpression)
      BOLT_GEN_CHILD_CASE(RecordExpressionField)
      BOLT_GEN_CHILD_CASE(RecordPattern)
      BOLT_GEN_CHILD_CASE(RecordPatternField)
      BOLT_GEN_CHILD_CASE(RecordTypeExpression)
      BOLT_GEN_CHILD_CASE(RecordTypeExpressionField)
      BOLT_GEN_CHILD_CASE(RecordVariantDeclarationMember)
      BOLT_GEN_CHILD_CASE(ReferenceExpression)
      BOLT_GEN_CHILD_CASE(ReferenceTypeExpression)
      BOLT_GEN_CHILD_CASE(ReturnExpression)
      BOLT_GEN_CHILD_CASE(ReturnKeyword)
      BOLT_GEN_CHILD_CASE(SourceFile)
      BOLT_GEN_CHILD_CASE(StringLiteral)
      BOLT_GEN_CHILD_CASE(StructKeyword)
      BOLT_GEN_CHILD_CASE(SuffixFunctionDeclaration)
      BOLT_GEN_CHILD_CASE(Tilde)
      BOLT_GEN_CHILD_CASE(TupleExpression)
      BOLT_GEN_CHILD_CASE(TuplePattern)
      BOLT_GEN_CHILD_CASE(TupleTypeExpression)
      BOLT_GEN_CHILD_CASE(TupleVariantDeclarationMember)
      BOLT_GEN_CHILD_CASE(TypeAssert)
      BOLT_GEN_CHILD_CASE(TypeAssertAnnotation)
      BOLT_GEN_CHILD_CASE(TypeKeyword)
      BOLT_GEN_CHILD_CASE(TypeclassConstraintExpression)
      BOLT_GEN_CHILD_CASE(VBar)
      BOLT_GEN_CHILD_CASE(VarTypeExpression)
      BOLT_GEN_CHILD_CASE(VariableDeclaration)
      BOLT_GEN_CHILD_CASE(VariantDeclaration)
      BOLT_GEN_CHILD_CASE(WrappedOperator)
    }
  }

  void visitEachChildImpl(VBar* N) {
  }

  void visitEachChildImpl(Equals* N) {
  }

  void visitEachChildImpl(Colon* N) {
  }

  void visitEachChildImpl(Comma* N) {
  }

  void visitEachChildImpl(Dot* N) {
  }

  void visitEachChildImpl(DotDot* N) {
  }

  void visitEachChildImpl(Tilde* N) {
  }

  void visitEachChildImpl(At* N) {
  }

  void visitEachChildImpl(Backslash* N) {
  }

  void visitEachChildImpl(DoKeyword* N) {
  }

  void visitEachChildImpl(LParen* N) {
  }

  void visitEachChildImpl(RParen* N) {
  }

  void visitEachChildImpl(LBracket* N) {
  }

  void visitEachChildImpl(RBracket* N) {
  }

  void visitEachChildImpl(LBrace* N) {
  }

  void visitEachChildImpl(RBrace* N) {
  }

  void visitEachChildImpl(RArrow* N) {
  }

  void visitEachChildImpl(RArrowAlt* N) {
  }

  void visitEachChildImpl(LetKeyword* N) {
  }

  void visitEachChildImpl(ForeignKeyword* N) {
  }

  void visitEachChildImpl(MutKeyword* N) {
  }

  void visitEachChildImpl(PubKeyword* N) {
  }

  void visitEachChildImpl(TypeKeyword* N) {
  }

  void visitEachChildImpl(ReturnKeyword* N) {
  }

  void visitEachChildImpl(ModKeyword* N) {
  }

  void visitEachChildImpl(StructKeyword* N) {
  }

  void visitEachChildImpl(EnumKeyword* N) {
  }

  void visitEachChildImpl(FnKeyword* N) {
  }

  void visitEachChildImpl(ClassKeyword* N) {
  }

  void visitEachChildImpl(InstanceKeyword* N) {
  }

  void visitEachChildImpl(ElifKeyword* N) {
  }

  void visitEachChildImpl(IfKeyword* N) {
  }

  void visitEachChildImpl(ElseKeyword* N) {
  }

  void visitEachChildImpl(MatchKeyword* N) {
  }

  void visitEachChildImpl(Invalid* N) {
  }

  void visitEachChildImpl(EndOfFile* N) {
  }

  void visitEachChildImpl(BlockStart* N) {
  }

  void visitEachChildImpl(BlockEnd* N) {
  }

  void visitEachChildImpl(LineFoldEnd* N) {
  }

  void visitEachChildImpl(CustomOperator* N) {
  }

  void visitEachChildImpl(Assignment* N) {
  }

  void visitEachChildImpl(Identifier* N) {
  }

  void visitEachChildImpl(IdentifierAlt* N) {
  }

  void visitEachChildImpl(StringLiteral* N) {
  }

  void visitEachChildImpl(IntegerLiteral* N) {
  }

  void visitEachChildImpl(WrappedOperator* N) {
    BOLT_VISIT(N->LParen);
    BOLT_VISIT_OPERATOR(N->Op);
    BOLT_VISIT(N->RParen);
  }

  void visitEachChildImpl(ExpressionAnnotation* N) {
    BOLT_VISIT(N->At);
    BOLT_VISIT(N->Expression);
  }

  void visitEachChildImpl(TypeAssertAnnotation* N) {
    BOLT_VISIT(N->At);
    BOLT_VISIT(N->Colon);
    BOLT_VISIT(N->TE);
  }

  void visitEachChildImpl(TypeclassConstraintExpression* N) {
    BOLT_VISIT(N->Name);
    for (auto TE: N->TEs) {
      BOLT_VISIT(TE);
    }
  }

  void visitEachChildImpl(EqualityConstraintExpression* N) {
    BOLT_VISIT(N->Left);
    BOLT_VISIT(N->Tilde);
    BOLT_VISIT(N->Right);
  }

  void visitEachChildImpl(RecordTypeExpressionField* N) {
    BOLT_VISIT(N->Name);
    BOLT_VISIT(N->Colon);
    BOLT_VISIT(N->TE);
  }

  void visitEachChildImpl(RecordTypeExpression* N) {
    BOLT_VISIT(N->LBrace);
    for (auto [Field, Comma]: N->Fields) {
      BOLT_VISIT(Field);
      if (Comma) {
        BOLT_VISIT(Comma);
      }
    }
    if (N->VBar) {
      BOLT_VISIT(N->VBar);
    }
    if (N->Rest) {
      BOLT_VISIT(N->Rest);
    }
    BOLT_VISIT(N->RBrace);
  }

  void visitEachChildImpl(QualifiedTypeExpression* N) {
    for (auto [CE, Comma]: N->Constraints) {
      BOLT_VISIT(CE);
      if (Comma) {
        BOLT_VISIT(Comma);
      }
    }
    BOLT_VISIT(N->RArrowAlt);
    BOLT_VISIT(N->TE);
  }

  void visitEachChildImpl(ReferenceTypeExpression* N) {
    for (auto [Name, Dot]: N->ModulePath) {
      BOLT_VISIT(Name);
      BOLT_VISIT(Dot);
    }
    BOLT_VISIT(N->Name);
  }

  void visitEachChildImpl(ArrowTypeExpression* N) {
    for (auto PT: N->ParamTypes) {
      BOLT_VISIT(PT);
    }
    BOLT_VISIT(N->ReturnType);
  }

  void visitEachChildImpl(AppTypeExpression* N) {
    BOLT_VISIT(N->Op);
    for (auto Arg: N->Args) {
      BOLT_VISIT(Arg);
    }
  }

  void visitEachChildImpl(VarTypeExpression* N) {
    BOLT_VISIT(N->Name);
  }

  void visitEachChildImpl(NestedTypeExpression* N) {
    BOLT_VISIT(N->LParen);
    BOLT_VISIT(N->TE);
    BOLT_VISIT(N->RParen);
  }

  void visitEachChildImpl(TupleTypeExpression* N) {
    BOLT_VISIT(N->LParen);
    for (auto [TE, Comma]: N->Elements) {
      if (Comma) {
        BOLT_VISIT(Comma);
      }
      BOLT_VISIT(TE);
    }
    BOLT_VISIT(N->RParen);
  }

  void visitEachChildImpl(BindPattern* N) {
    BOLT_VISIT(N->Name);
  }

  void visitEachChildImpl(LiteralPattern* N) {
    BOLT_VISIT(N->Literal);
  }

  void visitEachChildImpl(RecordPatternField* N) {
    if (N->DotDot) {
      BOLT_VISIT(N->DotDot);
    }
    if (N->Name) {
      BOLT_VISIT(N->Name);
    }
    if (N->Equals) {
      BOLT_VISIT(N->Equals);
    }
    if (N->Pattern) {
      BOLT_VISIT(N->Pattern);
    }
  }

  void visitEachChildImpl(RecordPattern* N) {
    BOLT_VISIT(N->LBrace);
    for (auto [Field, Comma]: N->Fields) {
      BOLT_VISIT(Field);
      if (Comma) {
        BOLT_VISIT(Comma);
      }
    }
    BOLT_VISIT(N->RBrace);
  }

  void visitEachChildImpl(NamedRecordPattern* N) {
    for (auto [Name, Dot]: N->ModulePath) {
      BOLT_VISIT(Name);
      if (Dot) {
        BOLT_VISIT(Dot);
      }
    }
    BOLT_VISIT(N->Name);
    BOLT_VISIT(N->LBrace);
    for (auto [Field, Comma]: N->Fields) {
      BOLT_VISIT(Field);
      if (Comma) {
        BOLT_VISIT(Comma);
      }
    }
    BOLT_VISIT(N->LBrace);
    BOLT_VISIT(N->RBrace);
  }

  void visitEachChildImpl(NamedTuplePattern* N) {
    BOLT_VISIT(N->Name);
    for (auto P: N->Patterns) {
      BOLT_VISIT(P);
    }
  }

  void visitEachChildImpl(TuplePattern* N) {
    BOLT_VISIT(N->LParen);
    for (auto [P, Comma]: N->Elements) {
      BOLT_VISIT(P);
      if (Comma) {
        BOLT_VISIT(Comma);
      }
    }
    BOLT_VISIT(N->RParen);
  }

  void visitEachChildImpl(NestedPattern* N) {
    BOLT_VISIT(N->LParen);
    BOLT_VISIT(N->P);
    BOLT_VISIT(N->RParen);
  }

  void visitEachChildImpl(ListPattern* N) {
    BOLT_VISIT(N->LBracket);
    for (auto [Element, Separator]: N->Elements) {
      BOLT_VISIT(Element);
      if (Separator) {
        BOLT_VISIT(Separator);
      }
    }
    BOLT_VISIT(N->RBracket);
  }

  void visitEachChildImpl(ReferenceExpression* N) {
    for (auto A: N->Annotations) {
      BOLT_VISIT(A);
    }
    for (auto [Name, Dot]: N->ModulePath) {
      BOLT_VISIT(Name);
      BOLT_VISIT(Dot);
    }
    BOLT_VISIT_SYMBOL(N->Name);
  }

  void visitEachChildImpl(MatchCase* N) {
    BOLT_VISIT(N->Pattern);
    BOLT_VISIT(N->RArrowAlt);
    BOLT_VISIT(N->Expression);
  }

  void visitEachChildImpl(MatchExpression* N) {
    for (auto A: N->Annotations) {
      BOLT_VISIT(A);
    }
    BOLT_VISIT(N->MatchKeyword);
    if (N->Value) {
      BOLT_VISIT(N->Value);
    }
    BOLT_VISIT(N->BlockStart);
    for (auto Case: N->Cases) {
      BOLT_VISIT(Case);
    }
  }

  void visitEachChildImpl(BlockExpression* N) {
    for (auto A: N->Annotations) {
      BOLT_VISIT(A);
    }
    BOLT_VISIT(N->DoKeyword);
    BOLT_VISIT(N->BlockStart);
    for (auto Element: N->Elements) {
      BOLT_VISIT(Element);
    }
  }

  void visitEachChildImpl(MemberExpression* N) {
    for (auto A: N->Annotations) {
      BOLT_VISIT(A);
    }
    BOLT_VISIT(N->getExpression());
    BOLT_VISIT(N->Dot);
    BOLT_VISIT(N->Name);
  }

  void visitEachChildImpl(TupleExpression* N) {
    for (auto A: N->Annotations) {
      BOLT_VISIT(A);
    }
    BOLT_VISIT(N->LParen);
    for (auto [E, Comma]: N->Elements) {
      BOLT_VISIT(E);
      if (Comma) {
        BOLT_VISIT(Comma);
      }
    }
    BOLT_VISIT(N->RParen);
  }

  void visitEachChildImpl(NestedExpression* N) {
    for (auto A: N->Annotations) {
      BOLT_VISIT(A);
    }
    BOLT_VISIT(N->LParen);
    BOLT_VISIT(N->Inner);
    BOLT_VISIT(N->RParen);
  }

  void visitEachChildImpl(LiteralExpression* N) {
    for (auto A: N->Annotations) {
      BOLT_VISIT(A);
    }
    BOLT_VISIT(N->Token);
  }

  void visitEachChildImpl(CallExpression* N) {
    for (auto A: N->Annotations) {
      BOLT_VISIT(A);
    }
    BOLT_VISIT(N->Function);
    for (auto Arg: N->Args) {
      BOLT_VISIT(Arg);
    }
  }

  void visitEachChildImpl(FunctionExpression* N) {
    for (auto A: N->Annotations) {
      BOLT_VISIT(A);
    }
    BOLT_VISIT(N->Backslash);
    for (auto P: N->getParameters()) {
      BOLT_VISIT(P);
    }
    BOLT_VISIT(N->RArrow);
    BOLT_VISIT(N->getExpression());
  }

  void visitEachChildImpl(InfixExpression* N) {
    for (auto A: N->Annotations) {
      BOLT_VISIT(A);
    }
    BOLT_VISIT(N->Left);
    BOLT_VISIT_OPERATOR(N->Operator);
    BOLT_VISIT(N->Right);
  }

  void visitEachChildImpl(PrefixExpression* N) {
    for (auto A: N->Annotations) {
      BOLT_VISIT(A);
    }
    BOLT_VISIT(N->Operator);
    BOLT_VISIT(N->Argument);
  }

  void visitEachChildImpl(RecordExpressionField* N) {
    BOLT_VISIT(N->Name);
    BOLT_VISIT(N->Equals);
    BOLT_VISIT(N->E);
  }

  void visitEachChildImpl(RecordExpression* N) {
    for (auto A: N->Annotations) {
      BOLT_VISIT(A);
    }
    BOLT_VISIT(N->LBrace);
    for (auto [Field, Comma]: N->Fields) {
      BOLT_VISIT(Field);
      if (Comma) {
        BOLT_VISIT(Comma);
      }
    }
    BOLT_VISIT(N->RBrace);
  }

  void visitEachChildImpl(ReturnExpression* N) {
    for (auto A: N->Annotations) {
      BOLT_VISIT(A);
    }
    BOLT_VISIT(N->ReturnKeyword);
    BOLT_VISIT(N->E);
  }

  void visitEachChildImpl(IfExpression* N) {
    for (auto Part: N->Parts) {
      BOLT_VISIT(Part);
    }
  }

  void visitEachChildImpl(IfExpressionPart* N) {
    for (auto A: N->Annotations) {
      BOLT_VISIT(A);
    }
    BOLT_VISIT(N->Keyword);
    if (N->Test != nullptr) {
      BOLT_VISIT(N->Test);
    }
    BOLT_VISIT(N->BlockStart);
    for (auto Element: N->Elements) {
      BOLT_VISIT(Element);
    }
  }

  void visitEachChildImpl(TypeAssert* N) {
    BOLT_VISIT(N->Colon);
    BOLT_VISIT(N->TypeExpression);
  }

  void visitEachChildImpl(Parameter* N) {
    BOLT_VISIT(N->Pattern);
    if (N->TypeAssert != nullptr) {
      BOLT_VISIT(N->TypeAssert);
    }
  }

  void visitEachChildImpl(LetBlockBody* N) {
    BOLT_VISIT(N->BlockStart);
    for (auto Element: N->Elements) {
      BOLT_VISIT(Element);
    }
  }

  void visitEachChildImpl(LetExprBody* N) {
    BOLT_VISIT(N->Equals);
    BOLT_VISIT(N->Expression);
  }

  void visitEachChildImpl(PrefixFunctionDeclaration* N) {
    for (auto A: N->Annotations) {
      BOLT_VISIT(A);
    }
    if (N->PubKeyword) {
      BOLT_VISIT(N->PubKeyword);
    }
    if (N->ForeignKeyword) {
      BOLT_VISIT(N->ForeignKeyword);
    }
    BOLT_VISIT(N->FnKeyword);
    BOLT_VISIT(N->Param);
    BOLT_VISIT_OPERATOR(N->Name);
    if (N->TypeAssert) {
      BOLT_VISIT(N->TypeAssert);
    }
    if (N->Body) {
      BOLT_VISIT(N->Body);
    }
  }

  void visitEachChildImpl(InfixFunctionDeclaration* N) {
    for (auto A: N->Annotations) {
      BOLT_VISIT(A);
    }
    if (N->PubKeyword) {
      BOLT_VISIT(N->PubKeyword);
    }
    if (N->ForeignKeyword) {
      BOLT_VISIT(N->ForeignKeyword);
    }
    BOLT_VISIT(N->FnKeyword);
    BOLT_VISIT(N->Left);
    BOLT_VISIT_OPERATOR(N->Name);
    BOLT_VISIT(N->Right);
    if (N->TypeAssert) {
      BOLT_VISIT(N->TypeAssert);
    }
    if (N->Body) {
      BOLT_VISIT(N->Body);
    }
  }

  void visitEachChildImpl(SuffixFunctionDeclaration* N) {
    for (auto A: N->Annotations) {
      BOLT_VISIT(A);
    }
    if (N->PubKeyword) {
      BOLT_VISIT(N->PubKeyword);
    }
    if (N->ForeignKeyword) {
      BOLT_VISIT(N->ForeignKeyword);
    }
    BOLT_VISIT(N->FnKeyword);
    BOLT_VISIT_OPERATOR(N->Name);
    BOLT_VISIT(N->Param);
    if (N->TypeAssert) {
      BOLT_VISIT(N->TypeAssert);
    }
    if (N->Body) {
      BOLT_VISIT(N->Body);
    }
  }

  void visitEachChildImpl(NamedFunctionDeclaration* N) {
    for (auto A: N->Annotations) {
      BOLT_VISIT(A);
    }
    if (N->PubKeyword) {
      BOLT_VISIT(N->PubKeyword);
    }
    if (N->ForeignKeyword) {
      BOLT_VISIT(N->ForeignKeyword);
    }
    BOLT_VISIT(N->FnKeyword);
    BOLT_VISIT_SYMBOL(N->Name);
    for (auto Param: N->Params) {
      BOLT_VISIT(Param);
    }
    if (N->TypeAssert) {
      BOLT_VISIT(N->TypeAssert);
    }
    if (N->Body) {
      BOLT_VISIT(N->Body);
    }
  }

  void visitEachChildImpl(VariableDeclaration* N) {
    for (auto A: N->Annotations) {
      BOLT_VISIT(A);
    }
    if (N->PubKeyword) {
      BOLT_VISIT(N->PubKeyword);
    }
    BOLT_VISIT(N->LetKeyword);
    if (N->MutKeyword) {
      BOLT_VISIT(N->MutKeyword);
    }
    BOLT_VISIT(N->Pattern);
    if (N->TypeAssert) {
      BOLT_VISIT(N->TypeAssert);
    }
    if (N->Body) {
      BOLT_VISIT(N->Body);
    }
  }

  void visitEachChildImpl(RecordDeclarationField* N) {
    BOLT_VISIT(N->Name);
    BOLT_VISIT(N->Colon);
    BOLT_VISIT(N->TypeExpression);
  }

  void visitEachChildImpl(RecordDeclaration* N) {
    if (N->PubKeyword) {
      BOLT_VISIT(N->PubKeyword);
    }
    BOLT_VISIT(N->StructKeyword);
    BOLT_VISIT(N->Name);
    BOLT_VISIT(N->StructKeyword);
    for (auto Field: N->Fields) {
      BOLT_VISIT(Field);
    }
  }

  void visitEachChildImpl(VariantDeclaration* N) {
    if (N->PubKeyword) {
      BOLT_VISIT(N->PubKeyword);
    }
    BOLT_VISIT(N->EnumKeyword);
    BOLT_VISIT(N->Name);
    for (auto TV: N->TVs) {
      BOLT_VISIT(TV);
    }
    BOLT_VISIT(N->BlockStart);
    for (auto Member: N->Members) {
      BOLT_VISIT(Member);
    }
  }

  void visitEachChildImpl(TupleVariantDeclarationMember* N) {
    BOLT_VISIT(N->Name);
    for (auto Element: N->Elements) {
      BOLT_VISIT(Element);
    }
  }

  void visitEachChildImpl(RecordVariantDeclarationMember* N) {
    BOLT_VISIT(N->Name);
    BOLT_VISIT(N->BlockStart);
    for (auto Field: N->Fields) {
      BOLT_VISIT(Field);
    }
  }

  void visitEachChildImpl(ClassDeclaration* N) {
    if (N->PubKeyword) {
      BOLT_VISIT(N->PubKeyword);
    }
    BOLT_VISIT(N->ClassKeyword);
    BOLT_VISIT(N->Name);
    for (auto Name: N->TypeVars) {
      BOLT_VISIT(Name);
    }
    BOLT_VISIT(N->BlockStart);
    for (auto Element: N->Elements) {
      BOLT_VISIT(Element);
    }
  }

  void visitEachChildImpl(InstanceDeclaration* N) {
    BOLT_VISIT(N->InstanceKeyword);
    BOLT_VISIT(N->Name);
    for (auto TE: N->TypeExps) {
      BOLT_VISIT(TE);
    }
    BOLT_VISIT(N->BlockStart);
    for (auto Element: N->Elements) {
      BOLT_VISIT(Element);
    }
  }

  void visitEachChildImpl(SourceFile* N) {
    for (auto Element: N->Elements) {
      BOLT_VISIT(Element);
    }
  }

};

}
