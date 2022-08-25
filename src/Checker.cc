
#include <stack>

#include "bolt/Diagnostics.hpp"
#include "zen/config.hpp"

#include "bolt/CST.hpp"
#include "bolt/Checker.hpp"

namespace bolt {

  std::string describe(const Type* Ty);

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
      case TypeKind::Tuple:
      {
        auto Y = static_cast<TTuple*>(this);
        for (auto Ty: Y->ElementTypes) {
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
        return Match != Sub.end() ? Match->second->substitute(Sub) : Y;
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
        return new TCon(Y->Id, NewArgs, Y->DisplayName);
      }
      case TypeKind::Tuple:
      {
        auto Y = static_cast<TTuple*>(this);
        std::vector<Type*> NewElementTypes;
        for (auto Ty: Y->ElementTypes) {
          NewElementTypes.push_back(Ty->substitute(Sub));
        }
        return new TTuple(NewElementTypes);
      }
    }
  }

  Constraint* Constraint::substitute(const TVSub &Sub) {
    switch (Kind) {
      case ConstraintKind::Equal:
      {
        auto Y = static_cast<CEqual*>(this);
        return new CEqual(Y->Left->substitute(Sub), Y->Right->substitute(Sub), Y->Source);
      }
      case ConstraintKind::Many:
      {
        auto Y = static_cast<CMany*>(this);
        auto NewConstraints = new ConstraintSet();
        for (auto Element: Y->Constraints) {
          NewConstraints->push_back(Element->substitute(Sub));
        }
        return new CMany(*NewConstraints);
      }
      case ConstraintKind::Empty:
        return this;
    }
  }

  Scheme* InferContext::lookup(ByteString Name) {
    InferContext* Curr = this;
    for (;;) {
      auto Match = Curr->Env.find(Name);
      if (Match != Curr->Env.end()) {
        return &Match->second;
      }
      Curr = Curr->Parent;
      if (Curr == nullptr) {
        return nullptr;
      }
    }
  }

  Type* InferContext::lookupMono(ByteString Name) {
    auto Scm = lookup(Name);
    if (Scm == nullptr) {
      return nullptr;
    }
    auto& F = Scm->as<Forall>();
    ZEN_ASSERT(F.TVs == nullptr || F.TVs->empty());
    return F.Type;
  }

  void InferContext::addBinding(ByteString Name, Scheme S) {
    Env.emplace(Name, S);
  }

  void InferContext::addConstraint(Constraint *C) {
    Constraints.push_back(C);
  }

  Checker::Checker(DiagnosticEngine& DE):
    DE(DE) {
      BoolType = new TCon(nextConTypeId++, {}, "Bool");
      IntType = new TCon(nextConTypeId++, {}, "Int");
      StringType = new TCon(nextConTypeId++, {}, "String");
    }

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

      case NodeType::IfStatement:
      {
        auto Y = static_cast<IfStatement*>(X);
        for (auto Part: Y->Parts) {
          if (Part->Test != nullptr) {
            Ctx.addConstraint(new CEqual { BoolType, inferExpression(Part->Test, Ctx), Part->Test });
          }
          for (auto Element: Part->Elements) {
            infer(Element, Ctx);
          }
        }
        break;
      }

      case NodeType::LetDeclaration:
      {
        auto Y = static_cast<LetDeclaration*>(X);

        auto NewCtx = new InferContext { Ctx };

        Type* Ty;
        if (Y->TypeAssert) {
          Ty = inferTypeExpression(Y->TypeAssert->TypeExpression, *NewCtx);
        } else {
          Ty = createTypeVar(*NewCtx);
        }

        std::vector<Type*> ParamTypes;
        Type* RetType;

        for (auto Param: Y->Params) {
          // TODO incorporate Param->TypeAssert or make it a kind of pattern
          TVar* TV = createTypeVar(*NewCtx);
          TVSet NoTVs;
          ConstraintSet NoConstraints;
          inferBindings(Param->Pattern, TV, *NewCtx, NoConstraints, NoTVs);
          ParamTypes.push_back(TV);
        }

        if (Y->Body) {
          switch (Y->Body->Type) {
            case NodeType::LetExprBody:
            {
              auto Z = static_cast<LetExprBody*>(Y->Body);
              RetType = inferExpression(Z->Expression, *NewCtx);
              break;
            }
            case NodeType::LetBlockBody:
            {
              auto Z = static_cast<LetBlockBody*>(Y->Body);
              RetType = createTypeVar(*NewCtx);
              NewCtx->ReturnType = RetType;
              for (auto Element: Z->Elements) {
                infer(Element, *NewCtx);
              }
              break;
            }
            default:
              ZEN_UNREACHABLE
          }
        } else {
          RetType = createTypeVar(*NewCtx);
        }

        NewCtx->addConstraint(new CEqual { Ty, new TArrow(ParamTypes, RetType), X });

        inferBindings(Y->Pattern, Ty, Ctx, NewCtx->Constraints, NewCtx->TVs);

        break;
      }

      case NodeType::ReturnStatement:
      {
        auto Y = static_cast<ReturnStatement*>(X);
        Type* ReturnType;
        if (Y->Expression) {
          ReturnType = inferExpression(Y->Expression, Ctx);
        } else {
          ReturnType = new TTuple({});
        }
        ZEN_ASSERT(Ctx.ReturnType != nullptr);
        Ctx.addConstraint(new CEqual { ReturnType, Ctx.ReturnType, X });
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

  TVar* Checker::createTypeVar(InferContext& Ctx) {
    auto TV = new TVar(nextTypeVarId++);
    Ctx.TVs.emplace(TV);
    return TV;
  }

  Type* Checker::instantiate(Scheme& S, InferContext& Ctx, Node* Source) {

    switch (S.getKind()) {

      case SchemeKind::Forall:
      {
        auto& F = S.as<Forall>();

        TVSub Sub;
        if (F.TVs) {
          for (auto TV: *F.TVs) {
            Sub[TV] = createTypeVar(Ctx);
          }
        }

        if (F.Constraints) {

          for (auto Constraint: *F.Constraints) {

            auto NewConstraint = Constraint->substitute(Sub);

            // This makes error messages prettier by relating the typing failure
            // to the call site rather than the definition.
            if (NewConstraint->getKind() == ConstraintKind::Equal) {
                static_cast<CEqual *>(NewConstraint)->Source = Source;
            }

            Ctx.addConstraint(NewConstraint);
          }
        }

        return F.Type->substitute(Sub);
      }

    }

  }

  Type* Checker::inferTypeExpression(TypeExpression* X, InferContext& Ctx) {

    switch (X->Type) {

      case NodeType::ReferenceTypeExpression:
      {
        auto Y = static_cast<ReferenceTypeExpression*>(X);
        auto Ty = Ctx.lookupMono(Y->Name->Name->Text);
        if (Ty == nullptr) {
          DE.add<BindingNotFoundDiagnostic>(Y->Name->Name->Text, Y->Name->Name);
          return new TAny();
        }
        return Ty;
      }

      case NodeType::ArrowTypeExpression:
      {
        auto Y = static_cast<ArrowTypeExpression*>(X);
        std::vector<Type*> ParamTypes;
        for (auto ParamType: Y->ParamTypes) {
          ParamTypes.push_back(inferTypeExpression(ParamType, Ctx));
        }
        auto ReturnType = inferTypeExpression(Y->ReturnType, Ctx);
        return new TArrow(ParamTypes, ReturnType);
      }

      default:
        ZEN_UNREACHABLE

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
            Ty = Ctx.lookupMono("Int");
            break;
          case NodeType::StringLiteral:
            Ty = Ctx.lookupMono("String");
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
        auto Scm = Ctx.lookup(Y->Name->Name->Text);
        if (Scm == nullptr) {
          DE.add<BindingNotFoundDiagnostic>(Y->Name->Name->Text, Y->Name);
          return new TAny();
        }
        return instantiate(*Scm, Ctx, X);
      }

      case NodeType::CallExpression:
      {
        auto Y = static_cast<CallExpression*>(X);
        auto OpTy = inferExpression(Y->Function, Ctx);
        auto RetType = createTypeVar(Ctx);
        std::vector<Type*> ArgTypes;
        for (auto Arg: Y->Args) {
          ArgTypes.push_back(inferExpression(Arg, Ctx));
        }
        Ctx.addConstraint(new CEqual { OpTy, new TArrow(ArgTypes, RetType), X });
        return RetType;
      }

      case NodeType::InfixExpression:
      {
        auto Y = static_cast<InfixExpression*>(X);
        auto Scm = Ctx.lookup(Y->Operator->getText());
        if (Scm == nullptr) {
          DE.add<BindingNotFoundDiagnostic>(Y->Operator->getText(), Y->Operator);
          return new TAny();
        }
        auto OpTy = instantiate(*Scm, Ctx, Y->Operator);
        auto RetTy = createTypeVar(Ctx);
        std::vector<Type*> ArgTys;
        ArgTys.push_back(inferExpression(Y->LHS, Ctx));
        ArgTys.push_back(inferExpression(Y->RHS, Ctx));
        Ctx.addConstraint(new CEqual { new TArrow(ArgTys, RetTy), OpTy, X });
        return RetTy;
      }

      default:
        ZEN_UNREACHABLE

    }

  }

  void Checker::inferBindings(Pattern* Pattern, Type* Type, InferContext& Ctx, ConstraintSet& Constraints, TVSet& TVs) {

    switch (Pattern->Type) {

      case NodeType::BindPattern:
        Ctx.addBinding(static_cast<BindPattern*>(Pattern)->Name->Text, Forall(TVs, Constraints, Type));
        break;

      default:
        ZEN_UNREACHABLE

    }
  }

  void Checker::check(SourceFile *SF) {
    InferContext Toplevel;
    Toplevel.addBinding("String", Forall(StringType));
    Toplevel.addBinding("Int", Forall(IntType));
    Toplevel.addBinding("Bool", Forall(BoolType));
    Toplevel.addBinding("True", Forall(BoolType));
    Toplevel.addBinding("False", Forall(BoolType));
    Toplevel.addBinding("+", Forall(new TArrow({ IntType, IntType }, IntType)));
    Toplevel.addBinding("-", Forall(new TArrow({ IntType, IntType }, IntType)));
    Toplevel.addBinding("*", Forall(new TArrow({ IntType, IntType }, IntType)));
    Toplevel.addBinding("/", Forall(new TArrow({ IntType, IntType }, IntType)));
    infer(SF, Toplevel);
    solve(new CMany(Toplevel.Constraints));
  }

  void Checker::solve(Constraint* Constraint) {

    std::stack<class Constraint*> Queue;
    Queue.push(Constraint);
    TVSub Solution;

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
          std::cerr << describe(Y->Left) << " ~ " << describe(Y->Right) << std::endl;
          if (!unify(Y->Left, Y->Right, Solution)) {
            DE.add<UnificationErrorDiagnostic>(Y->Left->substitute(Solution), Y->Right->substitute(Solution), Y->Source);
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
        return false;
      }
      Solution[Y] = B;
      return true;
    }

    if (B->getKind() == TypeKind::Var) {
      return unify(B, A, Solution);
    }

    if (A->getKind() == TypeKind::Any || B->getKind() == TypeKind::Any) {
      return true;
    }

    if (A->getKind() == TypeKind::Arrow && B->getKind() == TypeKind::Arrow) {
      auto Y = static_cast<TArrow*>(A);
      auto Z = static_cast<TArrow*>(B);
      if (Y->ParamTypes.size() != Z->ParamTypes.size()) {
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

    if (A->getKind() == TypeKind::Tuple && B->getKind() == TypeKind::Tuple) {
      auto Y = static_cast<TTuple*>(A);
      auto Z = static_cast<TTuple*>(B);
      if (Y->ElementTypes.size() != Z->ElementTypes.size()) {
        return false;
      }
      auto Count = Y->ElementTypes.size();
      bool Success = true;
      for (size_t I = 0; I < Count; I++) {
        if (!unify(Y->ElementTypes[I], Z->ElementTypes[I], Solution)) {
          Success = false;
        }
      }
      return Success;
    }

    if (A->getKind() == TypeKind::Con && B->getKind() == TypeKind::Con) {
      auto Y = static_cast<TCon*>(A);
      auto Z = static_cast<TCon*>(B);
      if (Y->Id != Z->Id) {
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

    return false;
  }

}

