
#pragma once

#include "zen/config.hpp"

#include "bolt/CST.hpp"

namespace bolt {

  template<typename D, typename R = void>
  class CSTVisitor {
  public:

    void visit(Node* N) {

#define BOLT_GEN_CASE(name) \
        case NodeKind::name: \
          return static_cast<D*>(this)->visit ## name(static_cast<name*>(N));

      switch (N->getKind()) {
        BOLT_GEN_CASE(Equals)
        BOLT_GEN_CASE(Colon)
        BOLT_GEN_CASE(Comma)
        BOLT_GEN_CASE(Dot)
        BOLT_GEN_CASE(DotDot)
        BOLT_GEN_CASE(Tilde)
        BOLT_GEN_CASE(LParen)
        BOLT_GEN_CASE(RParen)
        BOLT_GEN_CASE(LBracket)
        BOLT_GEN_CASE(RBracket)
        BOLT_GEN_CASE(LBrace)
        BOLT_GEN_CASE(RBrace)
        BOLT_GEN_CASE(RArrow)
        BOLT_GEN_CASE(RArrowAlt)
        BOLT_GEN_CASE(LetKeyword)
        BOLT_GEN_CASE(FnKeyword)
        BOLT_GEN_CASE(MutKeyword)
        BOLT_GEN_CASE(PubKeyword)
        BOLT_GEN_CASE(TypeKeyword)
        BOLT_GEN_CASE(ReturnKeyword)
        BOLT_GEN_CASE(ModKeyword)
        BOLT_GEN_CASE(StructKeyword)
        BOLT_GEN_CASE(EnumKeyword)
        BOLT_GEN_CASE(ClassKeyword)
        BOLT_GEN_CASE(InstanceKeyword)
        BOLT_GEN_CASE(ElifKeyword)
        BOLT_GEN_CASE(IfKeyword)
        BOLT_GEN_CASE(ElseKeyword)
        BOLT_GEN_CASE(MatchKeyword)
        BOLT_GEN_CASE(Invalid)
        BOLT_GEN_CASE(EndOfFile)
        BOLT_GEN_CASE(BlockStart)
        BOLT_GEN_CASE(BlockEnd)
        BOLT_GEN_CASE(LineFoldEnd)
        BOLT_GEN_CASE(CustomOperator)
        BOLT_GEN_CASE(Assignment)
        BOLT_GEN_CASE(Identifier)
        BOLT_GEN_CASE(IdentifierAlt)
        BOLT_GEN_CASE(StringLiteral)
        BOLT_GEN_CASE(IntegerLiteral)
        BOLT_GEN_CASE(TypeclassConstraintExpression)
        BOLT_GEN_CASE(EqualityConstraintExpression)
        BOLT_GEN_CASE(QualifiedTypeExpression)
        BOLT_GEN_CASE(ReferenceTypeExpression)
        BOLT_GEN_CASE(ArrowTypeExpression)
        BOLT_GEN_CASE(AppTypeExpression)
        BOLT_GEN_CASE(VarTypeExpression)
        BOLT_GEN_CASE(NestedTypeExpression)
        BOLT_GEN_CASE(TupleTypeExpression)
        BOLT_GEN_CASE(BindPattern)
        BOLT_GEN_CASE(LiteralPattern)
        BOLT_GEN_CASE(NamedPattern)
        BOLT_GEN_CASE(TuplePattern)
        BOLT_GEN_CASE(NestedPattern)
        BOLT_GEN_CASE(ListPattern)
        BOLT_GEN_CASE(ReferenceExpression)
        BOLT_GEN_CASE(MatchCase)
        BOLT_GEN_CASE(MatchExpression)
        BOLT_GEN_CASE(MemberExpression)
        BOLT_GEN_CASE(TupleExpression)
        BOLT_GEN_CASE(NestedExpression)
        BOLT_GEN_CASE(ConstantExpression)
        BOLT_GEN_CASE(CallExpression)
        BOLT_GEN_CASE(InfixExpression)
        BOLT_GEN_CASE(PrefixExpression)
        BOLT_GEN_CASE(RecordExpressionField)
        BOLT_GEN_CASE(RecordExpression)
        BOLT_GEN_CASE(ExpressionStatement)
        BOLT_GEN_CASE(ReturnStatement)
        BOLT_GEN_CASE(IfStatement)
        BOLT_GEN_CASE(IfStatementPart)
        BOLT_GEN_CASE(TypeAssert)
        BOLT_GEN_CASE(Parameter)
        BOLT_GEN_CASE(LetBlockBody)
        BOLT_GEN_CASE(LetExprBody)
        BOLT_GEN_CASE(FunctionDeclaration)
        BOLT_GEN_CASE(VariableDeclaration)
        BOLT_GEN_CASE(RecordDeclaration)
        BOLT_GEN_CASE(RecordDeclarationField)
        BOLT_GEN_CASE(VariantDeclaration)
        BOLT_GEN_CASE(TupleVariantDeclarationMember)
        BOLT_GEN_CASE(RecordVariantDeclarationMember)
        BOLT_GEN_CASE(ClassDeclaration)
        BOLT_GEN_CASE(InstanceDeclaration)
        BOLT_GEN_CASE(SourceFile)
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

    void visitFnKeyword(FnKeyword* N) {
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

    void visitEnumKeyword(EnumKeyword* N) {
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

    void visitAppTypeExpression(AppTypeExpression* N) {
      visitTypeExpression(N);
    }

    void visitVarTypeExpression(VarTypeExpression* N) {
      visitTypeExpression(N);
    }

    void visitNestedTypeExpression(NestedTypeExpression* N) {
      visitTypeExpression(N);
    }

    void visitTupleTypeExpression(TupleTypeExpression* N) {
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

    void visitNamedPattern(NamedPattern* N) {
      visitPattern(N);
    }

    void visitTuplePattern(TuplePattern* N) {
      visitPattern(N);
    }

    void visitNestedPattern(NestedPattern* N) {
      visitPattern(N);
    }

    void visitListPattern(ListPattern* N) {
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

    void visitMemberExpression(MemberExpression* N) {
      visitExpression(N);
    }

    void visitTupleExpression(TupleExpression* N) {
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

    void visitRecordExpressionField(RecordExpressionField* N) {
      visitNode(N);
    }

    void visitRecordExpression(RecordExpression* N) {
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

    void visitFunctionDeclaration(FunctionDeclaration* N) {
      visitNode(N);
    }

    void visitVariableDeclaration(VariableDeclaration* N) {
      visitNode(N);
    }

    void visitRecordDeclarationField(RecordDeclarationField* N) {
      visitNode(N);
    }

    void visitRecordDeclaration(RecordDeclaration* N) {
      visitNode(N);
    }

    void visitVariantDeclaration(VariantDeclaration* N) {
      visitNode(N);
    }

    void visitVariantDeclarationMember(VariantDeclarationMember* N) {
      visitNode(N);
    }

    void visitTupleVariantDeclarationMember(TupleVariantDeclarationMember* N) {
      visitVariantDeclarationMember(N);
    }

    void visitRecordVariantDeclarationMember(RecordVariantDeclarationMember* N) {
      visitVariantDeclarationMember(N);
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

#define BOLT_GEN_CHILD_CASE(name) \
      case NodeKind::name: \
        visitEachChild(static_cast<name*>(N)); \
        break;

      switch (N->getKind()) {
        BOLT_GEN_CHILD_CASE(Equals)
        BOLT_GEN_CHILD_CASE(Colon)
        BOLT_GEN_CHILD_CASE(Comma)
        BOLT_GEN_CHILD_CASE(Dot)
        BOLT_GEN_CHILD_CASE(DotDot)
        BOLT_GEN_CHILD_CASE(Tilde)
        BOLT_GEN_CHILD_CASE(LParen)
        BOLT_GEN_CHILD_CASE(RParen)
        BOLT_GEN_CHILD_CASE(LBracket)
        BOLT_GEN_CHILD_CASE(RBracket)
        BOLT_GEN_CHILD_CASE(LBrace)
        BOLT_GEN_CHILD_CASE(RBrace)
        BOLT_GEN_CHILD_CASE(RArrow)
        BOLT_GEN_CHILD_CASE(RArrowAlt)
        BOLT_GEN_CHILD_CASE(LetKeyword)
        BOLT_GEN_CHILD_CASE(FnKeyword)
        BOLT_GEN_CHILD_CASE(MutKeyword)
        BOLT_GEN_CHILD_CASE(PubKeyword)
        BOLT_GEN_CHILD_CASE(TypeKeyword)
        BOLT_GEN_CHILD_CASE(ReturnKeyword)
        BOLT_GEN_CHILD_CASE(ModKeyword)
        BOLT_GEN_CHILD_CASE(StructKeyword)
        BOLT_GEN_CHILD_CASE(EnumKeyword)
        BOLT_GEN_CHILD_CASE(ClassKeyword)
        BOLT_GEN_CHILD_CASE(InstanceKeyword)
        BOLT_GEN_CHILD_CASE(ElifKeyword)
        BOLT_GEN_CHILD_CASE(IfKeyword)
        BOLT_GEN_CHILD_CASE(ElseKeyword)
        BOLT_GEN_CHILD_CASE(MatchKeyword)
        BOLT_GEN_CHILD_CASE(Invalid)
        BOLT_GEN_CHILD_CASE(EndOfFile)
        BOLT_GEN_CHILD_CASE(BlockStart)
        BOLT_GEN_CHILD_CASE(BlockEnd)
        BOLT_GEN_CHILD_CASE(LineFoldEnd)
        BOLT_GEN_CHILD_CASE(CustomOperator)
        BOLT_GEN_CHILD_CASE(Assignment)
        BOLT_GEN_CHILD_CASE(Identifier)
        BOLT_GEN_CHILD_CASE(IdentifierAlt)
        BOLT_GEN_CHILD_CASE(StringLiteral)
        BOLT_GEN_CHILD_CASE(IntegerLiteral)
        BOLT_GEN_CHILD_CASE(TypeclassConstraintExpression)
        BOLT_GEN_CHILD_CASE(EqualityConstraintExpression)
        BOLT_GEN_CHILD_CASE(QualifiedTypeExpression)
        BOLT_GEN_CHILD_CASE(ReferenceTypeExpression)
        BOLT_GEN_CHILD_CASE(ArrowTypeExpression)
        BOLT_GEN_CHILD_CASE(AppTypeExpression)
        BOLT_GEN_CHILD_CASE(VarTypeExpression)
        BOLT_GEN_CHILD_CASE(NestedTypeExpression)
        BOLT_GEN_CHILD_CASE(TupleTypeExpression)
        BOLT_GEN_CHILD_CASE(BindPattern)
        BOLT_GEN_CHILD_CASE(LiteralPattern)
        BOLT_GEN_CHILD_CASE(NamedPattern)
        BOLT_GEN_CHILD_CASE(TuplePattern)
        BOLT_GEN_CHILD_CASE(NestedPattern)
        BOLT_GEN_CHILD_CASE(ListPattern)
        BOLT_GEN_CHILD_CASE(ReferenceExpression)
        BOLT_GEN_CHILD_CASE(MatchCase)
        BOLT_GEN_CHILD_CASE(MatchExpression)
        BOLT_GEN_CHILD_CASE(MemberExpression)
        BOLT_GEN_CHILD_CASE(TupleExpression)
        BOLT_GEN_CHILD_CASE(NestedExpression)
        BOLT_GEN_CHILD_CASE(ConstantExpression)
        BOLT_GEN_CHILD_CASE(CallExpression)
        BOLT_GEN_CHILD_CASE(InfixExpression)
        BOLT_GEN_CHILD_CASE(PrefixExpression)
        BOLT_GEN_CHILD_CASE(RecordExpressionField)
        BOLT_GEN_CHILD_CASE(RecordExpression)
        BOLT_GEN_CHILD_CASE(ExpressionStatement)
        BOLT_GEN_CHILD_CASE(ReturnStatement)
        BOLT_GEN_CHILD_CASE(IfStatement)
        BOLT_GEN_CHILD_CASE(IfStatementPart)
        BOLT_GEN_CHILD_CASE(TypeAssert)
        BOLT_GEN_CHILD_CASE(Parameter)
        BOLT_GEN_CHILD_CASE(LetBlockBody)
        BOLT_GEN_CHILD_CASE(LetExprBody)
        BOLT_GEN_CHILD_CASE(FunctionDeclaration)
        BOLT_GEN_CHILD_CASE(VariableDeclaration)
        BOLT_GEN_CHILD_CASE(RecordDeclaration)
        BOLT_GEN_CHILD_CASE(RecordDeclarationField)
        BOLT_GEN_CHILD_CASE(VariantDeclaration)
        BOLT_GEN_CHILD_CASE(TupleVariantDeclarationMember)
        BOLT_GEN_CHILD_CASE(RecordVariantDeclarationMember)
        BOLT_GEN_CHILD_CASE(ClassDeclaration)
        BOLT_GEN_CHILD_CASE(InstanceDeclaration)
        BOLT_GEN_CHILD_CASE(SourceFile)
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

    void visitEachChild(FnKeyword* N) {
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

    void visitEachChild(EnumKeyword* N) {
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

    void visitEachChild(AppTypeExpression* N) {
      BOLT_VISIT(N->Op);
      for (auto Arg: N->Args) {
        BOLT_VISIT(Arg);
      }
    }

    void visitEachChild(VarTypeExpression* N) {
      BOLT_VISIT(N->Name);
    }

    void visitEachChild(NestedTypeExpression* N) {
      BOLT_VISIT(N->LParen);
      BOLT_VISIT(N->TE);
      BOLT_VISIT(N->RParen);
    }

    void visitEachChild(TupleTypeExpression* N) {
      BOLT_VISIT(N->LParen);
      for (auto [TE, Comma]: N->Elements) {
        if (Comma) {
          BOLT_VISIT(Comma);
        }
        BOLT_VISIT(TE);
      }
      BOLT_VISIT(N->RParen);
    }

    void visitEachChild(BindPattern* N) {
      BOLT_VISIT(N->Name);
    }

    void visitEachChild(LiteralPattern* N) {
      BOLT_VISIT(N->Literal);
    }

    void visitEachChild(NamedPattern* N) {
      BOLT_VISIT(N->Name);
      for (auto P: N->Patterns) {
        BOLT_VISIT(P);
      }
    }

    void visitEachChild(TuplePattern* N) {
      BOLT_VISIT(N->LParen);
      for (auto [P, Comma]: N->Elements) {
        BOLT_VISIT(P);
        if (Comma) {
          BOLT_VISIT(Comma);
        }
      }
      BOLT_VISIT(N->RParen);
    }

    void visitEachChild(NestedPattern* N) {
      BOLT_VISIT(N->LParen);
      BOLT_VISIT(N->P);
      BOLT_VISIT(N->RParen);
    }

    void visitEachChild(ListPattern* N) {
      BOLT_VISIT(N->LBracket);
      for (auto [Element, Separator]: N->Elements) {
        BOLT_VISIT(Element);
        if (Separator) {
          BOLT_VISIT(Separator);
        }
      }
      BOLT_VISIT(N->RBracket);
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

    void visitEachChild(MemberExpression* N) {
      BOLT_VISIT(N->getExpression());
      BOLT_VISIT(N->Dot);
      BOLT_VISIT(N->Name);
    }

    void visitEachChild(TupleExpression* N) {
      BOLT_VISIT(N->LParen);
      for (auto [E, Comma]: N->Elements) {
        BOLT_VISIT(E);
        if (Comma) {
          BOLT_VISIT(Comma);
        }
      }
      BOLT_VISIT(N->RParen);
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

    void visitEachChild(RecordExpressionField* N) {
      BOLT_VISIT(N->Name);
      BOLT_VISIT(N->Equals);
      BOLT_VISIT(N->E);
    }

    void visitEachChild(RecordExpression* N) {
      BOLT_VISIT(N->LBrace);
      for (auto [Field, Comma]: N->Fields) {
        BOLT_VISIT(Field);
        if (Comma) {
          BOLT_VISIT(Comma);
        }
      }
      BOLT_VISIT(N->RBrace);
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

    void visitEachChild(FunctionDeclaration* N) {
      if (N->PubKeyword) {
        BOLT_VISIT(N->PubKeyword);
      }
      BOLT_VISIT(N->FnKeyword);
      BOLT_VISIT(N->Name);
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

    void visitEachChild(VariableDeclaration* N) {
      if (N->PubKeyword) {
        BOLT_VISIT(N->PubKeyword);
      }
      BOLT_VISIT(N->LetKeyword);
      BOLT_VISIT(N->Pattern);
      if (N->TypeAssert) {
        BOLT_VISIT(N->TypeAssert);
      }
      if (N->Body) {
        BOLT_VISIT(N->Body);
      }
    }

    void visitEachChild(RecordDeclarationField* N) {
      BOLT_VISIT(N->Name);
      BOLT_VISIT(N->Colon);
      BOLT_VISIT(N->TypeExpression);
    }

    void visitEachChild(RecordDeclaration* N) {
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

    void visitEachChild(VariantDeclaration* N) {
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

    void visitEachChild(TupleVariantDeclarationMember* N) {
      BOLT_VISIT(N->Name);
      for (auto Element: N->Elements) {
        BOLT_VISIT(Element);
      }
    }

    void visitEachChild(RecordVariantDeclarationMember* N) {
      BOLT_VISIT(N->Name);
      BOLT_VISIT(N->BlockStart);
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
