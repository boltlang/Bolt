
#include "bolt/CSTVisitor.hpp"
#include "zen/graph.hpp"

#include "bolt/ByteString.hpp"
#include "bolt/CST.hpp"
#include "bolt/Type.hpp"
#include "bolt/Diagnostics.hpp"
#include <cwchar>
#include <functional>
#include "bolt/Checker.hpp"

namespace bolt {

static inline void mergeTo(ConstraintSet& Out, const ConstraintSet& Other) {
  for (auto C: Other) {
    Out.push_back(C);
  }
}

TypeScheme* TypeEnv::lookup(ByteString Name, SymbolKind Kind) {
  auto Curr = this;
  do {
    auto Match = Curr->Mapping.find(std::make_tuple(Name, Kind));
    if (Match != Curr->Mapping.end()) {
      return Match->second;
    }
    Curr = Curr->Parent;
  } while (Curr);
  return nullptr;
}

void TypeEnv::add(ByteString Name, TypeScheme* Scm, SymbolKind Kind) {
  Mapping.emplace(std::make_tuple(Name, Kind), Scm);
}

void TypeEnv::add(ByteString Name, Type* Ty, SymbolKind Kind) {
  add(Name, new TypeScheme { {}, Ty }, Kind);
}

using TVSub = std::unordered_map<TVar*, Type*>;

Type* substituteType(Type* Ty, const TVSub& Sub) {
  switch (Ty->getKind()) {
    case TypeKind::App:
      {
        auto A = static_cast<TApp*>(Ty);
        auto NewLeft = substituteType(A->getLeft(), Sub);
        auto NewRight = substituteType(A->getRight(), Sub);
        if (A->getLeft() == NewLeft && A->getRight() == NewRight) {
          return Ty;
        }
        return new TApp(NewLeft, NewRight);
      }
    case TypeKind::Con:
      return Ty;
    case TypeKind::Var:
      {
        auto NewTy = Ty->find();
        if (NewTy->getKind() != TypeKind::Var) {
          return substituteType(NewTy, Sub);
        }
        auto Match = Sub.find(static_cast<TVar*>(NewTy));
        return Match == Sub.end() 
            ? NewTy
            : Match->second;
      }
    case TypeKind::Fun:
      {
        auto F = static_cast<TFun*>(Ty);
        auto NewLeft = substituteType(F->getLeft(), Sub);
        auto NewRight = substituteType(F->getRight(), Sub);
        if (F->getLeft() == NewLeft && F->getRight() == NewRight) {
          return Ty;
        }
        return new TFun(NewLeft, NewRight);
      }
  }
}

Checker::Checker(DiagnosticEngine& DE):
  DE(DE) {
    IntType = new TCon("Int");
    BoolType = new TCon("Bool");
    StringType = new TCon("String");
    UnitType = new TCon("()");
  }

Type* Checker::instantiate(TypeScheme* Scm) {
  TVSub Sub;
  for (auto TV: Scm->Unbound) {
    auto Fresh = createTVar();
    Sub[TV] = Fresh;
  }
  return substituteType(Scm->getType(), Sub);
}

std::tuple<ConstraintSet, Type*> Checker::inferExpr(TypeEnv& Env, Expression* Expr, Type* RetTy) {

  ConstraintSet Out;
  Type* Ty;

  for (auto Ann: Expr->Annotations) {
    if (Ann->getKind() == NodeKind::TypeAssertAnnotation) {
      auto [AnnOut, AnnTy] = inferTypeExpr(Env, static_cast<TypeAssertAnnotation*>(Ann)->getTypeExpression());
      mergeTo(Out, AnnOut);
    }
  }

  switch (Expr->getKind()) {

    case NodeKind::BlockExpression:
      {
        auto E = static_cast<BlockExpression*>(Expr);
        auto N = E->Elements.size();
        for (std::size_t I = 0; I+1 < N; ++I) {
          auto Element = E->Elements[I];
          
          auto CC = inferElement(Env, Element, RetTy);
          mergeTo(Out, CC);
        }
        auto Last = E->Elements[N-1];
        auto [CC, ResTy] = inferExpr(Env, cast<Expression>(Last), RetTy);
        mergeTo(Out, CC);
        Ty = ResTy;
        break;
      }

    case NodeKind::ReferenceExpression:
      {
        auto E = static_cast<ReferenceExpression*>(Expr);
        auto Name = E->Name.getCanonicalText();
        auto Match = Env.lookup(Name, SymbolKind::Var);
        if (Match == nullptr) {
          DE.add<BindingNotFoundDiagnostic>(Name, E->Name);
          Ty = createTVar();
        } else {
          Ty = instantiate(Match);
        }
        break;
      }

    case NodeKind::LiteralExpression:
      {
        auto E = static_cast<LiteralExpression*>(Expr);
        switch (E->Token ->getKind()) {
          case NodeKind::IntegerLiteral:
            Ty = getIntType();
            break;
          case NodeKind::StringLiteral:
            Ty = getStringType();
            break;
          default:
            ZEN_UNREACHABLE
        }
        break;
      }

    case NodeKind::CallExpression:
      {
        auto E = static_cast<CallExpression*>(Expr);
        auto RetTy = createTVar();
        Type* FunTy = RetTy;
        for (auto It = E->Args.end(); It-- != E->Args.begin();) {
          auto [ArgOut, ArgTy] = inferExpr(Env, *It, RetTy);
          mergeTo(Out, ArgOut);
          FunTy = new TFun(ArgTy, FunTy);
        }
        auto FunOut = checkExpr(Env, E->Function, FunTy, RetTy);
        mergeTo(Out, FunOut);
        Ty = RetTy;
        break;
      }

    case NodeKind::InfixExpression:
      {
        auto E = static_cast<InfixExpression*>(Expr);
        auto [LeftOut, LeftTy] = inferExpr(Env, E->Left, RetTy);
        mergeTo(Out, LeftOut);
        auto [RightOut, RightTy] = inferExpr(Env, E->Right, RetTy);
        mergeTo(Out, RightOut);
        auto Name = E->Operator.getCanonicalText();
        auto Match = Env.lookup(Name, SymbolKind::Var);
        if (Match == nullptr) {
          DE.add<BindingNotFoundDiagnostic>(Name, E->Operator);
          return { Out, createTVar() };
        }
        auto RetTy = createTVar();
        auto FunTy = new TFun(LeftTy, new TFun(RightTy, RetTy));
        Out.push_back(new CTypesEqual(FunTy, instantiate(Match), E));
        Ty = RetTy;
        break;
      }

    case NodeKind::ReturnExpression:
      {
        auto E = static_cast<ReturnExpression*>(Expr);
        if (E->hasExpression()) {
          auto [ValOut, ValTy] = inferExpr(Env, E->getExpression(), RetTy);
          mergeTo(Out, ValOut);
          // Since evaluation stops at the return expression, it can be matched with any type.
          Out.push_back(new CTypesEqual { ValTy, RetTy, E });
        } else {
          Out.push_back(new CTypesEqual { getUnitType(), RetTy, E });
        }
        Ty = createTVar();
        break;
      }

    // TODO LambdaExpression

    default:
      ZEN_UNREACHABLE

  }

  Expr->setType(Ty);

  return { Out, Ty };
}

void Checker::visitPattern(Pattern* P, Type* Ty, TypeEnv& Out) {
  switch (P->getKind()) {
    case NodeKind::BindPattern:
      {
        auto Q = static_cast<BindPattern*>(P);
        // TODO Make a TypedNode out of a Pattern?
        Out.add(Q->Name->getCanonicalText(), Ty, SymbolKind::Var);
        break;
      }
    default:
      ZEN_UNREACHABLE
  }
}

std::tuple<ConstraintSet, Type*> Checker::inferTypeExpr(TypeEnv& Env, TypeExpression* TE) {

  ConstraintSet Out;
  Type* Ty;

  switch (TE->getKind()) {

    case NodeKind::ReferenceTypeExpression:
      {
        auto E = static_cast<ReferenceTypeExpression*>(TE);
        auto Name = E->Name->getCanonicalText();
        auto Match = Env.lookup(Name, SymbolKind::Type);
        if (Match == nullptr) {
          DE.add<BindingNotFoundDiagnostic>(Name, E->Name);
          Ty = createTVar();
        } else {
          Ty = instantiate(Match);
        }
        break;
      }

    default:
      ZEN_UNREACHABLE

  }

  TE->setType(Ty);

  return { Out, Ty };
}

ConstraintSet Checker::inferFunctionDeclaration(TypeEnv& Env, FunctionDeclaration* D) {

  auto TA = D->getTypeAssert();
  auto Params = D->getParams();
  auto Body = D->getBody();

  ConstraintSet Out;

  TypeEnv NewEnv { Env };

  auto RetTy = createTVar();

  Type* Ty = RetTy;
  for (auto It = Params.end(); It-- != Params.begin(); ) {
    auto Param = *It;
    auto ParamTy = createTVar();
    visitPattern(Param->Pattern, ParamTy, NewEnv);
    Ty = new TFun(ParamTy, Ty);
  }

  if (TA != nullptr) {
    auto [TEOut, TETy] = inferTypeExpr(Env, TA->TypeExpression);
    mergeTo(Out, TEOut);
    Out.push_back(new CTypesEqual(Ty, TETy, TA->TypeExpression));
  }

  if (Body != nullptr) {
    // TODO elminate BlockBody and replace with BlockExpr
    ZEN_ASSERT(Body->getKind() == NodeKind::LetExprBody);
    auto [BodyOut, BodyTy] = inferExpr(NewEnv, cast<LetExprBody>(Body)->Expression, RetTy);
    mergeTo(Out, BodyOut);
    Out.push_back(new CTypesEqual(RetTy, BodyTy, Body));
  }

  // Env.add(D->getNameAsString(), Ty, SymbolKind::Var);

  D->setType(Ty);

  return Out;
}

ConstraintSet Checker::inferVariableDeclaration(TypeEnv& Env, VariableDeclaration* Decl, Type* RetTy) {

  ConstraintSet Out;

  Type* Ty = nullptr;

  if (Decl->TypeAssert != nullptr) {
    auto [AssertOut, AssertTy] = inferTypeExpr(Env, Decl->TypeAssert->TypeExpression);
    mergeTo(Out, AssertOut);
    Ty = AssertTy;
  }

  if (Decl->Body != nullptr) {
    // TODO elminate BlockBody and replace with BlockExpr
    ZEN_ASSERT(Decl->Body->getKind() == NodeKind::LetExprBody);
    auto [BodyOut, BodyTy] = inferExpr(Env, cast<LetExprBody>(Decl->Body)->Expression, RetTy);
    mergeTo(Out, BodyOut);
    if (Ty == nullptr) {
      Ty = BodyTy;
    } else {
      Out.push_back(new CTypesEqual(Ty, BodyTy, Decl->Body));
    }
  }

  // Currently we don't perform generalisation on variable declarations
  Env.add(Decl->getNameAsString(), Ty, SymbolKind::Var);

  return Out;
}

bool hasTypeVar(Type* Ty, TVar* TV) {
  switch (TV->getKind()) {
    case TypeKind::App:
      {
        auto T = static_cast<TApp*>(Ty);
        return hasTypeVar(T->getLeft(), TV)
            || hasTypeVar(T->getRight(), TV);
      }
    case TypeKind::Con:
        return false;
    case TypeKind::Fun:
      {
        auto T = static_cast<TFun*>(Ty);
        return hasTypeVar(T->getLeft(), TV)
            || hasTypeVar(T->getRight(), TV);
      }
    case TypeKind::Var:
      {
        auto T = static_cast<TVar*>(Ty);
        return T->find() == TV;
      }
  }
}

bool TypeEnv::hasVar(TVar* TV) const {
  for (auto [_, Scm]: Mapping) {
    if (Scm->Unbound.count(TV)) {
      // FIXME
      ZEN_UNREACHABLE
    }
    if (hasTypeVar(Scm->getType(), TV)) {
      return true;
    }
  }
  return false;
}

auto getUnbound(const TypeEnv& Env, Type* Ty) {
  struct Visitor : public TypeVisitor {
    const TypeEnv& Env;
    Visitor(const TypeEnv& Env):
      Env(Env) {}
    std::vector<TVar*> Out;
    void visitVar(TVar* TV) {
      if (!Env.hasVar(TV)) {
        Out.push_back(TV);
      }
    }
  } V { Env };
  V.visit(Ty);
  return V.Out;
}

ConstraintSet Checker::inferMany(TypeEnv& Env, std::vector<Node*>& Elements, Type* RetTy) {

  using Graph = zen::hash_graph<Node*>;

  TypeEnv NewEnv { Env };

  Graph G;

  std::function<void(Node*, Node*)> populate = [&](auto From, auto N) {
    struct Visitor : CSTVisitor<Visitor> {
      Graph& G;
      Node* From;
      void visitReferenceExpression(ReferenceExpression* E) {
        auto To = E->getScope()->lookup(E->getSymbolPath());
        if (To != nullptr) {
          if (isa<Parameter>(To)) {
            To = To->Parent;
          }
          G.add_edge(From, To);
        }
      }
    } V { {}, G, From };
    V.visit(N);
  };

  std::vector<Node*> Stmts;

  for (auto Element: Elements) {
    if (isa<FunctionDeclaration>(Element)) {
      auto M = static_cast<FunctionDeclaration*>(Element);
      G.add_vertex(Element);
      if (M->hasBody()) {
        populate(M, M->getBody());
      }
    } else if (isa<VariableDeclaration>(Element)) {
      auto M = static_cast<VariableDeclaration*>(Element);
      G.add_vertex(Element);
      if (M->hasExpression()) {
        populate(M, M->getExpression());
      }
    } else {
      Stmts.push_back(Element);
    }
  }

  for (auto Nodes: zen::toposort(G)) {
    ConstraintSet Out;
    for (auto N: Nodes) {
      if (isa<FunctionDeclaration>(N)) {
        mergeTo(Out, inferFunctionDeclaration(Env, static_cast<FunctionDeclaration*>(N)));
      } else if (isa<VariableDeclaration>(N)) {
        mergeTo(Out, inferVariableDeclaration(Env, static_cast<VariableDeclaration*>(N), RetTy));
      } else {
        ZEN_UNREACHABLE
      }
    }
    solve(Out);
    for (auto N: Nodes) {
      if (isa<FunctionDeclaration>(N)) {
        auto M = static_cast<FunctionDeclaration*>(N);
        auto Unbound = getUnbound(Env, cast<Declaration>(N)->getType());
        Env.add(
          M->getNameAsString(),
          new TypeScheme { { Unbound.begin(), Unbound.end() }, M->getType() },
          SymbolKind::Var
        );
      }
    }
  }

  ConstraintSet Out;

  for (auto Stmt: Stmts) {
    mergeTo(Out, inferElement(Env, Stmt, RetTy));
  }

  return Out;
}

ConstraintSet Checker::inferElement(TypeEnv& Env, Node* N, Type* RetTy) {

  if (isa<Expression>(N)) {
    auto [Out, Ty] = inferExpr(Env, cast<Expression>(N), RetTy);
    return Out;
  }

  switch (N->getKind()) {

    case NodeKind::PrefixFunctionDeclaration:
    case NodeKind::InfixFunctionDeclaration:
    case NodeKind::SuffixFunctionDeclaration:
    case NodeKind::NamedFunctionDeclaration:
      return inferFunctionDeclaration(Env, static_cast<FunctionDeclaration*>(N));

    case NodeKind::ReturnExpression:
      {
        auto M = static_cast<ReturnExpression*>(N);
        if (!M->hasExpression()) {
          return {};
        }
        auto [ValOut, ValTy] = inferExpr(Env, M->getExpression(), RetTy);
        return { new CTypesEqual(ValTy, RetTy, N) };
      }

    default:
      ZEN_UNREACHABLE

  }

}

ConstraintSet Checker::inferSourceFile(TypeEnv& Env, SourceFile* SF) {
  return inferMany(Env, SF->Elements, nullptr);
}

ConstraintSet Checker::checkExpr(TypeEnv& Env, Expression* Expr, Type* Expected, Type* RetTy) {

  switch (Expr->getKind()) {

    case NodeKind::LiteralExpression:
      {
        auto E = static_cast<LiteralExpression*>(Expr);
        switch (E->Token->getKind()) {
          case NodeKind::IntegerLiteral:
            if (*Expected == *getIntType()) {
              return {};
            }
            break;
          case NodeKind::StringLiteral:
            if (*Expected == *getStringType()) {
              return {};
            }
            break;
          default:
            break;
        }
     }

    // TODO
    // case NodeKind::FunctionExpression:
 
    default:
      {
        auto [Out, Actual] = inferExpr(Env, Expr, RetTy);
        Out.push_back(new CTypesEqual(Actual, Expected, Expr));
        return Out;
      }

  }

}

void Checker::solve(const std::vector<Constraint*>& Constraints) {
  for (auto C: Constraints) {
    switch (C->getKind()) {
      case ConstraintKind::TypesEqual:
        {
          auto D = static_cast<CTypesEqual*>(C);
          unifyTypeType(D->getLeft(), D->getRight(), D->getOrigin());
          break;
        }
    }
  }
}

void Checker::unifyTypeType(Type* A, Type* B, Node* N) {
  A = A->find();
  B = B->find();
  if (A->getKind() == TypeKind::Var) {
    auto TV = static_cast<TVar*>(A);
    // TODO occurs check
    TV->set(B);
    return;
  }
  if (B->getKind() == TypeKind::Var) {
    unifyTypeType(B, A, N);
    return;
  }
  if (A->getKind() == TypeKind::Con && B->getKind() == TypeKind::Con) {
    auto C1 = static_cast<TCon*>(A);
    auto C2 = static_cast<TCon*>(B);
    if (C1->getName() == C2->getName()) {
      return;
    }
  }
  if (A->getKind() == TypeKind::Fun && B->getKind() == TypeKind::Fun) {
    auto F1 = static_cast<TFun*>(A);
    auto F2 = static_cast<TFun*>(B);
    unifyTypeType(F1->getLeft(), F2->getLeft(), N);
    unifyTypeType(F1->getRight(), F2->getRight(), N);
    return;
  }
  DE.add<TypeMismatchError>(A, B, N);
}

void Checker::run(SourceFile* SF) {
  TypeEnv Env;
  Env.add("Int", getIntType(), SymbolKind::Type);
  Env.add("Bool", getBoolType(), SymbolKind::Type);
  Env.add("String", getStringType(), SymbolKind::Type);
  Env.add("True", getBoolType(), SymbolKind::Var);
  Env.add("False", getBoolType(), SymbolKind::Var);
  Env.add("+", new TFun(getIntType(), new TFun(getIntType(), getIntType())), SymbolKind::Var);
  Env.add("-", new TFun(getIntType(), new TFun(getIntType(), getIntType())), SymbolKind::Var);
  auto Out = inferSourceFile(Env, SF);
  solve(Out);
}

Type* resolveType(Type* Ty) {
  switch (Ty->getKind()) {
    case TypeKind::App:
      {
        auto A = static_cast<TApp*>(Ty);
        auto NewLeft = resolveType(A->getLeft());
        auto NewRight = resolveType(A->getRight());
        if (A->getLeft() == NewLeft && A->getRight() == NewRight) {
          return Ty;
        }
        return new TApp(NewLeft, NewRight);
      }
    case TypeKind::Con:
      return Ty;
    case TypeKind::Var:
      {
        auto NewTy = Ty->find();
        if (NewTy->getKind() != TypeKind::Var) {
          return resolveType(NewTy);
        } else {
          return NewTy;
        }
      }
    case TypeKind::Fun:
      {
        auto F = static_cast<TFun*>(Ty);
        auto NewLeft = resolveType(F->getLeft());
        auto NewRight = resolveType(F->getRight());
        if (F->getLeft() == NewLeft && F->getRight() == NewRight) {
          return Ty;
        }
        return new TFun(NewLeft, NewRight);
      }
  }
}

Type* Checker::getTypeOfNode(Node* N) {
  auto M = cast<TypedNode>(N);
  return resolveType(M->getType());
}

}
