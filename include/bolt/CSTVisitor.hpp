
#pragma once

#include "bolt/CST.hpp"

namespace bolt {

  template<typename D, typename R = void>
  class CSTVisitor {
  public:

    void visit(Node* N) {
      switch (N->getKind()) {
        case NodeKind::Equals:
          return static_cast<D*>(this)->visitEquals(static_cast<Equals*>(N));
        case NodeKind::Colon:
          return static_cast<D*>(this)->visitColon(static_cast<Colon*>(N));
        case NodeKind::Comma:
          return static_cast<D*>(this)->visitComma(static_cast<Comma*>(N));
        case NodeKind::Dot:
          return static_cast<D*>(this)->visitDot(static_cast<Dot*>(N));
        case NodeKind::DotDot:
          return static_cast<D*>(this)->visitDotDot(static_cast<DotDot*>(N));
        case NodeKind::Tilde:
          return static_cast<D*>(this)->visitTilde(static_cast<Tilde*>(N));
        case NodeKind::LParen:
          return static_cast<D*>(this)->visitLParen(static_cast<LParen*>(N));
        case NodeKind::RParen:
          return static_cast<D*>(this)->visitRParen(static_cast<RParen*>(N));
        case NodeKind::LBracket:
          return static_cast<D*>(this)->visitLBracket(static_cast<LBracket*>(N));
        case NodeKind::RBracket:
          return static_cast<D*>(this)->visitRBracket(static_cast<RBracket*>(N));
        case NodeKind::LBrace:
          return static_cast<D*>(this)->visitLBrace(static_cast<LBrace*>(N));
        case NodeKind::RBrace:
          return static_cast<D*>(this)->visitRBrace(static_cast<RBrace*>(N));
        case NodeKind::RArrow:
          return static_cast<D*>(this)->visitRArrow(static_cast<RArrow*>(N));
        case NodeKind::RArrowAlt:
          return static_cast<D*>(this)->visitRArrowAlt(static_cast<RArrowAlt*>(N));
        case NodeKind::LetKeyword:
          return static_cast<D*>(this)->visitLetKeyword(static_cast<LetKeyword*>(N));
        case NodeKind::MutKeyword:
          return static_cast<D*>(this)->visitMutKeyword(static_cast<MutKeyword*>(N));
        case NodeKind::PubKeyword:
          return static_cast<D*>(this)->visitPubKeyword(static_cast<PubKeyword*>(N));
        case NodeKind::TypeKeyword:
          return static_cast<D*>(this)->visitTypeKeyword(static_cast<TypeKeyword*>(N));
        case NodeKind::ReturnKeyword:
          return static_cast<D*>(this)->visitReturnKeyword(static_cast<ReturnKeyword*>(N));
        case NodeKind::ModKeyword:
          return static_cast<D*>(this)->visitModKeyword(static_cast<ModKeyword*>(N));
        case NodeKind::StructKeyword:
          return static_cast<D*>(this)->visitStructKeyword(static_cast<StructKeyword*>(N));
        case NodeKind::ClassKeyword:
          return static_cast<D*>(this)->visitClassKeyword(static_cast<ClassKeyword*>(N));
        case NodeKind::InstanceKeyword:
          return static_cast<D*>(this)->visitInstanceKeyword(static_cast<InstanceKeyword*>(N));
        case NodeKind::ElifKeyword:
          return static_cast<D*>(this)->visitElifKeyword(static_cast<ElifKeyword*>(N));
        case NodeKind::IfKeyword:
          return static_cast<D*>(this)->visitIfKeyword(static_cast<IfKeyword*>(N));
        case NodeKind::ElseKeyword:
          return static_cast<D*>(this)->visitElseKeyword(static_cast<ElseKeyword*>(N));
        case NodeKind::MatchKeyword:
          return static_cast<D*>(this)->visitMatchKeyword(static_cast<MatchKeyword*>(N));
        case NodeKind::Invalid:
          return static_cast<D*>(this)->visitInvalid(static_cast<Invalid*>(N));
        case NodeKind::EndOfFile:
          return static_cast<D*>(this)->visitEndOfFile(static_cast<EndOfFile*>(N));
        case NodeKind::BlockStart:
          return static_cast<D*>(this)->visitBlockStart(static_cast<BlockStart*>(N));
        case NodeKind::BlockEnd:
          return static_cast<D*>(this)->visitBlockEnd(static_cast<BlockEnd*>(N));
        case NodeKind::LineFoldEnd:
          return static_cast<D*>(this)->visitLineFoldEnd(static_cast<LineFoldEnd*>(N));
        case NodeKind::CustomOperator:
          return static_cast<D*>(this)->visitCustomOperator(static_cast<CustomOperator*>(N));
        case NodeKind::Assignment:
          return static_cast<D*>(this)->visitAssignment(static_cast<Assignment*>(N));
        case NodeKind::Identifier:
          return static_cast<D*>(this)->visitIdentifier(static_cast<Identifier*>(N));
        case NodeKind::IdentifierAlt:
          return static_cast<D*>(this)->visitIdentifierAlt(static_cast<IdentifierAlt*>(N));
        case NodeKind::StringLiteral:
          return static_cast<D*>(this)->visitStringLiteral(static_cast<StringLiteral*>(N));
        case NodeKind::IntegerLiteral:
          return static_cast<D*>(this)->visitIntegerLiteral(static_cast<IntegerLiteral*>(N));
        case NodeKind::TypeclassConstraintExpression:
          return static_cast<D*>(this)->visitTypeclassConstraintExpression(static_cast<TypeclassConstraintExpression*>(N));
        case NodeKind::EqualityConstraintExpression:
          return static_cast<D*>(this)->visitEqualityConstraintExpression(static_cast<EqualityConstraintExpression*>(N));
        case NodeKind::QualifiedTypeExpression:
          return static_cast<D*>(this)->visitQualifiedTypeExpression(static_cast<QualifiedTypeExpression*>(N));
        case NodeKind::ReferenceTypeExpression:
          return static_cast<D*>(this)->visitReferenceTypeExpression(static_cast<ReferenceTypeExpression*>(N));
        case NodeKind::ArrowTypeExpression:
          return static_cast<D*>(this)->visitArrowTypeExpression(static_cast<ArrowTypeExpression*>(N));
        case NodeKind::VarTypeExpression:
          return static_cast<D*>(this)->visitVarTypeExpression(static_cast<VarTypeExpression*>(N));
        case NodeKind::BindPattern:
          return static_cast<D*>(this)->visitBindPattern(static_cast<BindPattern*>(N));
        case NodeKind::LiteralPattern:
          return static_cast<D*>(this)->visitLiteralPattern(static_cast<LiteralPattern*>(N));
        case NodeKind::ReferenceExpression:
          return static_cast<D*>(this)->visitReferenceExpression(static_cast<ReferenceExpression*>(N));
        case NodeKind::MatchCase:
          return static_cast<D*>(this)->visitMatchCase(static_cast<MatchCase*>(N));
        case NodeKind::MatchExpression:
          return static_cast<D*>(this)->visitMatchExpression(static_cast<MatchExpression*>(N));
        case NodeKind::NestedExpression:
          return static_cast<D*>(this)->visitNestedExpression(static_cast<NestedExpression*>(N));
        case NodeKind::ConstantExpression:
          return static_cast<D*>(this)->visitConstantExpression(static_cast<ConstantExpression*>(N));
        case NodeKind::CallExpression:
          return static_cast<D*>(this)->visitCallExpression(static_cast<CallExpression*>(N));
        case NodeKind::InfixExpression:
          return static_cast<D*>(this)->visitInfixExpression(static_cast<InfixExpression*>(N));
        case NodeKind::PrefixExpression:
          return static_cast<D*>(this)->visitPrefixExpression(static_cast<PrefixExpression*>(N));
        case NodeKind::ExpressionStatement:
          return static_cast<D*>(this)->visitExpressionStatement(static_cast<ExpressionStatement*>(N));
        case NodeKind::ReturnStatement:
          return static_cast<D*>(this)->visitReturnStatement(static_cast<ReturnStatement*>(N));
        case NodeKind::IfStatement:
          return static_cast<D*>(this)->visitIfStatement(static_cast<IfStatement*>(N));
        case NodeKind::IfStatementPart:
          return static_cast<D*>(this)->visitIfStatementPart(static_cast<IfStatementPart*>(N));
        case NodeKind::TypeAssert:
          return static_cast<D*>(this)->visitTypeAssert(static_cast<TypeAssert*>(N));
        case NodeKind::Parameter:
          return static_cast<D*>(this)->visitParameter(static_cast<Parameter*>(N));
        case NodeKind::LetBlockBody:
          return static_cast<D*>(this)->visitLetBlockBody(static_cast<LetBlockBody*>(N));
        case NodeKind::LetExprBody:
          return static_cast<D*>(this)->visitLetExprBody(static_cast<LetExprBody*>(N));
        case NodeKind::LetDeclaration:
          return static_cast<D*>(this)->visitLetDeclaration(static_cast<LetDeclaration*>(N));
        case NodeKind::StructDeclarationField:
          return static_cast<D*>(this)->visitStructDeclarationField(static_cast<StructDeclarationField*>(N));
        case NodeKind::StructDeclaration:
          return static_cast<D*>(this)->visitStructDeclaration(static_cast<StructDeclaration*>(N));
        case NodeKind::ClassDeclaration:
          return static_cast<D*>(this)->visitClassDeclaration(static_cast<ClassDeclaration*>(N));
        case NodeKind::InstanceDeclaration:
          return static_cast<D*>(this)->visitInstanceDeclaration(static_cast<InstanceDeclaration*>(N));
        case NodeKind::SourceFile:
          return static_cast<D*>(this)->visitSourceFile(static_cast<SourceFile*>(N));
     }
    }

