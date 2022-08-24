
#include <stack>

#include "bolt/Diagnostics.hpp"
#include "zen/config.hpp"

#include "bolt/CST.hpp"
#include "bolt/Checker.hpp"

namespace bolt {

  Scheme* TypeEnv::lookup(ByteString Name) {
    auto Match = Mapping.find(Name);
    if (Match == Mapping.end()) {
      return {};
    }
    return &Match->second;
  }

  Type* TypeEnv::lookupMono(ByteString Name) {
    auto Match = Mapping.find(Name);
    if (Match == Mapping.end()) {
      return nullptr;
    }
    auto& F = Match->second.as<Forall>();
    ZEN_ASSERT(F.TVs.empty());
    return F.Type;
  }

  void TypeEnv::add(ByteString Name, Scheme S) {
    Mapping.emplace(Name, S);
  }

  bool Type::hasTypeVar(const TVar* TV) {
    switch (Kind) {
      case TypeKind::Var:
        return static_cast<TVar*>(this)->Id == TV->Id;
      case TypeKind::Arrow:
      {
        auto Y = static_cast<TArrow*>(this);
        for (auto Ty: Y->ParamTypes) {
          if (Ty->hasTypeVar(TV)) {
            return true;
          }
        }
        return Y->ReturnType->hasTypeVar(TV);
      }
      case TypeKind::Con:
      {
        auto Y = static_cast<TCon*>(this);
        for (auto Ty: Y->Args) {
          if (Ty->hasTypeVar(TV)) {
            return true;
          }
        }
        return false;
      }
      case TypeKind::Any:
        return false;
    }
  }

  Type* Type::substitute(const TVSub &Sub) {
    switch (Kind) {
      case TypeKind::Var:
      {
        auto Y = static_cast<TVar*>(this);
        auto Match = Sub.find(Y);
        return Match != Sub.end() ? Match->second : Y;
      }
      case TypeKind::Arrow:
      {
        auto Y = static_cast<TArrow*>(this);
        std::vector<Type*> NewParamTypes;
        for (auto Ty: Y->ParamTypes) {
          NewParamTypes.push_back(Ty->substitute(Sub));
        }
        auto NewRetTy = Y->ReturnType->substitute(Sub) ;
        return new TArrow(NewParamTypes, NewRetTy);
      }
      case TypeKind::Any:
        return this;
      case TypeKind::Con:
      {
        auto Y = static_cast<TCon*>(this);
        std::vector<Type*> NewArgs;
        for (auto Arg: Y->Args) {
          NewArgs.push_back(Arg->substitute(Sub));
        }
        return new TCon(Y->Id, Y->Args, Y->DisplayName);
      }
    }
  }

  void InferContext::addConstraint(Constraint *C) {
    Constraints.push_back(C);
  }

  Checker::Checker(DiagnosticEngine& DE):
    DE(DE) {}

  void Checker::infer(Node* X, InferContext& Ctx) {

    switch (X->Type) {

      case NodeType::SourceFile:
      {
        auto Y = static_cast<SourceFile*>(X);
        for (auto Element: Y->Elements) {
          infer(Element, Ctx);
        }
        break;
      }

      case NodeType::LetDeclaration:
      {
        // TODO
        break;
      }


      case NodeType::ExpressionStatement:
      {
        auto Y = static_cast<ExpressionStatement*>(X);
        inferExpression(Y->Expression, Ctx);
        break;
      }

      default:
        ZEN_UNREACHABLE

    }

  }

  TVar* Checker::createTypeVar() {
    return new TVar(nextTypeVarId++);
  }

  Type* Checker::instantiate(Scheme& S) {

    switch (S.getKind()) {

      case SchemeKind::Forall:
      {
        auto& F = S.as<Forall>();
        TVSub Sub;
        for (auto TV: F.TVs) {
          Sub[TV] = createTypeVar();
        }
        return F.Type->substitute(Sub);
      }

    }

  }


