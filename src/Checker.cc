
#include <stack>

#include "bolt/Diagnostics.hpp"
#include "zen/config.hpp"

#include "bolt/CST.hpp"
#include "bolt/Checker.hpp"

namespace bolt {

  std::string describe(const Type* Ty);

  void Type::addTypeVars(TVSet& TVs) {
    switch (Kind) {
      case TypeKind::Var:
        TVs.emplace(static_cast<TVar*>(this));
        break;
      case TypeKind::Arrow:
      {
        auto Y = static_cast<TArrow*>(this);
        for (auto Ty: Y->ParamTypes) {
          Ty->addTypeVars(TVs);
        }
        Y->ReturnType->addTypeVars(TVs);
        break;
      }
      case TypeKind::Con:
      {
        auto Y = static_cast<TCon*>(this);
        for (auto Ty: Y->Args) {
          Ty->addTypeVars(TVs);
        }
        break;
      }
      case TypeKind::Tuple:
      {
        auto Y = static_cast<TTuple*>(this);
        for (auto Ty: Y->ElementTypes) {
          Ty->addTypeVars(TVs);
        }
        break;
      }
      case TypeKind::Any:
        break;
    }
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
        bool Changed = false;
        std::vector<Type*> NewParamTypes;
        for (auto Ty: Y->ParamTypes) {
          auto NewParamType = Ty->substitute(Sub);
          if (NewParamType != Ty) {
            Changed = true;
          }
          NewParamTypes.push_back(NewParamType);
        }
        auto NewRetTy = Y->ReturnType->substitute(Sub) ;
        if (NewRetTy != Y->ReturnType) {
          Changed = true;
        }
        return Changed ? new TArrow(NewParamTypes, NewRetTy) : this;
      }
      case TypeKind::Any:
        return this;
      case TypeKind::Con:
      {
        auto Y = static_cast<TCon*>(this);
        bool Changed = false;
        std::vector<Type*> NewArgs;
        for (auto Arg: Y->Args) {
          auto NewArg = Arg->substitute(Sub);
          if (NewArg != Arg) {
            Changed = true;
          }
          NewArgs.push_back(NewArg);
        }
        return Changed ? new TCon(Y->Id, NewArgs, Y->DisplayName) : this;
      }
      case TypeKind::Tuple:
      {
        auto Y = static_cast<TTuple*>(this);
        bool Changed = false;
        std::vector<Type*> NewElementTypes;
        for (auto Ty: Y->ElementTypes) {
          auto NewElementType = Ty->substitute(Sub);
          if (NewElementType != Ty) {
            Changed = true;
          }
          NewElementTypes.push_back(NewElementType);
        }
        return Changed ? new TTuple(NewElementTypes) : this;
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
        for (auto Element: Y->Elements) {
          NewConstraints->push_back(Element->substitute(Sub));
        }
        return new CMany(*NewConstraints);
      }
      case ConstraintKind::Empty:
        return this;
    }
  }

  Checker::Checker(DiagnosticEngine& DE):
    DE(DE) {
      BoolType = new TCon(nextConTypeId++, {}, "Bool");
      IntType = new TCon(nextConTypeId++, {}, "Int");
      StringType = new TCon(nextConTypeId++, {}, "String");
    }

  Scheme* Checker::lookup(ByteString Name) {
    for (auto Iter = Contexts.rbegin(); Iter != Contexts.rend(); Iter++) {
      auto Curr = *Iter;
      auto Match = Curr->Env.find(Name);
      if (Match != Curr->Env.end()) {
        return &Match->second;
      }
    }
    return nullptr;
  }

  Type* Checker::lookupMono(ByteString Name) {
    auto Scm = lookup(Name);
    if (Scm == nullptr) {
      return nullptr;
    }
    auto& F = Scm->as<Forall>();
    ZEN_ASSERT(F.TVs == nullptr || F.TVs->empty());
    return F.Type;
  }

  void Checker::addBinding(ByteString Name, Scheme S) {
    Contexts.back()->Env.emplace(Name, S);
  }

  Type* Checker::getReturnType() {
    auto Ty = Contexts.back()->ReturnType;
    ZEN_ASSERT(Ty != nullptr);
    return Ty;
  }

  static bool hasTypeVar(TVSet& Set, Type* Type) {
    for (auto TV: Type->getTypeVars()) {
      if (Set.count(TV)) {
        return true;
      }
    }
    return false;
  }

  void Checker::addConstraint(Constraint* Constraint) {
    switch (Constraint->getKind()) {
      case ConstraintKind::Equal:
      {
        auto Y = static_cast<CEqual*>(Constraint);
        for (auto Iter = Contexts.rbegin(); Iter != Contexts.rend(); Iter++) {
          auto& Ctx = **Iter;
          if (hasTypeVar(Ctx.TVs, Y->Left) || hasTypeVar(Ctx.TVs, Y->Right)) {
            Ctx.Constraints.push_back(Constraint);
            return;
          }
        }
        Contexts.front()->Constraints.push_back(Constraint);
        //auto I = std::max(Y->Left->MaxDepth, Y->Right->MaxDepth);
        //ZEN_ASSERT(I < Contexts.size());
        //auto Ctx = Contexts[I];
        //Ctx->Constraints.push_back(Constraint);
        break;
      }
      case ConstraintKind::Many:
      {
        auto Y = static_cast<CMany*>(Constraint);
        for (auto Element: Y->Elements) {
          addConstraint(Element);
        }
        break;
      }
      case ConstraintKind::Empty:
        break;
    }
  }

  void Checker::forwardDeclare(Node* X) {

    switch (X->Type) {

      case NodeType::ExpressionStatement:
      case NodeType::ReturnStatement:
      case NodeType::IfStatement:
        break;

      case NodeType::SourceFile:
      {
        auto Y = static_cast<SourceFile*>(X);
        for (auto Element: Y->Elements) {
          forwardDeclare(Element) ;
        }
        break;
      }

      case NodeType::LetDeclaration:
      {
        auto Y = static_cast<LetDeclaration*>(X);

        auto NewCtx = new InferContext();
        Y->Ctx = NewCtx;
        std::cerr << Y << std::endl;

        Contexts.push_back(NewCtx);

        Type* Ty;
        if (Y->TypeAssert) {
          Ty = inferTypeExpression(Y->TypeAssert->TypeExpression);
        } else {
          Ty = createTypeVar();
        }
        Y->Ty = Ty;

        if (Y->Body) {
          switch (Y->Body->Type) {
            case NodeType::LetExprBody:
              break;
            case NodeType::LetBlockBody:
            {
              auto Z = static_cast<LetBlockBody*>(Y->Body);
              for (auto Element: Z->Elements) {
                forwardDeclare(Element);
              }
              break;
            }
            default:
              ZEN_UNREACHABLE
          }
        }

        Contexts.pop_back();

        inferBindings(Y->Pattern, Ty, NewCtx->Constraints, NewCtx->TVs);


        break;
      }

      default:
        ZEN_UNREACHABLE

    }

  }

  void Checker::infer(Node* X) {

    switch (X->Type) {

      case NodeType::SourceFile:
      {
        auto Y = static_cast<SourceFile*>(X);
        for (auto Element: Y->Elements) {
          infer(Element);
        }
        break;
      }

      case NodeType::IfStatement:
      {
        auto Y = static_cast<IfStatement*>(X);
        for (auto Part: Y->Parts) {
          if (Part->Test != nullptr) {
            addConstraint(new CEqual { BoolType, inferExpression(Part->Test), Part->Test });
          }
          for (auto Element: Part->Elements) {
            infer(Element);
          }
        }
        break;
      }

      case NodeType::LetDeclaration:
      {
        auto Y = static_cast<LetDeclaration*>(X);

        auto NewCtx = Y->Ctx;
        Contexts.push_back(NewCtx);

        std::vector<Type*> ParamTypes;
        Type* RetType;

        for (auto Param: Y->Params) {
          // TODO incorporate Param->TypeAssert or make it a kind of pattern
          TVar* TV = createTypeVar();
          TVSet NoTVs;
          ConstraintSet NoConstraints;
          inferBindings(Param->Pattern, TV, NoConstraints, NoTVs);
          ParamTypes.push_back(TV);
        }

        if (Y->Body) {
          switch (Y->Body->Type) {
            case NodeType::LetExprBody:
            {
              auto Z = static_cast<LetExprBody*>(Y->Body);
              RetType = inferExpression(Z->Expression);
              break;
            }
            case NodeType::LetBlockBody:
            {
              auto Z = static_cast<LetBlockBody*>(Y->Body);
              RetType = createTypeVar();
              NewCtx->ReturnType = RetType;
              for (auto Element: Z->Elements) {
                infer(Element);
              }
              break;
            }
            default:
              ZEN_UNREACHABLE
          }
        } else {
          RetType = createTypeVar();
        }

        addConstraint(new CEqual { Y->Ty, new TArrow(ParamTypes, RetType), X });

        Contexts.pop_back();

        break;
      }

      case NodeType::ReturnStatement:
      {
        auto Y = static_cast<ReturnStatement*>(X);
        Type* ReturnType;
        if (Y->Expression) {
          ReturnType = inferExpression(Y->Expression);
        } else {
          ReturnType = new TTuple({});
        }
        addConstraint(new CEqual { ReturnType, getReturnType(), X });
        break;
      }

      case NodeType::ExpressionStatement:
      {
        auto Y = static_cast<ExpressionStatement*>(X);
        inferExpression(Y->Expression);
        break;
      }

      default:
        ZEN_UNREACHABLE

    }

  }

  TVar* Checker::createTypeVar() {
    auto TV = new TVar(nextTypeVarId++);
    Contexts.back()->TVs.emplace(TV);
    return TV;
  }

  Type* Checker::instantiate(Scheme& S, Node* Source) {

    switch (S.getKind()) {

      case SchemeKind::Forall:
      {
        auto& F = S.as<Forall>();

        TVSub Sub;
        for (auto TV: *F.TVs) {
          Sub[TV] = createTypeVar();
        }

        for (auto Constraint: *F.Constraints) {

          auto NewConstraint = Constraint->substitute(Sub);

          // This makes error messages prettier by relating the typing failure
          // to the call site rather than the definition.
          if (NewConstraint->getKind() == ConstraintKind::Equal) {
              static_cast<CEqual *>(NewConstraint)->Source = Source;
          }

          addConstraint(NewConstraint);
        }

        // FIXME substitute should always clone if we set MaxDepth
        auto NewType = F.Type->substitute(Sub);
        //NewType->MaxDepth = std::max(static_cast<unsigned>(Contexts.size()-1), F.Type->MaxDepth);
        return NewType;
      }

    }

  }

  Type* Checker::inferTypeExpression(TypeExpression* X) {

    switch (X->Type) {

      case NodeType::ReferenceTypeExpression:
      {
        auto Y = static_cast<ReferenceTypeExpression*>(X);
        auto Ty = lookupMono(Y->Name->Name->Text);
        if (Ty == nullptr) {
          DE.add<BindingNotFoundDiagnostic>(Y->Name->Name->Text, Y->Name->Name);
          return new TAny();
        }
        Mapping[X] = Ty;
        return Ty;
      }

      case NodeType::ArrowTypeExpression:
      {
        auto Y = static_cast<ArrowTypeExpression*>(X);
        std::vector<Type*> ParamTypes;
        for (auto ParamType: Y->ParamTypes) {
          ParamTypes.push_back(inferTypeExpression(ParamType));
        }
        auto ReturnType = inferTypeExpression(Y->ReturnType);
        auto Ty = new TArrow(ParamTypes, ReturnType);
        Mapping[X] = Ty;
        return Ty;
      }

      default:
        ZEN_UNREACHABLE

    }
  }

  Type* Checker::inferExpression(Expression* X) {

    switch (X->Type) {

      case NodeType::ConstantExpression:
      {
        auto Y = static_cast<ConstantExpression*>(X);
        Type* Ty = nullptr;
        switch (Y->Token->Type) {
          case NodeType::IntegerLiteral:
            Ty = lookupMono("Int");
            break;
          case NodeType::StringLiteral:
            Ty = lookupMono("String");
            break;
          default:
            ZEN_UNREACHABLE
        }
        ZEN_ASSERT(Ty != nullptr);
        Mapping[X] = Ty;
        return Ty;
      }

      case NodeType::ReferenceExpression:
      {
        auto Y = static_cast<ReferenceExpression*>(X);
        ZEN_ASSERT(Y->Name->ModulePath.empty());
        auto Ctx = lookupCall(Y, Y->Name->getSymbolPath());
        if (Ctx) {
          return Ctx->ReturnType;
        }
        auto Scm = lookup(Y->Name->Name->Text);
        if (Scm == nullptr) {
          DE.add<BindingNotFoundDiagnostic>(Y->Name->Name->Text, Y->Name);
          return new TAny();
        }
        auto Ty = instantiate(*Scm, X);
        Mapping[X] = Ty;
        return Ty;
      }

      case NodeType::CallExpression:
      {
        auto Y = static_cast<CallExpression*>(X);
        auto OpTy = inferExpression(Y->Function);
        auto RetType = createTypeVar();
        std::vector<Type*> ArgTypes;
        for (auto Arg: Y->Args) {
          ArgTypes.push_back(inferExpression(Arg));
        }
        addConstraint(new CEqual { OpTy, new TArrow(ArgTypes, RetType), X });
        Mapping[X] = RetType;
        return RetType;
      }

      case NodeType::InfixExpression:
      {
        auto Y = static_cast<InfixExpression*>(X);
        auto Scm = lookup(Y->Operator->getText());
        if (Scm == nullptr) {
          DE.add<BindingNotFoundDiagnostic>(Y->Operator->getText(), Y->Operator);
          return new TAny();
        }
        auto OpTy = instantiate(*Scm, Y->Operator);
        auto RetTy = createTypeVar();
        std::vector<Type*> ArgTys;
        ArgTys.push_back(inferExpression(Y->LHS));
        ArgTys.push_back(inferExpression(Y->RHS));
        addConstraint(new CEqual { new TArrow(ArgTys, RetTy), OpTy, X });
        Mapping[X] = RetTy;
        return RetTy;
      }

      case NodeType::NestedExpression:
      {
        auto Y = static_cast<NestedExpression*>(X);
        return inferExpression(Y->Inner);
      }

      default:
        ZEN_UNREACHABLE

    }

  }

  void Checker::inferBindings(Pattern* Pattern, Type* Type, ConstraintSet& Constraints, TVSet& TVs) {

    switch (Pattern->Type) {

      case NodeType::BindPattern:
        addBinding(static_cast<BindPattern*>(Pattern)->Name->Text, Forall(TVs, Constraints, Type));
        break;

      default:
        ZEN_UNREACHABLE

    }
  }

  TVSub Checker::check(SourceFile *SF) {
    Contexts.push_back(new InferContext {});
    ConstraintSet NoConstraints;
    addBinding("String", Forall(StringType));
    addBinding("Int", Forall(IntType));
    addBinding("Bool", Forall(BoolType));
    addBinding("True", Forall(BoolType));
    addBinding("False", Forall(BoolType));
    auto A = createTypeVar();
    TVSet SingleA { A };
    addBinding("==", Forall(SingleA, NoConstraints, new TArrow({ A, A }, BoolType)));
    addBinding("+", Forall(new TArrow({ IntType, IntType }, IntType)));
    addBinding("-", Forall(new TArrow({ IntType, IntType }, IntType)));
    addBinding("*", Forall(new TArrow({ IntType, IntType }, IntType)));
    addBinding("/", Forall(new TArrow({ IntType, IntType }, IntType)));
    forwardDeclare(SF);
    infer(SF);
    TVSub Solution;
    solve(new CMany(Contexts.front()->Constraints), Solution);
    Contexts.pop_back();
    return Solution;
  }

  void Checker::solve(Constraint* Constraint, TVSub& Solution) {

    std::stack<class Constraint*> Queue;
    Queue.push(Constraint);

    while (!Queue.empty()) {

      auto Constraint = Queue.top();

      Queue.pop();

      switch (Constraint->getKind()) {

        case ConstraintKind::Empty:
          break;

        case ConstraintKind::Many:
        {
          auto Y = static_cast<CMany*>(Constraint);
          for (auto Constraint: Y->Elements) {
            Queue.push(Constraint);
          }
          break;
        }

        case ConstraintKind::Equal:
        {
          auto Y = static_cast<CEqual*>(Constraint);
          //std::cerr << describe(Y->Left) << " ~ " << describe(Y->Right) << std::endl;
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

    if (A->getKind() == TypeKind::Arrow) {
      auto Y = static_cast<TArrow*>(A);
      if (Y->ParamTypes.empty()) {
        return unify(Y->ReturnType, B, Solution);
      }
    }

    if (B->getKind() == TypeKind::Arrow) {
      return unify(B, A, Solution);
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

  InferContext* Checker::lookupCall(Node* Source, SymbolPath Path) {
    auto Def = Source->getScope()->lookup(Path);
    auto Match = CallGraph.find(Def);
    if (Match == CallGraph.end()) {
      return nullptr;
    }
    return Match->second;
  }

  Type* Checker::getType(Node *Node, const TVSub &Solution) {
    auto Match = Mapping.find(Node);
    if (Match == Mapping.end()) {
      return nullptr;
    }
    return Match->second->substitute(Solution);
  }

}