  protected:

    void visitNode(Node* N) {
      visitEachChild(N);
    }

    void visitToken(Token* N) {
      visitNode(N);
    }

    void visitEquals(Equals* N) {
      visitToken(N);
    }

    void visitColon(Colon* N) {
      visitToken(N);
    }

    void visitComma(Comma* N) {
      visitToken(N);
    }

    void visitDot(Dot* N) {
      visitToken(N);
    }

    void visitDotDot(DotDot* N) {
      visitToken(N);
    }

    void visitTilde(Tilde* N) {
      visitToken(N);
    }

    void visitLParen(LParen* N) {
      visitToken(N);
    }

    void visitRParen(RParen* N) {
      visitToken(N);
    }

    void visitLBracket(LBracket* N) {
      visitToken(N);
    }

    void visitRBracket(RBracket* N) {
      visitToken(N);
    }

    void visitLBrace(LBrace* N) {
      visitToken(N);
    }

    void visitRBrace(RBrace* N) {
      visitToken(N);
    }

    void visitRArrow(RArrow* N) {
      visitToken(N);
    }

    void visitRArrowAlt(RArrowAlt* N) {
      visitToken(N);
    }

    void visitLetKeyword(LetKeyword* N) {
      visitToken(N);
    }

    void visitMutKeyword(MutKeyword* N) {
      visitToken(N);
    }