  Type* Checker::inferExpression(Expression* X, InferContext& Ctx) {

    switch (X->Type) {

      case NodeType::ConstantExpression:
      {
        auto Y = static_cast<ConstantExpression*>(X);
        Type* Ty = nullptr;
        switch (Y->Token->Type) {
          case NodeType::IntegerLiteral:
            Ty = Ctx.Env.lookupMono("Int");
            break;
          case NodeType::StringLiteral:
            Ty = Ctx.Env.lookupMono("String");
            break;
          default:
            ZEN_UNREACHABLE
        }
        ZEN_ASSERT(Ty != nullptr);
        return Ty;
      }

      case NodeType::ReferenceExpression:
      {
        auto Y = static_cast<ReferenceExpression*>(X);
        ZEN_ASSERT(Y->Name->ModulePath.empty());
        auto Scm = Ctx.Env.lookup(Y->Name->Name->Text);
        if (Scm == nullptr) {
          DE.add<BindingNotFoundDiagnostic>(Y->Name->Name->Text, Y->Name);
          return new TAny();
        }
        return instantiate(*Scm);
      }

      case NodeType::InfixExpression:
      {
        auto Y = static_cast<InfixExpression*>(X);
        auto Scm = Ctx.Env.lookup(Y->Operator->getText());
        if (Scm == nullptr) {
          DE.add<BindingNotFoundDiagnostic>(Y->Operator->getText(), Y->Operator);
          return new TAny();
        }
        auto OpTy = instantiate(*Scm);
        auto RetTy = createTypeVar();
        std::vector<Type*> ArgTys;
        ArgTys.push_back(inferExpression(Y->LHS, Ctx));
        ArgTys.push_back(inferExpression(Y->RHS, Ctx));
        Ctx.addConstraint(new CEqual { new TArrow(ArgTys, RetTy), OpTy });
        return RetTy;
      }

      default:
        ZEN_UNREACHABLE

    }

  }

  void Checker::check(SourceFile *SF) {
    TypeEnv Global;
    auto StringTy = new TCon(nextConTypeId++, {}, "String");
    Global.add("String", Forall(StringTy));
    auto IntTy = new TCon(nextConTypeId++, {}, "Int");
    Global.add("Int", Forall(IntTy));
    Global.add("+", Forall(new TArrow({ IntTy, IntTy }, IntTy)));
    ConstraintSet Constraints;
    InferContext Toplevel { Constraints, Global };
    infer(SF, Toplevel);
    solve(new CMany(Constraints));
  }

  void Checker::solve(Constraint* Constraint) {

    std::stack<class Constraint*> Queue;
    Queue.push(Constraint);
    TVSub Sub;

    while (!Queue.empty()) {

      auto Constraint = Queue.top();

      Queue.pop();

      switch (Constraint->getKind()) {

        case ConstraintKind::Empty:
          break;

        case ConstraintKind::Many:
        {
          auto Y = static_cast<CMany*>(Constraint);
          for (auto Constraint: Y->Constraints) {
            Queue.push(Constraint);
          }
          break;
        }

        case ConstraintKind::Equal:
        {
          auto Y = static_cast<CEqual*>(Constraint);
          if (!unify(Y->Left, Y->Right, Sub)) {
            DE.add<UnificationErrorDiagnostic>(Y->Left, Y->Right);
          }
          break;
        }

      }

    }

  }

  bool Checker::unify(Type* A, Type* B, TVSub& Solution) {

    if (A->getKind() == TypeKind::Var) {
      auto Match = Solution.find(static_cast<TVar*>(A));
      if (Match != Solution.end()) {
        A = Match->second;
      }
    }

    if (B->getKind() == TypeKind::Var) {
      auto Match = Solution.find(static_cast<TVar*>(B));
      if (Match != Solution.end()) {
        B = Match->second;
      }
    }

    if (A->getKind() == TypeKind::Var) {
      auto Y = static_cast<TVar*>(A);
      if (B->hasTypeVar(Y)) {
        // TODO occurs check
      }
      Solution[Y] = B;
      return true;
    }

    if (B->getKind() == TypeKind::Var) {
      return unify(B, A, Solution);
    }

    if (A->getKind() == TypeKind::Arrow && B->getKind() == TypeKind::Arrow) {
      auto Y = static_cast<TArrow*>(A);
      auto Z = static_cast<TArrow*>(B);
      if (Y->ParamTypes.size() != Z->ParamTypes.size()) {
        // TODO diagnostic
        return false;
      }
      auto Count = Y->ParamTypes.size();
      for (std::size_t I = 0; I < Count; I++) {
        if (!unify(Y->ParamTypes[I], Z->ParamTypes[I], Solution)) {
          return false;
        }
      }
      return unify(Y->ReturnType, Z->ReturnType, Solution);
    }

    if (A->getKind() == TypeKind::Con && B->getKind() == TypeKind::Arrow) {
      auto Y = static_cast<TCon*>(A);
      auto Z = static_cast<TCon*>(B);
      if (Y->Id != Z->Id) {
        // TODO diagnostic
        return false;
      }
      ZEN_ASSERT(Y->Args.size() == Z->Args.size());
      auto Count = Y->Args.size();
      for (std::size_t I = 0; I < Count; I++) {
        if (!unify(Y->Args[I], Z->Args[I], Solution)) {
          return false;
        }
      }
      return true;
    }

    // TODO diagnostic
    return false;
  }

}