    void visitPubKeyword(PubKeyword* N) {
      visitToken(N);
    }

    void visitTypeKeyword(TypeKeyword* N) {
      visitToken(N);
    }

    void visitReturnKeyword(ReturnKeyword* N) {
      visitToken(N);
    }

    void visitModKeyword(ModKeyword* N) {
      visitToken(N);
    }

    void visitStructKeyword(StructKeyword* N) {
      visitToken(N);
    }

    void visitClassKeyword(ClassKeyword* N) {
      visitToken(N);
    }

    void visitInstanceKeyword(InstanceKeyword* N) {
      visitToken(N);
    }

    void visitElifKeyword(ElifKeyword* N) {
      visitToken(N);
    }

    void visitIfKeyword(IfKeyword* N) {
      visitToken(N);
    }

    void visitElseKeyword(ElseKeyword* N) {
      visitToken(N);
    }

    void visitMatchKeyword(MatchKeyword* N) {
      visitToken(N);
    }

    void visitInvalid(Invalid* N) {
      visitToken(N);
    }

    void visitEndOfFile(EndOfFile* N) {
      visitToken(N);
    }

    void visitBlockStart(BlockStart* N) {
      visitToken(N);
    }

    void visitBlockEnd(BlockEnd* N) {
      visitToken(N);
    }

    void visitLineFoldEnd(LineFoldEnd* N) {
      visitToken(N);
    }

    void visitCustomOperator(CustomOperator* N) {
      visitToken(N);
    }

    void visitAssignment(Assignment* N) {
      visitToken(N);
    }

    void visitIdentifier(Identifier* N) {
      visitToken(N);
    }

    void visitIdentifierAlt(IdentifierAlt* N) {
      visitToken(N);
    }

    void visitStringLiteral(StringLiteral* N) {
      visitToken(N);
    }

    void visitIntegerLiteral(IntegerLiteral* N) {
      visitToken(N);
    }

    void visitConstraintExpression(ConstraintExpression* N) {
      visitNode(N);
    }

    void visitTypeclassConstraintExpression(TypeclassConstraintExpression* N) {
      visitConstraintExpression(N);
    }

    void visitEqualityConstraintExpression(EqualityConstraintExpression* N) {
      visitConstraintExpression(N);
    }

    void visitTypeExpression(TypeExpression* N) {
      visitNode(N);
    }

    void visitQualifiedTypeExpression(QualifiedTypeExpression* N) {
      visitTypeExpression(N);
    }

    void visitReferenceTypeExpression(ReferenceTypeExpression* N) {
      visitTypeExpression(N);
    }

    void visitArrowTypeExpression(ArrowTypeExpression* N) {
      visitTypeExpression(N);
    }

    void visitVarTypeExpression(VarTypeExpression* N) {
      visitTypeExpression(N);
    }

    void visitPattern(Pattern* N) {
      visitNode(N);
    }

    void visitBindPattern(BindPattern* N) {
      visitPattern(N);
    }

    void visitLiteralPattern(LiteralPattern* N) {
      visitPattern(N);
    }

    void visitExpression(Expression* N) {
      visitNode(N);
    }

    void visitReferenceExpression(ReferenceExpression* N) {
      visitExpression(N);
    }

    void visitMatchCase(MatchCase* N) {
      visitNode(N);
    }

    void visitMatchExpression(MatchExpression* N) {
      visitExpression(N);
    }

    void visitNestedExpression(NestedExpression* N) {
      visitExpression(N);
    }

    void visitConstantExpression(ConstantExpression* N) {
      visitExpression(N);
    }

    void visitCallExpression(CallExpression* N) {
      visitExpression(N);
    }

    void visitInfixExpression(InfixExpression* N) {
      visitExpression(N);
    }

    void visitPrefixExpression(PrefixExpression* N) {
      visitExpression(N);
    }

    void visitStatement(Statement* N) {
      visitNode(N);
    }

    void visitExpressionStatement(ExpressionStatement* N) {
      visitStatement(N);
    }

    void visitReturnStatement(ReturnStatement* N) {
      visitStatement(N);
    }

    void visitIfStatement(IfStatement* N) {
      visitStatement(N);
    }

    void visitIfStatementPart(IfStatementPart* N) {
      visitNode(N);
    }

    void visitTypeAssert(TypeAssert* N) {
      visitNode(N);
    }

    void visitParameter(Parameter* N) {
      visitNode(N);
    }

    void visitLetBody(LetBody* N) {
      visitNode(N);
    }

    void visitLetBlockBody(LetBlockBody* N) {
      visitLetBody(N);
    }

    void visitLetExprBody(LetExprBody* N) {
      visitLetBody(N);
    }

    void visitLetDeclaration(LetDeclaration* N) {
      visitNode(N);
    }

    void visitStructDeclarationField(StructDeclarationField* N) {
      visitNode(N);
    }

    void visitStructDeclaration(StructDeclaration* N) {
      visitNode(N);
    }

    void visitClassDeclaration(ClassDeclaration* N) {
      visitNode(N);
    }

    void visitInstanceDeclaration(InstanceDeclaration* N) {
      visitNode(N);
    }

    void visitSourceFile(SourceFile* N) {
      visitNode(N);
    }

  public:

    void visitEachChild(Node* N) {
      switch (N->getKind()) {
        case NodeKind::Equals:
          visitEachChild(static_cast<Equals*>(N));
          break;
        case NodeKind::Colon:
          visitEachChild(static_cast<Colon*>(N));
          break;
        case NodeKind::Comma:
          visitEachChild(static_cast<Comma*>(N));
          break;
        case NodeKind::Dot:
          visitEachChild(static_cast<Dot*>(N));
          break;
        case NodeKind::DotDot:
          visitEachChild(static_cast<DotDot*>(N));
          break;
        case NodeKind::Tilde:
          visitEachChild(static_cast<Tilde*>(N));
          break;
        case NodeKind::LParen:
          visitEachChild(static_cast<LParen*>(N));
          break;
        case NodeKind::RParen:
          visitEachChild(static_cast<RParen*>(N));
          break;
        case NodeKind::LBracket:
          visitEachChild(static_cast<LBracket*>(N));
          break;
        case NodeKind::RBracket:
          visitEachChild(static_cast<RBracket*>(N));
          break;
        case NodeKind::LBrace:
          visitEachChild(static_cast<LBrace*>(N));
          break;
        case NodeKind::RBrace:
          visitEachChild(static_cast<RBrace*>(N));
          break;
        case NodeKind::RArrow:
          visitEachChild(static_cast<RArrow*>(N));
          break;
        case NodeKind::RArrowAlt:
          visitEachChild(static_cast<RArrowAlt*>(N));
          break;
        case NodeKind::LetKeyword:
          visitEachChild(static_cast<LetKeyword*>(N));
          break;
        case NodeKind::MutKeyword:
          visitEachChild(static_cast<MutKeyword*>(N));
          break;
        case NodeKind::PubKeyword:
          visitEachChild(static_cast<PubKeyword*>(N));
          break;
        case NodeKind::TypeKeyword:
          visitEachChild(static_cast<TypeKeyword*>(N));
          break;
        case NodeKind::ReturnKeyword:
          visitEachChild(static_cast<ReturnKeyword*>(N));
          break;
        case NodeKind::ModKeyword:
          visitEachChild(static_cast<ModKeyword*>(N));
          break;
        case NodeKind::StructKeyword:
          visitEachChild(static_cast<StructKeyword*>(N));
          break;
        case NodeKind::ClassKeyword:
          visitEachChild(static_cast<ClassKeyword*>(N));
          break;
        case NodeKind::InstanceKeyword:
          visitEachChild(static_cast<InstanceKeyword*>(N));
          break;
        case NodeKind::ElifKeyword:
          visitEachChild(static_cast<ElifKeyword*>(N));
          break;
        case NodeKind::IfKeyword:
          visitEachChild(static_cast<IfKeyword*>(N));
          break;
        case NodeKind::ElseKeyword:
          visitEachChild(static_cast<ElseKeyword*>(N));
          break;
        case NodeKind::MatchKeyword:
          visitEachChild(static_cast<MatchKeyword*>(N));
          break;
        case NodeKind::Invalid:
          visitEachChild(static_cast<Invalid*>(N));
          break;
        case NodeKind::EndOfFile:
          visitEachChild(static_cast<EndOfFile*>(N));
          break;
        case NodeKind::BlockStart:
          visitEachChild(static_cast<BlockStart*>(N));
          break;
        case NodeKind::BlockEnd:
          visitEachChild(static_cast<BlockEnd*>(N));
          break;
        case NodeKind::LineFoldEnd:
          visitEachChild(static_cast<LineFoldEnd*>(N));
          break;
        case NodeKind::CustomOperator:
          visitEachChild(static_cast<CustomOperator*>(N));
          break;
        case NodeKind::Assignment:
          visitEachChild(static_cast<Assignment*>(N));
          break;
        case NodeKind::Identifier:
          visitEachChild(static_cast<Identifier*>(N));
          break;
        case NodeKind::IdentifierAlt:
          visitEachChild(static_cast<IdentifierAlt*>(N));
          break;
        case NodeKind::StringLiteral:
          visitEachChild(static_cast<StringLiteral*>(N));
          break;
        case NodeKind::IntegerLiteral:
          visitEachChild(static_cast<IntegerLiteral*>(N));
          break;
        case NodeKind::TypeclassConstraintExpression:
          visitEachChild(static_cast<TypeclassConstraintExpression*>(N));
          break;
        case NodeKind::EqualityConstraintExpression:
          visitEachChild(static_cast<EqualityConstraintExpression*>(N));
          break;
        case NodeKind::QualifiedTypeExpression:
          visitEachChild(static_cast<QualifiedTypeExpression*>(N));
          break;
        case NodeKind::ReferenceTypeExpression:
          visitEachChild(static_cast<ReferenceTypeExpression*>(N));
          break;
        case NodeKind::ArrowTypeExpression:
          visitEachChild(static_cast<ArrowTypeExpression*>(N));
          break;
        case NodeKind::VarTypeExpression:
          visitEachChild(static_cast<VarTypeExpression*>(N));
          break;
        case NodeKind::BindPattern:
          visitEachChild(static_cast<BindPattern*>(N));
          break;
        case NodeKind::LiteralPattern:
          visitEachChild(static_cast<LiteralPattern*>(N));
          break;
        case NodeKind::ReferenceExpression:
          visitEachChild(static_cast<ReferenceExpression*>(N));
          break;
        case NodeKind::MatchCase:
          visitEachChild(static_cast<MatchCase*>(N));
          break;
        case NodeKind::MatchExpression:
          visitEachChild(static_cast<MatchExpression*>(N));
          break;
        case NodeKind::NestedExpression:
          visitEachChild(static_cast<NestedExpression*>(N));
          break;
        case NodeKind::ConstantExpression:
          visitEachChild(static_cast<ConstantExpression*>(N));
          break;
        case NodeKind::CallExpression:
          visitEachChild(static_cast<CallExpression*>(N));
          break;
        case NodeKind::InfixExpression:
          visitEachChild(static_cast<InfixExpression*>(N));
          break;
        case NodeKind::PrefixExpression:
          visitEachChild(static_cast<PrefixExpression*>(N));
          break;
        case NodeKind::ExpressionStatement:
          visitEachChild(static_cast<ExpressionStatement*>(N));
          break;
        case NodeKind::ReturnStatement:
          visitEachChild(static_cast<ReturnStatement*>(N));
          break;
        case NodeKind::IfStatement:
          visitEachChild(static_cast<IfStatement*>(N));
          break;
        case NodeKind::IfStatementPart:
          visitEachChild(static_cast<IfStatementPart*>(N));
          break;
        case NodeKind::TypeAssert:
          visitEachChild(static_cast<TypeAssert*>(N));
          break;
        case NodeKind::Parameter:
          visitEachChild(static_cast<Parameter*>(N));
          break;
        case NodeKind::LetBlockBody:
          visitEachChild(static_cast<LetBlockBody*>(N));
          break;
        case NodeKind::LetExprBody:
          visitEachChild(static_cast<LetExprBody*>(N));
          break;
        case NodeKind::LetDeclaration:
          visitEachChild(static_cast<LetDeclaration*>(N));
          break;
        case NodeKind::StructDeclaration:
          visitEachChild(static_cast<StructDeclaration*>(N));
          break;
        case NodeKind::StructDeclarationField:
          visitEachChild(static_cast<StructDeclarationField*>(N));
          break;
        case NodeKind::ClassDeclaration:
          visitEachChild(static_cast<ClassDeclaration*>(N));
          break;
        case NodeKind::InstanceDeclaration:
          visitEachChild(static_cast<InstanceDeclaration*>(N));
          break;
        case NodeKind::SourceFile:
          visitEachChild(static_cast<SourceFile*>(N));
          break;
        default:
          ZEN_UNREACHABLE
      }
    }

#define BOLT_VISIT(node) static_cast<D*>(this)->visit(node)

    void visitEachChild(Equals* N) {
    }

    void visitEachChild(Colon* N) {
    }

    void visitEachChild(Comma* N) {
    }

    void visitEachChild(Dot* N) {
    }

    void visitEachChild(DotDot* N) {
    }

    void visitEachChild(Tilde* N) {
    }

    void visitEachChild(LParen* N) {
    }

    void visitEachChild(RParen* N) {
    }

    void visitEachChild(LBracket* N) {
    }

    void visitEachChild(RBracket* N) {
    }

    void visitEachChild(LBrace* N) {
    }

    void visitEachChild(RBrace* N) {
    }

    void visitEachChild(RArrow* N) {
    }

    void visitEachChild(RArrowAlt* N) {
    }

    void visitEachChild(LetKeyword* N) {
    }

    void visitEachChild(MutKeyword* N) {
    }

    void visitEachChild(PubKeyword* N) {
    }

    void visitEachChild(TypeKeyword* N) {
    }

    void visitEachChild(ReturnKeyword* N) {
    }

    void visitEachChild(ModKeyword* N) {
    }

    void visitEachChild(StructKeyword* N) {
    }

    void visitEachChild(ClassKeyword* N) {
    }

    void visitEachChild(InstanceKeyword* N) {
    }

    void visitEachChild(ElifKeyword* N) {
    }

    void visitEachChild(IfKeyword* N) {
    }

    void visitEachChild(ElseKeyword* N) {
    }

    void visitEachChild(MatchKeyword* N) {
    }

    void visitEachChild(Invalid* N) {
    }

    void visitEachChild(EndOfFile* N) {
    }

    void visitEachChild(BlockStart* N) {
    }

    void visitEachChild(BlockEnd* N) {
    }

    void visitEachChild(LineFoldEnd* N) {
    }

    void visitEachChild(CustomOperator* N) {
    }

    void visitEachChild(Assignment* N) {
    }

    void visitEachChild(Identifier* N) {
    }

    void visitEachChild(IdentifierAlt* N) {
    }

    void visitEachChild(StringLiteral* N) {
    }

    void visitEachChild(IntegerLiteral* N) {
    }

    void visitEachChild(TypeclassConstraintExpression* N) {
      BOLT_VISIT(N->Name);
      for (auto TE: N->TEs) {
        BOLT_VISIT(TE);
      }
    }

    void visitEachChild(EqualityConstraintExpression* N) {
      BOLT_VISIT(N->Left);
      BOLT_VISIT(N->Tilde);
      BOLT_VISIT(N->Right);
    }

    void visitEachChild(QualifiedTypeExpression* N) {
      for (auto [CE, Comma]: N->Constraints) {
        BOLT_VISIT(CE);
        if (Comma) {
          BOLT_VISIT(Comma);
        }
      }
      BOLT_VISIT(N->RArrowAlt);
      BOLT_VISIT(N->TE);
    }

    void visitEachChild(ReferenceTypeExpression* N) {
      for (auto [Name, Dot]: N->ModulePath) {
        BOLT_VISIT(Name);
        BOLT_VISIT(Dot);
      }
      BOLT_VISIT(N->Name);
    }

    void visitEachChild(ArrowTypeExpression* N) {
      for (auto PT: N->ParamTypes) {
        BOLT_VISIT(PT);
      }
      BOLT_VISIT(N->ReturnType);
    }

    void visitEachChild(VarTypeExpression* N) {
      BOLT_VISIT(N->Name);
    }

    void visitEachChild(BindPattern* N) {
      BOLT_VISIT(N->Name);
    }

    void visitEachChild(LiteralPattern* N) {
      BOLT_VISIT(N->Literal);
    }

    void visitEachChild(ReferenceExpression* N) {
       for (auto [Name, Dot]: N->ModulePath) {
         BOLT_VISIT(Name);
         BOLT_VISIT(Dot);
       }
       BOLT_VISIT(N->Name);
    }

    void visitEachChild(MatchCase* N) {
      BOLT_VISIT(N->Pattern);
      BOLT_VISIT(N->RArrowAlt);
      BOLT_VISIT(N->Expression);
    }

    void visitEachChild(MatchExpression* N) {
      BOLT_VISIT(N->MatchKeyword);
      if (N->Value) {
        BOLT_VISIT(N->Value);
      }
      BOLT_VISIT(N->BlockStart);
      for (auto Case: N->Cases) {
        BOLT_VISIT(Case);
      }
    }

    void visitEachChild(NestedExpression* N) {
      BOLT_VISIT(N->LParen);
      BOLT_VISIT(N->Inner);
      BOLT_VISIT(N->RParen);
    }

    void visitEachChild(ConstantExpression* N) {
      BOLT_VISIT(N->Token);
    }

    void visitEachChild(CallExpression* N) {
      BOLT_VISIT(N->Function);
      for (auto Arg: N->Args) {
        BOLT_VISIT(Arg);
      }
    }

    void visitEachChild(InfixExpression* N) {
      BOLT_VISIT(N->LHS);
      BOLT_VISIT(N->Operator);
      BOLT_VISIT(N->RHS);
    }

    void visitEachChild(PrefixExpression* N) {
      BOLT_VISIT(N->Operator);
      BOLT_VISIT(N->Argument);
    }

    void visitEachChild(ExpressionStatement* N) {
      BOLT_VISIT(N->Expression);
    }

    void visitEachChild(ReturnStatement* N) {
      BOLT_VISIT(N->ReturnKeyword);
      BOLT_VISIT(N->Expression);
    }

    void visitEachChild(IfStatement* N) {
      for (auto Part: N->Parts) {
        BOLT_VISIT(Part);
      }
    }

    void visitEachChild(IfStatementPart* N) {
      BOLT_VISIT(N->Keyword);
      if (N->Test != nullptr) {
        BOLT_VISIT(N->Test);
      }
      BOLT_VISIT(N->BlockStart);
      for (auto Element: N->Elements) {
        BOLT_VISIT(Element);
      }
    }

    void visitEachChild(TypeAssert* N) {
      BOLT_VISIT(N->Colon);
      BOLT_VISIT(N->TypeExpression);
    }

    void visitEachChild(Parameter* N) {
      BOLT_VISIT(N->Pattern);
      if (N->TypeAssert != nullptr) {
        BOLT_VISIT(N->TypeAssert);
      }
    }

    void visitEachChild(LetBlockBody* N) {
      BOLT_VISIT(N->BlockStart);
      for (auto Element: N->Elements) {
        BOLT_VISIT(Element);
      }
    }

    void visitEachChild(LetExprBody* N) {
      BOLT_VISIT(N->Equals);
      BOLT_VISIT(N->Expression);
    }

    void visitEachChild(LetDeclaration* N) {
      if (N->PubKeyword) {
        BOLT_VISIT(N->PubKeyword);
      }
      BOLT_VISIT(N->LetKeyword);
      if (N->MutKeyword) {
        BOLT_VISIT(N->MutKeyword);
      }
      BOLT_VISIT(N->Pattern);
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

    void visitEachChild(StructDeclarationField* N) {
      BOLT_VISIT(N->Name);
      BOLT_VISIT(N->Colon);
      BOLT_VISIT(N->TypeExpression);
    }

    void visitEachChild(StructDeclaration* N) {
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

    void visitEachChild(ClassDeclaration* N) {
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

    void visitEachChild(InstanceDeclaration* N) {
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

    void visitEachChild(SourceFile* N) {
      for (auto Element: N->Elements) {
        BOLT_VISIT(Element);
      }
    }

  };

}
