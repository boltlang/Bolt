
#include <algorithm>
#include <stack>
#include <map>

#include "zen/config.hpp"

#include "bolt/Type.hpp"
#include "bolt/CSTVisitor.hpp"
#include "bolt/DiagnosticEngine.hpp"
#include "bolt/Diagnostics.hpp"
#include "bolt/CST.hpp"
#include "bolt/Checker.hpp"

namespace bolt {

Constraint* Constraint::substitute(const TVSub &Sub) {
  switch (Kind) {
    case ConstraintKind::Equal:
    {
      auto Equal = static_cast<CEqual*>(this);
      return new CEqual(Equal->Left->substitute(Sub), Equal->Right->substitute(Sub), Equal->Source);
    }
    case ConstraintKind::Many:
    {
      auto Many = static_cast<CMany*>(this);
      auto NewConstraints = new ConstraintSet();
      for (auto Element: Many->Elements) {
        NewConstraints->push_back(Element->substitute(Sub));
      }
      return new CMany(*NewConstraints);
    }
    case ConstraintKind::Field:
    {
      auto Field = static_cast<CField*>(this);
      auto NewTupleTy = Field->TupleTy->substitute(Sub);
      auto NewFieldTy = Field->FieldTy->substitute(Sub);
      return new CField(NewTupleTy, Field->I, NewFieldTy, Field->Source);
    }
    case ConstraintKind::Empty:
      return this;
  }
  ZEN_UNREACHABLE
}

Type* Checker::solveType(Type* Ty) {
  return Ty->rewrite([this](auto Ty) { return Ty->find(); }, true);
}

Checker::Checker(const LanguageConfig& Config, DiagnosticEngine& DE):
  Config(Config), DE(DE) {
    BoolType = createConType("Bool");
    IntType = createConType("Int");
    StringType = createConType("String");
    ListType = createConType("List");
    UnitType  = new Type(TTuple({}));
  }

Scheme* Checker::lookup(ByteString Name, SymKind Kind) {
  auto Curr = &getContext();
  for (;;) {
    auto Match = Curr->Env.lookup(Name, Kind);
    if (Match != nullptr) {
      return Match;
    }
    Curr = Curr->Parent;
    if (!Curr) {
      break;
    }
  }
  return nullptr;
}

Type* Checker::lookupMono(ByteString Name, SymKind Kind) {
  auto Scm = lookup(Name, Kind);
  if (Scm == nullptr) {
    return nullptr;
  }
  auto F = static_cast<Forall*>(Scm);
  ZEN_ASSERT(F->TVs == nullptr || F->TVs->empty());
  return F->Type;
}

void Checker::addBinding(ByteString Name, Scheme* Scm, SymKind Kind) {
  getContext().Env.add(Name, Scm, Kind);
}

Type* Checker::getReturnType() {
  auto Ty = getContext().ReturnType;
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

void Checker::setContext(InferContext* Ctx) {
  ActiveContext = Ctx;
}

void Checker::popContext() {
  ZEN_ASSERT(ActiveContext);
  ActiveContext = ActiveContext->Parent;
}

InferContext& Checker::getContext() {
  ZEN_ASSERT(ActiveContext);
  return *ActiveContext;
}

void Checker::makeEqual(Type* A, Type* B, Node* Source) {
  addConstraint(new CEqual(A, B, Source));
}

void Checker::addConstraint(Constraint* C) {

  switch (C->getKind()) {

    case ConstraintKind::Field:
      // FIXME Check if this is all that needs to be done
      getContext().Constraints->push_back(C);
      break;

    case ConstraintKind::Equal:
    {
      auto Y = static_cast<CEqual*>(C);

      // This will store all inference contexts in Contexts, from most local
      // one to most general one. Because this order is not ideal, the code
      // below will have to handle that.
      auto Curr = &getContext();
      std::vector<InferContext*> Contexts;
      for (;;) {
        Contexts.push_back(Curr);
        Curr = Curr->Parent;
        if (!Curr) {
          break;
        }
      }

      std::size_t Global = Contexts.size()-1;

      // If no MaxLevelLeft was found, that means that not a single
      // corresponding type variable was found in the contexts. We set it to
      // Contexts.size()-1, which corresponds to the global inference context.
      std::size_t MaxLevelLeft = Global;
      for (std::size_t I = 0; I < Global; I++) {
        auto Ctx = Contexts[I];
        if (hasTypeVar(*Ctx->TVs, Y->Left)) {
          MaxLevelLeft = I;
          break;
        }
      }

      // Same as above but now mirrored for Y->Right
      std::size_t MaxLevelRight = Global;
      for (std::size_t I = 0; I < Global; I++) {
        auto Ctx = Contexts[I];
        if (hasTypeVar(*Ctx->TVs, Y->Right)) {
          MaxLevelRight = I;
          break;
        }
      }

      // The lowest index is determined by the one that has no type variables
      // in Y->Left AND in Y->Right. This implies max() must be used, so that
      // the very first enounter of a type variable matters.
      auto UpperLevel = std::max(MaxLevelLeft, MaxLevelRight);

      // Now find the lowest index LowerLevel such that all the contexts that are more
      // local do not contain any type variables that are present in the
      // equality constraint.
      std::size_t LowerLevel = UpperLevel;
      for (std::size_t I = Global; I-- > 0; ) {
        auto Ctx = Contexts[I];
        if (hasTypeVar(*Ctx->TVs, Y->Left) || hasTypeVar(*Ctx->TVs, Y->Right)) {
          LowerLevel = I;
          break;
        }
      }

      if (UpperLevel == LowerLevel || MaxLevelLeft == Global || MaxLevelRight == Global) {
        unify(Y->Left, Y->Right, Y->Source);
      } else {
        Contexts[UpperLevel]->Constraints->push_back(C);
      }

      break;
    }

    case ConstraintKind::Many:
    {
      auto Y = static_cast<CMany*>(C);
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

  switch (X->getKind()) {

    case NodeKind::ExpressionStatement:
    case NodeKind::ReturnStatement:
    case NodeKind::IfStatement:
      break;

    case NodeKind::SourceFile:
    {
      auto File = static_cast<SourceFile*>(X);
      for (auto Element: File->Elements) {
        forwardDeclare(Element) ;
      }
      break;
    }

    case NodeKind::ClassDeclaration:
    {
      auto Class = static_cast<ClassDeclaration*>(X);
      // for (auto TE: Class->TypeVars) {
      //   auto TV = new TVarRigid(NextTypeVarId++, TE->Name->getCanonicalText());
      //   // TV->Contexts.emplace(Class->Name->getCanonicalText());
      //   TE->setType(TV);
      // }
      for (auto Element: Class->Elements) {
        forwardDeclare(Element);
      }
      break;
    }

    case NodeKind::InstanceDeclaration:
    {
      auto Decl = static_cast<InstanceDeclaration*>(X);

      // Needed to set the associated Type on the CST node
      for (auto TE: Decl->TypeExps) {
        inferTypeExpression(TE);
      }

      auto Match = InstanceMap.find(getCanonicalText(Decl->Name));
      if (Match == InstanceMap.end()) {
        InstanceMap.emplace(getCanonicalText(Decl->Name), std::vector { Decl });
      } else {
        Match->second.push_back(Decl);
      }

      for (auto Element: Decl->Elements) {
        forwardDeclare(Element);
      }

      break;
    }

    case NodeKind::LetDeclaration:
    {
      // Function declarations are handled separately in forwardDeclareLetDeclaration() and inferExpression()
      auto Decl = static_cast<LetDeclaration*>(X);
      if (!Decl->isVariable()) {
        break;
      }
      Type* Ty;
      if (Decl->TypeAssert) {
        Ty = inferTypeExpression(Decl->TypeAssert->TypeExpression);
      } else {
        Ty = createTypeVar();
      }
      Decl->setType(Ty);
      break;
    }

    case NodeKind::VariantDeclaration:
    {
      auto Decl = static_cast<VariantDeclaration*>(X);

      setContext(Decl->Ctx);

      std::vector<Type*> Vars;
      for (auto TE: Decl->TVs) {
        auto TV = createRigidVar(getCanonicalText(TE->Name));
        Decl->Ctx->TVs->emplace(TV);
        Decl->Ctx->Env.add(getCanonicalText(TE->Name), new Forall(TV), SymKind::Type);
        Vars.push_back(TV);
      }

      Type* Ty = createConType(getCanonicalText(Decl->Name));

      // Build the type that is actually returned by constructor functions
      auto RetTy = Ty;
      for (auto Var: Vars) {
        RetTy = new Type(TApp(RetTy, Var));
      }

      // Must be added early so we can create recursive types
      Decl->Ctx->Parent->Env.add(getCanonicalText(Decl->Name), new Forall(Ty), SymKind::Type);

      for (auto Member: Decl->Members) {
        switch (Member->getKind()) {
          case NodeKind::TupleVariantDeclarationMember:
          {
            auto TupleMember = static_cast<TupleVariantDeclarationMember*>(Member);
            std::vector<Type*> ParamTypes;
            for (auto Element: TupleMember->Elements) {
              // inferTypeExpression will look up any TVars that were part of the signature of Decl
              ParamTypes.push_back(inferTypeExpression(Element, false));
            }
            Decl->Ctx->Parent->Env.add(
              getCanonicalText(TupleMember->Name),
              new Forall(
                Decl->Ctx->TVs,
                Decl->Ctx->Constraints,
                Type::buildArrow(ParamTypes, RetTy)
              ),
              SymKind::Var
            );
            break;
          }
          case NodeKind::RecordVariantDeclarationMember:
          {
            // TODO
            break;
          }
          default:
            ZEN_UNREACHABLE
        }
      }

      popContext();

      break;
    }

    case NodeKind::RecordDeclaration:
    {
      auto Decl = static_cast<RecordDeclaration*>(X);

      setContext(Decl->Ctx);

      std::vector<Type*> Vars;
      for (auto TE: Decl->Vars) {
        auto TV = createRigidVar(getCanonicalText(TE->Name));
        Decl->Ctx->TVs->emplace(TV);
        Decl->Ctx->Env.add(getCanonicalText(TE->Name), new Forall(TV), SymKind::Type);
        Vars.push_back(TV);
      }

      auto Name = getCanonicalText(Decl->Name);
      auto Ty = createConType(Name);

      // Must be added early so we can create recursive types
      Decl->Ctx->Parent->Env.add(Name, new Forall(Ty), SymKind::Type);

      Type* RetTy = Ty;
      for (auto TV: Vars) {
        RetTy = new Type(TApp(RetTy, TV));
      }

      // Corresponds to the logic of one branch of a VariantDeclarationMember
      Type* FieldsTy = new Type(TNil());
      for (auto Field: Decl->Fields) {
        FieldsTy = new Type(
          TField(
            getCanonicalText(Field->Name),
            new Type(TPresent(inferTypeExpression(Field->TypeExpression, false))),
            FieldsTy
          )
        );
      }
      Decl->Ctx->Parent->Env.add(
        Name,
        new Forall(
          Decl->Ctx->TVs,
          Decl->Ctx->Constraints,
          new Type(TArrow(FieldsTy, RetTy))
        ),
        SymKind::Var
      );

      popContext();

      break;
    }

    default:
      ZEN_UNREACHABLE

  }

}

void Checker::initialize(Node* N) {

  struct Init : public CSTVisitor<Init> {

    Checker& C;

    std::stack<InferContext*> Contexts;

    InferContext* createDerivedContext() {
      return C.createInferContext(Contexts.top());
    }

    void visitVariantDeclaration(VariantDeclaration* Decl) {
      Decl->Ctx = createDerivedContext();
    }

    void visitRecordDeclaration(RecordDeclaration* Decl) {
      Decl->Ctx = createDerivedContext();
    }

    void visitMatchCase(MatchCase* C) {
      C->Ctx = createDerivedContext();
      Contexts.push(C->Ctx);
      visitEachChild(C);
      Contexts.pop();
    }

    void visitSourceFile(SourceFile* SF) {
      SF->Ctx = C.createInferContext();
      Contexts.push(SF->Ctx);
      visitEachChild(SF);
      Contexts.pop();
    }

    void visitLetDeclaration(LetDeclaration* Let) {
      if (Let->isFunction()) {
        Let->Ctx = createDerivedContext();
        Contexts.push(Let->Ctx);
        visitEachChild(Let);
        Contexts.pop();
      }
    }

    // void visitVariableDeclaration(VariableDeclaration* Var) {
    //   Var->Ctx = Contexts.top();
    //   visitEachChild(Var);
    // }

  };

  Init I { {}, *this };
  I.visit(N);

}

void Checker::forwardDeclareFunctionDeclaration(LetDeclaration* Let, TVSet* TVs, ConstraintSet* Constraints) {

  if (!Let->isFunction()) {
    return;
  }

  // std::cerr << "declare " << Let->getNameAsString() << std::endl;

  setContext(Let->Ctx);

  auto addClassVars = [&](ClassDeclaration* Class, bool IsRigid) {
    auto Id = getCanonicalText(Class->Name);
    auto Ctx = &getContext();
    std::vector<Type*> Out;
    for (auto TE: Class->TypeVars) {
      auto Name = getCanonicalText(TE->Name);
      auto TV = IsRigid ? createRigidVar(Name) : createTypeVar();
      TV->asVar().Context.emplace(Id);
      Ctx->Env.add(Name, new Forall(TV), SymKind::Type);
      Out.push_back(TV);
    }
    return Out;
  };

  // If declaring a let-declaration inside a type class declaration,
  // we need to mark that the let-declaration requires this class.
  // This marking is set on the rigid type variables of the class, which
  // are then added to this local type environment.
  if (Let->isClass()) {
    addClassVars(static_cast<ClassDeclaration*>(Let->Parent), true);
  }

  // Here we infer the primary type of the let declaration. If there's a
  // type assert, that assert should be authoritative so we use that.
  // Otherwise, the type is not further specified and we create a new
  // unification variable.
  Type* Ty;
  if (Let->TypeAssert) {
    Ty = inferTypeExpression(Let->TypeAssert->TypeExpression);
  } else {
    Ty = createTypeVar();
  }
  Let->setType(Ty);

  // If declaring a let-declaration inside a type instance declaration,
  // we need to perform some work to make sure the type asserts of the
  // corresponding let-declaration in the type class declaration are
  // accounted for.
  if (Let->isInstance()) {

    auto Instance = static_cast<InstanceDeclaration*>(Let->Parent);
    auto Class = cast<ClassDeclaration>(Instance->getScope()->lookup({ {}, getCanonicalText(Instance->Name) }, SymbolKind::Class));
    // TODO check if `Class` is nullptr
    auto SigLet = cast<LetDeclaration>(Class->getScope()->lookupDirect({ {}, Let->getNameAsString() }, SymbolKind::Var));

    auto Params = addClassVars(Class, false);

    // The type asserts in the type class declaration might make use of
    // the type parameters of the type class declaration, so it is
    // important to make them available in the type environment. Moreover,
    // we will be unifying them with the actual types declared in the
    // instance declaration, so we keep track of them.
    // std::vector<TVar *> Params;
    // TVSub Sub;
    // for (auto TE: Class->TypeVars) {
    //   auto TV = createTypeVar();
    //   Sub.emplace(cast<TVar>(TE->getType()), TV);
    //   Params.push_back(TV);
    // }

    // Here we do the actual unification of e.g. Eq a with Eq Bool. The
    // unification variables we created previously will be unified with
    // e.g. Bool, which causes the type assert to also collapse to e.g.
    // Bool -> Bool -> Bool.
    for (auto [Param, TE] : zen::zip(Params, Instance->TypeExps)) {
      makeEqual(Param, TE->getType(), TE);
    }

    // It would be very strange if there was no type assert in the type
    // class let-declaration but we rather not let the compiler crash if that happens.
    if (SigLet->TypeAssert) {
      // Note that we can't do SigLet->TypeAssert->TypeExpression->getType()
      // because we need to re-generate the type within the local context of
      // this let-declaration.
      // TODO make CEqual accept multiple nodes
      makeEqual(Ty, inferTypeExpression(SigLet->TypeAssert->TypeExpression), Let);
    }

  }

  if (Let->Body) {
    switch (Let->Body->getKind()) {
      case NodeKind::LetExprBody:
        break;
      case NodeKind::LetBlockBody:
      {
        auto Block = static_cast<LetBlockBody*>(Let->Body);
        Let->Ctx->ReturnType = createTypeVar();
        for (auto Element: Block->Elements) {
          forwardDeclare(Element);
        }
        break;
      }
      default:
        ZEN_UNREACHABLE
    }
  }

  if (!Let->isInstance()) {
    Let->Ctx->Parent->Env.add(Let->getNameAsString(), new Forall(Let->Ctx->TVs, Let->Ctx->Constraints, Ty), SymKind::Var);
  }

}

void Checker::inferFunctionDeclaration(LetDeclaration* Decl) {

  if (!Decl->isFunction()) {
    return;
  }

  // std::cerr << "infer " << Decl->getNameAsString() << std::endl;

  auto OldCtx = ActiveContext;
  setContext(Decl->Ctx);

  std::vector<Type*> ParamTypes;
  Type* RetType;

  for (auto Param: Decl->Params) {
    ParamTypes.push_back(inferPattern(Param->Pattern));
  }

  if (Decl->Body) {
    switch (Decl->Body->getKind()) {
      case NodeKind::LetExprBody:
      {
        auto Expr = static_cast<LetExprBody*>(Decl->Body);
        RetType = inferExpression(Expr->Expression);
        break;
      }
      case NodeKind::LetBlockBody:
      {
        auto Block = static_cast<LetBlockBody*>(Decl->Body);
        RetType = Decl->Ctx->ReturnType;
        for (auto Element: Block->Elements) {
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

  makeEqual(Decl->getType(), Type::buildArrow(ParamTypes, RetType), Decl);

  setContext(OldCtx);
}

void Checker::infer(Node* N) {

  switch (N->getKind()) {

    case NodeKind::SourceFile:
    {
      auto File = static_cast<SourceFile*>(N);
      for (auto Element: File->Elements) {
        infer(Element);
      }
      break;
    }

    case NodeKind::ClassDeclaration:
    {
      auto Decl = static_cast<ClassDeclaration*>(N);
      for (auto Element: Decl->Elements) {
        infer(Element);
      }
      break;
    }

    case NodeKind::InstanceDeclaration:
    {
      auto Decl = static_cast<InstanceDeclaration*>(N);
      for (auto Element: Decl->Elements) {
        infer(Element);
      }
      break;
    }

    case NodeKind::VariantDeclaration:
    case NodeKind::RecordDeclaration:
      // Nothing to do for a type-level declaration
      break;

    case NodeKind::IfStatement:
    {
      auto IfStmt = static_cast<IfStatement*>(N);
      for (auto Part: IfStmt->Parts) {
        if (Part->Test != nullptr) {
          makeEqual(BoolType, inferExpression(Part->Test), Part->Test);
        }
        for (auto Element: Part->Elements) {
          infer(Element);
        }
      }
      break;
    }

    case NodeKind::ReturnStatement:
    {
      auto RetStmt = static_cast<ReturnStatement*>(N);
      Type* ReturnType;
      if (RetStmt->Expression) {
        makeEqual(inferExpression(RetStmt->Expression), getReturnType(), RetStmt->Expression);
      } else {
        ReturnType = UnitType;
        makeEqual(UnitType, getReturnType(), N);
      }
      break;
    }

    case NodeKind::LetDeclaration:
    {
      // Function declarations are handled separately in inferFunctionDeclaration()
      auto Decl = static_cast<LetDeclaration*>(N);
      if (Decl->Visited) {
        break;
      }
      if (Decl->isFunction()) {
        Decl->IsCycleActive = true;
        Decl->Visited = true;
        inferFunctionDeclaration(Decl);
        Decl->IsCycleActive = false;
      } else if (Decl->isVariable()) {
        auto Ty = Decl->getType();
        if (Decl->Body) {
          ZEN_ASSERT(Decl->Body->getKind() == NodeKind::LetExprBody);
          auto E = static_cast<LetExprBody*>(Decl->Body);
          auto Ty2 = inferExpression(E->Expression);
          makeEqual(Ty, Ty2, Decl);
        }
        auto Ty3 = inferPattern(Decl->Pattern);
        makeEqual(Ty, Ty3, Decl);
      }
      break;
    }

    case NodeKind::ExpressionStatement:
    {
      auto ExprStmt = static_cast<ExpressionStatement*>(N);
      inferExpression(ExprStmt->Expression);
      break;
    }

    default:
      ZEN_UNREACHABLE

  }

}

Type* Checker::createConType(ByteString Name) {
  return new Type(TCon(NextConTypeId++, Name));
}

Type* Checker::createRigidVar(ByteString Name) {
  auto TV = new Type(TVar(VarKind::Rigid, NextTypeVarId++, {}, Name, {{}}));
  getContext().TVs->emplace(TV);
  return TV;
}

Type* Checker::createTypeVar() {
  auto TV = new Type(TVar(VarKind::Unification, NextTypeVarId++, {}));
  getContext().TVs->emplace(TV);
  return TV;
}

InferContext* Checker::createInferContext(InferContext* Parent, TVSet* TVs, ConstraintSet* Constraints) {
  auto Ctx = new InferContext;
  Ctx->Parent = Parent;
  Ctx->TVs = new TVSet;
  Ctx->Constraints = new ConstraintSet;
  return Ctx;
}

Type* Checker::instantiate(Scheme* Scm, Node* Source) {

  switch (Scm->getKind()) {

    case SchemeKind::Forall:
    {
      auto F = static_cast<Forall*>(Scm);

      TVSub Sub;
      for (auto TV: *F->TVs) {
        auto Fresh = createTypeVar();
        // std::cerr << describe(TV) << " => " << describe(Fresh) << std::endl;
        Fresh->asVar().Context = TV->asVar().Context;
        Sub[TV] = Fresh;
      }

      for (auto Constraint: *F->Constraints) {

        // FIXME improve this
        if (Constraint->getKind() == ConstraintKind::Equal) {
          auto Eq = static_cast<CEqual*>(Constraint);
          Eq->Left = solveType(Eq->Left);
          Eq->Right = solveType(Eq->Right);
        }

        auto NewConstraint = Constraint->substitute(Sub);

        // This makes error messages prettier by relating the typing failure
        // to the call site rather than the definition.
        if (NewConstraint->getKind() == ConstraintKind::Equal) {
          auto Eq = static_cast<CEqual*>(Constraint);
          Eq->Source = Source;
        }

        addConstraint(NewConstraint);
      }

      // This call to solve happens because constraints may have already
      // been solved, with some unification variables being erased. To make
      // sure we instantiate unification variables that are still in use
      // we solve before substituting.
      return solveType(F->Type)->substitute(Sub);
    }

  }

  ZEN_UNREACHABLE
}

void Checker::inferConstraintExpression(ConstraintExpression* C) {
  switch (C->getKind()) {
    case NodeKind::TypeclassConstraintExpression:
    {
      auto D = static_cast<TypeclassConstraintExpression*>(C);
      std::vector<Type*> Types;
      for (auto TE: D->TEs) {
        auto Ty = inferTypeExpression(TE);
        Ty->asVar().Provided->emplace(getCanonicalText(D->Name));
        Types.push_back(Ty);
      }
      break;
    }
    case NodeKind::EqualityConstraintExpression:
    {
      auto D = static_cast<EqualityConstraintExpression*>(C);
      makeEqual(inferTypeExpression(D->Left), inferTypeExpression(D->Right), C);
      break;
    }
    default:
      ZEN_UNREACHABLE
  }
}

Type* Checker::inferTypeExpression(TypeExpression* N, bool AutoVars) {

  switch (N->getKind()) {

    case NodeKind::ReferenceTypeExpression:
    {
      auto RefTE = static_cast<ReferenceTypeExpression*>(N);
      auto Scm = lookup(getCanonicalText(RefTE->Name), SymKind::Type);
      Type* Ty;
      if (Scm == nullptr) {
        DE.add<BindingNotFoundDiagnostic>(getCanonicalText(RefTE->Name), RefTE->Name);
        Ty = createTypeVar();
      } else {
        Ty = instantiate(Scm, RefTE);
      }
      N->setType(Ty);
      return Ty;
    }

    case NodeKind::AppTypeExpression:
    {
      auto AppTE = static_cast<AppTypeExpression*>(N);
      Type* Ty = inferTypeExpression(AppTE->Op, AutoVars);
      for (auto Arg: AppTE->Args) {
        Ty = new Type(TApp(Ty, inferTypeExpression(Arg, AutoVars)));
      }
      N->setType(Ty);
      return Ty;
    }

    case NodeKind::VarTypeExpression:
    {
      auto VarTE = static_cast<VarTypeExpression*>(N);
      auto Ty = lookupMono(getCanonicalText(VarTE->Name), SymKind::Type);
      if (Ty == nullptr) {
        if (!AutoVars || Config.typeVarsRequireForall()) {
          DE.add<BindingNotFoundDiagnostic>(getCanonicalText(VarTE->Name), VarTE->Name);
        }
        Ty = createRigidVar(getCanonicalText(VarTE->Name));
        addBinding(getCanonicalText(VarTE->Name), new Forall(Ty), SymKind::Type);
      }
      ZEN_ASSERT(Ty->isVar());
      N->setType(Ty);
      return Ty;
    }

    case NodeKind::RecordTypeExpression:
    {
      auto RecTE = static_cast<RecordTypeExpression*>(N);
      auto Ty = RecTE->Rest ? inferTypeExpression(RecTE->Rest, AutoVars) : new Type(TNil());
      for (auto [Field, Comma]: RecTE->Fields) {
        Ty = new Type(TField(getCanonicalText(Field->Name), new Type(TPresent(inferTypeExpression(Field->TE, AutoVars))), Ty));
      }
      N->setType(Ty);
      return Ty;
    }

    case NodeKind::TupleTypeExpression:
    {
      auto TupleTE = static_cast<TupleTypeExpression*>(N);
      std::vector<Type*> ElementTypes;
      for (auto [TE, Comma]: TupleTE->Elements) {
        ElementTypes.push_back(inferTypeExpression(TE, AutoVars));
      }
      auto Ty = new Type(TTuple(ElementTypes));
      N->setType(Ty);
      return Ty;
    }

    case NodeKind::NestedTypeExpression:
    {
      auto NestedTE = static_cast<NestedTypeExpression*>(N);
      auto Ty = inferTypeExpression(NestedTE->TE, AutoVars);
      N->setType(Ty);
      return Ty;
    }

    case NodeKind::ArrowTypeExpression:
    {
      auto ArrowTE = static_cast<ArrowTypeExpression*>(N);
      std::vector<Type*> ParamTypes;
      for (auto ParamType: ArrowTE->ParamTypes) {
        ParamTypes.push_back(inferTypeExpression(ParamType, AutoVars));
      }
      auto ReturnType = inferTypeExpression(ArrowTE->ReturnType, AutoVars);
      auto Ty = Type::buildArrow(ParamTypes, ReturnType);
      N->setType(Ty);
      return Ty;
    }

    case NodeKind::QualifiedTypeExpression:
    {
      auto QTE = static_cast<QualifiedTypeExpression*>(N);
      for (auto [C, Comma]: QTE->Constraints) {
        inferConstraintExpression(C);
      }
      auto Ty = inferTypeExpression(QTE->TE, AutoVars);
      N->setType(Ty);
      return Ty;
    }

    default:
      ZEN_UNREACHABLE

  }
}

Type* sortRow(Type* Ty) {
  std::map<ByteString, Type*> Fields;
  while (Ty->isField()) {
    auto& Field = Ty->asField();
    Fields.emplace(Field.Name, Ty);
    Ty = Field.RestTy;
  }
  for (auto [Name, Field]: Fields) {
    Ty = new Type(TField(Name, Field->asField().Ty, Ty));
  }
  return Ty;
}

Type* Checker::inferExpression(Expression* X) {

  Type* Ty;

  for (auto A: X->Annotations) {
    if (A->getKind() == NodeKind::TypeAssertAnnotation) {
      inferTypeExpression(static_cast<TypeAssertAnnotation*>(A)->TE);
    }
  }

  switch (X->getKind()) {

    case NodeKind::MatchExpression:
    {
      auto Match = static_cast<MatchExpression*>(X);
      Type* ValTy;
      if  (Match->Value) {
        ValTy = inferExpression(Match->Value);
      } else {
        ValTy = createTypeVar();
      }
      Ty = createTypeVar();
      for (auto Case: Match->Cases) {
        auto OldCtx = &getContext();
        setContext(Case->Ctx);
        auto PattTy = inferPattern(Case->Pattern);
        makeEqual(PattTy, ValTy, Case);
        auto ExprTy = inferExpression(Case->Expression);
        makeEqual(ExprTy, Ty, Case->Expression);
        setContext(OldCtx);
      }
      if (!Match->Value) {
        Ty = new Type(TArrow(ValTy, Ty));
      }
      break;
    }

    case NodeKind::RecordExpression:
    {
      auto Record = static_cast<RecordExpression*>(X);
      Ty = new Type(TNil());
      for (auto [Field, Comma]: Record->Fields) {
        Ty = new Type(TField(
          getCanonicalText(Field->Name),
          new Type(TPresent(inferExpression(Field->getExpression()))),
          Ty
        ));
      }
      Ty = sortRow(Ty);
      break;
    }

    case NodeKind::LiteralExpression:
    {
      auto Const = static_cast<LiteralExpression*>(X);
      Ty = inferLiteral(Const->Token);
      break;
    }

    case NodeKind::ReferenceExpression:
    {
      auto Ref = static_cast<ReferenceExpression*>(X);
      ZEN_ASSERT(Ref->ModulePath.empty());
      if (Ref->Name->is<IdentifierAlt>()) {
        auto Scm = lookup(getCanonicalText(Ref->Name), SymKind::Var);
        if (!Scm) {
          DE.add<BindingNotFoundDiagnostic>(getCanonicalText(Ref->Name), Ref->Name);
          Ty = createTypeVar();
          break;
        }
        Ty = instantiate(Scm, X);
        break;
      }
      auto Target = Ref->getScope()->lookup(Ref->getSymbolPath());
      if (!Target) {
        DE.add<BindingNotFoundDiagnostic>(getCanonicalText(Ref->Name), Ref->Name);
        Ty = createTypeVar();
        break;
      }
      if (Target->getKind() == NodeKind::LetDeclaration) {
        auto Let = static_cast<LetDeclaration*>(Target);
        if (Let->IsCycleActive) {
          Ty = Let->getType();
          break;
        }
        if (!Let->Visited) {
          infer(Let);
        }
      }
      auto Scm = lookup(getCanonicalText(Ref->Name), SymKind::Var);
      ZEN_ASSERT(Scm);
      Ty = instantiate(Scm, X);
      break;
    }

    case NodeKind::CallExpression:
    {
      auto Call = static_cast<CallExpression*>(X);
      auto OpTy = inferExpression(Call->Function);
      Ty = createTypeVar();
      std::vector<Type*> ArgTypes;
      for (auto Arg: Call->Args) {
        ArgTypes.push_back(inferExpression(Arg));
      }
      makeEqual(OpTy, Type::buildArrow(ArgTypes, Ty), X);
      break;
    }

    case NodeKind::InfixExpression:
    {
      auto Infix = static_cast<InfixExpression*>(X);
      auto Scm = lookup(Infix->Operator->getText(), SymKind::Var);
      if (Scm == nullptr) {
        DE.add<BindingNotFoundDiagnostic>(Infix->Operator->getText(), Infix->Operator);
        Ty = createTypeVar();
        break;
      }
      auto OpTy = instantiate(Scm, Infix->Operator);
      Ty = createTypeVar();
      std::vector<Type*> ArgTys;
      ArgTys.push_back(inferExpression(Infix->Left));
      ArgTys.push_back(inferExpression(Infix->Right));
      makeEqual(Type::buildArrow(ArgTys, Ty), OpTy, X);
      break;
    }

    case NodeKind::TupleExpression:
    {
      auto Tuple = static_cast<TupleExpression*>(X);
      std::vector<Type*> Types;
      for (auto [E, Comma]: Tuple->Elements) {
        Types.push_back(inferExpression(E));
      }
      Ty = new Type(TTuple(Types));
      break;
    }

    case NodeKind::MemberExpression:
    {
      auto Member = static_cast<MemberExpression*>(X);
      auto ExprTy = inferExpression(Member->E);
      switch (Member->Name->getKind()) {
        case NodeKind::IntegerLiteral:
        {
          auto I = static_cast<IntegerLiteral*>(Member->Name);
          Ty = createTypeVar();
          addConstraint(new CField(ExprTy, I->asInt(), Ty, Member));
          break;
        }
        case NodeKind::Identifier:
        {
          auto K = static_cast<Identifier*>(Member->Name);
          Ty = createTypeVar();
          auto RestTy = createTypeVar();
          makeEqual(new Type(TField(getCanonicalText(K), Ty, RestTy)), ExprTy, Member);
          break;
        }
        default:
          ZEN_UNREACHABLE
      }
      break;
    }

    case NodeKind::NestedExpression:
    {
      auto Nested = static_cast<NestedExpression*>(X);
      Ty = inferExpression(Nested->Inner);
      break;
    }

    default:
      ZEN_UNREACHABLE

  }

  // Ty = find(Ty);
  X->setType(Ty);
  return Ty;
}

RecordPatternField* getRestField(std::vector<std::tuple<RecordPatternField*, Comma*>> Fields) {
  for (auto [Field, Comma]: Fields) {
    if (Field->DotDot) {
      return Field;
    }
  }
  return nullptr;
}

Type* Checker::inferPattern(
  Pattern* Pattern,
  ConstraintSet* Constraints,
  TVSet* TVs
) {

  switch (Pattern->getKind()) {

    case NodeKind::BindPattern:
    {
      auto P = static_cast<BindPattern*>(Pattern);
      auto Ty = createTypeVar();
      addBinding(getCanonicalText(P->Name), new Forall(TVs, Constraints, Ty), SymKind::Var);
      return Ty;
    }

    case NodeKind::NamedTuplePattern:
    {
      auto P = static_cast<NamedTuplePattern*>(Pattern);
      auto Scm = lookup(getCanonicalText(P->Name), SymKind::Var);
      std::vector<Type*> ElementTypes;
      for (auto P2: P->Patterns) {
        ElementTypes.push_back(inferPattern(P2, Constraints, TVs));
      }
      if (!Scm) {
        DE.add<BindingNotFoundDiagnostic>(getCanonicalText(P->Name), P->Name);
        return createTypeVar();
      }
      auto Ty = instantiate(Scm, P);
      auto RetTy = createTypeVar();
      makeEqual(Ty, Type::buildArrow(ElementTypes, RetTy), P);
      return RetTy;
    }

    case NodeKind::RecordPattern:
    {
      auto P = static_cast<RecordPattern*>(Pattern);
      auto RestField = getRestField(P->Fields);
      Type* RecordTy;
      if (RestField == nullptr) {
        RecordTy = new Type(TNil());
      } else if (RestField->Pattern) {
        RecordTy = inferPattern(RestField->Pattern);
      } else {
        RecordTy = createTypeVar();
      }
      for (auto [Field, Comma]: P->Fields) {
        if (Field->DotDot) {
          continue;
        }
        Type* FieldTy;
        if (Field->Pattern) {
          FieldTy = inferPattern(Field->Pattern, Constraints, TVs);
        } else {
          FieldTy = createTypeVar();
          addBinding(getCanonicalText(Field->Name), new Forall(TVs, Constraints, FieldTy), SymKind::Var);
        }
        RecordTy = new Type(TField(getCanonicalText(Field->Name), new Type(TPresent(FieldTy)), RecordTy));
      }
      return RecordTy;
    }

    case NodeKind::NamedRecordPattern:
    {
      auto P = static_cast<NamedRecordPattern*>(Pattern);
      auto Scm = lookup(getCanonicalText(P->Name), SymKind::Var);
      if (Scm == nullptr) {
        DE.add<BindingNotFoundDiagnostic>(getCanonicalText(P->Name), P->Name);
        return createTypeVar();
      }
      auto RestField = getRestField(P->Fields);
      Type* RecordTy;
      if (RestField == nullptr) {
        RecordTy = new Type(TNil());
      } else if (RestField->Pattern) {
        RecordTy = inferPattern(RestField->Pattern);
      } else {
        RecordTy = createTypeVar();
      }
      for (auto [Field, Comma]: P->Fields) {
        if (Field->DotDot) {
          continue;
        }
        Type* FieldTy;
        if (Field->Pattern) {
          FieldTy = inferPattern(Field->Pattern, Constraints, TVs);
        } else {
          FieldTy = createTypeVar();
          addBinding(getCanonicalText(Field->Name), new Forall(TVs, Constraints, FieldTy), SymKind::Var);
        }
        RecordTy = new Type(TField(getCanonicalText(Field->Name), new Type(TPresent(FieldTy)), RecordTy));
      }
      auto Ty = instantiate(Scm, P);
      auto RetTy = createTypeVar();
      makeEqual(Ty, new Type(TArrow(RecordTy, RetTy)), P);
      return RetTy;
    }

    case NodeKind::TuplePattern:
    {
      auto P = static_cast<TuplePattern*>(Pattern);
      std::vector<Type*> ElementTypes;
      for (auto [Element, Comma]: P->Elements) {
        ElementTypes.push_back(inferPattern(Element));
      }
      return new Type(TTuple(ElementTypes));
    }

    case NodeKind::ListPattern:
    {
      auto P = static_cast<ListPattern*>(Pattern);
      auto ElementType = createTypeVar();
      for (auto [Element, Separator]: P->Elements) {
        makeEqual(ElementType, inferPattern(Element), P);
      }
      return new Type(TApp(ListType, ElementType));
    }

    case NodeKind::NestedPattern:
    {
      auto P = static_cast<NestedPattern*>(Pattern);
      return inferPattern(P->P, Constraints, TVs);
    }

    case NodeKind::LiteralPattern:
    {
      auto P = static_cast<LiteralPattern*>(Pattern);
      return inferLiteral(P->Literal);
    }

    default:
      ZEN_UNREACHABLE

  }

}

Type* Checker::inferLiteral(Literal* L) {
  Type* Ty;
  switch (L->getKind()) {
    case NodeKind::IntegerLiteral:
      Ty = lookupMono("Int", SymKind::Type);
      break;
    case NodeKind::StringLiteral:
      Ty = lookupMono("String", SymKind::Type);
      break;
    default:
      ZEN_UNREACHABLE
  }
  ZEN_ASSERT(Ty != nullptr);
  return Ty;
}

void Checker::populate(SourceFile* SF) {

  struct Visitor : public CSTVisitor<Visitor> {

    Graph<Node*>& RefGraph;

    std::stack<Node*> Stack;

    void visitLetDeclaration(LetDeclaration* N) {
      RefGraph.addVertex(N);
      Stack.push(N);
      visitEachChild(N);
      Stack.pop();
    }

    void visitReferenceExpression(ReferenceExpression* N) {
      auto Y = static_cast<ReferenceExpression*>(N);
      auto Def = Y->getScope()->lookup(Y->getSymbolPath());
      // Name lookup failures will be reported directly in inferExpression().
      if (Def == nullptr || Def->getKind() != NodeKind::LetDeclaration) {
        return;
      }
      // This case ensures that a deeply nested structure that references a
      // parameter of a parent node but is not referenced itself is correctly handled.
      // Note that the edge goes from the parent let to the parameter. This is normal.
      if (Def->getKind() == NodeKind::Parameter) {
        RefGraph.addEdge(Stack.top(), Def->Parent);
        return;
      }
      if (!Stack.empty()) {
        RefGraph.addEdge(Def, Stack.top());
      }
    }

  };

  Visitor V { {}, RefGraph };
  V.visit(SF);

}

Type* Checker::getType(TypedNode *Node) {
  auto Ty = Node->getType();
  if (Node->Flags & NodeFlags_TypeIsSolved) {
    return Ty;
  }
  Ty = solveType(Ty);
  Node->setType(Ty);
  Node->Flags |= NodeFlags_TypeIsSolved;
  return Ty;
}

void Checker::check(SourceFile *SF) {
  initialize(SF);
  setContext(SF->Ctx);
  addBinding("String", new Forall(StringType), SymKind::Type);
  addBinding("Int", new Forall(IntType), SymKind::Type);
  addBinding("Bool", new Forall(BoolType), SymKind::Type);
  addBinding("List", new Forall(ListType), SymKind::Type);
  addBinding("True", new Forall(BoolType), SymKind::Var);
  addBinding("False", new Forall(BoolType), SymKind::Var);
  auto A = createTypeVar();
  addBinding("==", new Forall(new TVSet { A }, new ConstraintSet, Type::buildArrow({ A, A }, BoolType)), SymKind::Var);
  addBinding("+", new Forall(Type::buildArrow({ IntType, IntType }, IntType)), SymKind::Var);
  addBinding("-", new Forall(Type::buildArrow({ IntType, IntType }, IntType)), SymKind::Var);
  addBinding("*", new Forall(Type::buildArrow({ IntType, IntType }, IntType)), SymKind::Var);
  addBinding("/", new Forall(Type::buildArrow({ IntType, IntType }, IntType)), SymKind::Var);
  populate(SF);
  forwardDeclare(SF);
  auto SCCs = RefGraph.strongconnect();
  for (auto Nodes: SCCs) {
    auto TVs = new TVSet;
    auto Constraints = new ConstraintSet;
    for (auto N: Nodes) {
      if (N->getKind() != NodeKind::LetDeclaration) {
        continue;
      }
      auto Decl = static_cast<LetDeclaration*>(N);
      forwardDeclareFunctionDeclaration(Decl, TVs, Constraints);
    }
  }
  setContext(SF->Ctx);
  infer(SF);

  // Important because otherwise some logic for some optimisations will kick in that are no longer active.
  ActiveContext = nullptr;

  solve(new CMany(*SF->Ctx->Constraints));

  class Visitor : public CSTVisitor<Visitor> {

    Checker& C;

  public:

    Visitor(Checker& C):
      C(C) {}

    void visitAnnotation(Annotation* A) {

    }

    void visitExpression(Expression* X) {
      C.getType(X);
    }

  } V(*this);

  V.visit(SF);
}

void Checker::solve(Constraint* Constraint) {

  Queue.push_back(Constraint);
  bool DidJoin = false;
  std::deque<class Constraint*> NextQueue;

  while (true) {

    if (Queue.empty()) {
      if (NextQueue.empty() || !DidJoin) {
        break;
      }
      DidJoin = false;
      std::swap(Queue, NextQueue);
    }

    auto Constraint = Queue.front();
    Queue.pop_front();

    switch (Constraint->getKind()) {

      case ConstraintKind::Empty:
        break;

      case ConstraintKind::Field:
      {
        auto Field = static_cast<CField*>(Constraint);
        auto MaybeTuple = Field->TupleTy->find();
        if (MaybeTuple->isTuple()) {
          auto& Tuple = MaybeTuple->asTuple();
          if (Field->I >= Tuple.ElementTypes.size()) {
            DE.add<TupleIndexOutOfRangeDiagnostic>(MaybeTuple, Field->I, Field->Source);
          } else {
            auto ElementTy = Tuple.ElementTypes[Field->I];
            unify(ElementTy, Field->FieldTy, Field->Source);
          }
        } else if (MaybeTuple->isVar()) {
            NextQueue.push_back(Constraint);
        } else {
          DE.add<NotATupleDiagnostic>(MaybeTuple, Field->Source);
        }
        break;
      }

      case ConstraintKind::Many:
      {
        auto Many = static_cast<CMany*>(Constraint);
        for (auto Constraint: Many->Elements) {
          Queue.push_back(Constraint);
        }
        break;
      }

      case ConstraintKind::Equal:
      {
        auto Equal = static_cast<CEqual*>(Constraint);
        if (unify(Equal->Left, Equal->Right, Equal->Source)) {
            DidJoin = true;
        }
        break;
      }

    }

  }

}

bool assignableTo(Type* A, Type* B) {
  if (A->isCon() && B->isCon()) {
    auto& Con1 = A->asCon();
    auto& Con2 = B->asCon();
    if (Con1.Id != Con2.Id) {
      return false;
    }
    return true;
  }
  // TODO must handle a TApp
  ZEN_UNREACHABLE
}

class ArrowCursor {

  /// Types on this stack are guaranteed to be arrow types.
  std::stack<std::tuple<Type*, bool>> Stack;

  TypePath& Path;
  std::size_t I;

public:

  ArrowCursor(Type* Arr, TypePath& Path):
    Path(Path) {
      Stack.push({ Arr, true });
      Path.push_back(Arr->getStartIndex());
    }

  Type* next() {
    while (!Stack.empty()) {
      auto& [Arrow, First] = Stack.top();
      auto& Index = Path.back();
      if (!First) {
        Index.advance(Arrow);
      } else {
        First = false;
      }
      Type* Ty;
      if (Index == Arrow->getEndIndex()) {
        Path.pop_back();
        Stack.pop();
        continue;
      }
      Ty = Arrow->resolve(Index);
      if (Ty->isArrow()) {
        auto NewIndex = Arrow->getStartIndex();
        Stack.push({ Ty, true });
        Path.push_back(NewIndex);
      } else {
        return Ty;
      }
    }
    return nullptr;
  }

};

struct Unifier {

  Checker& C;
  // CEqual* Constraint;
  Type* Left;
  Type* Right;
  Node* Source;

  // Internal state used by the unifier
  ByteString CurrentFieldName;
  TypePath LeftPath;
  TypePath RightPath;
  bool DidJoin = false;

  Type* getLeft() const {
    return Left;
  }

  Type* getRight() const {
    return Right;
  }

  Node* getSource() const {
    return Source;
  }

  bool unifyField(Type* A, Type* B, bool DidSwap);

  bool unify(Type* A, Type* B, bool DidSwap);

  bool unify() {
    return unify(Left, Right, false);
  }

  std::vector<TypeclassContext> findInstanceContext(const TypeSig& Ty, TypeclassId& Class) {
    auto Match = C.InstanceMap.find(Class);
    std::vector<TypeclassContext> S;
    if (Match != C.InstanceMap.end()) {
      for (auto Instance: Match->second) {
        if (assignableTo(Ty.Orig, Instance->TypeExps[0]->getType())) {
          std::vector<TypeclassContext> S;
          for (auto Arg: Ty.Args) {
            TypeclassContext Classes;
            // TODO
            S.push_back(Classes);
          }
          return S;
        }
      }
    }
    C.DE.add<InstanceNotFoundDiagnostic>(Class, Ty.Orig, getSource());
    for (auto Arg: Ty.Args) {
      S.push_back({});
    }
    return S;
  }

  TypeSig getTypeSig(Type* Ty) {
    Type* Op = nullptr;
    std::vector<Type*> Args;
    std::function<void(Type*)> Visit = [&](Type* Ty) {
      if (Ty->isApp()) {
        Visit(Ty->asApp().Op);
        Visit(Ty->asApp().Arg);
      } else if (!Op) {
        Op = Ty;
      } else {
        Args.push_back(Ty);
      }
    };
    Visit(Ty);
    return TypeSig { Ty, Op, Args };
  }

  void propagateClasses(std::unordered_set<TypeclassId>& Classes, Type* Ty) {
    if (Ty->isVar()) {
      auto TV = Ty->asVar();
      for (auto Class: Classes) {
        TV.Context.emplace(Class);
      }
      if (TV.isRigid()) {
        for (auto Id: TV.Context) {
          if (!TV.Provided->count(Id)) {
            C.DE.add<TypeclassMissingDiagnostic>(TypeclassSignature { Id, { Ty } }, getSource());
          }
        }
      }
    } else if (Ty->isCon() || Ty->isApp()) {
      auto Sig = getTypeSig(Ty);
      for (auto Class: Classes) {
        propagateClassTycon(Class, Sig);
      }
    } else if (!Classes.empty()) {
      C.DE.add<InvalidTypeToTypeclassDiagnostic>(Ty, std::vector(Classes.begin(), Classes.end()), getSource());
    }
  };

  void propagateClassTycon(TypeclassId& Class, const TypeSig& Sig) {
    auto S = findInstanceContext(Sig, Class);
    for (auto [Classes, Arg]: zen::zip(S, Sig.Args)) {
      propagateClasses(Classes, Arg);
    }
  };

  /**
   * Assign a type to a unification variable.
   *
   * If there are class constraints, those are propagated.
   *
   * If this type variable is solved during inference, it will be removed from
   * the inference context.
   *
   * Other side effects may occur.
   */
  void join(Type* TV, Type* Ty) {

    // std::cerr << describe(TV) << " => " << describe(Ty) << std::endl;

    TV->set(Ty);

    DidJoin = true;

    propagateClasses(TV->asVar().Context, Ty);

    // This is a very specific adjustment that is critical to the
    // well-functioning of the infer/unify algorithm. When addConstraint() is
    // called, it may decide to solve the constraint immediately during
    // inference. If this happens, a type variable might get assigned a concrete
    // type such as Int. We therefore never want the variable to be polymorphic
    // and be instantiated with a fresh variable, as that would allow Bool to
    // collide with Int.
    //
    // Should it get assigned another unification variable, that's OK too
    // because then that variable is what matters and it will become the new
    // (possibly polymorphic) variable.
    if (C.ActiveContext) {
      // std::cerr << "erase " << describe(TV) << std::endl;
      auto TVs = C.ActiveContext->TVs;
      TVs->erase(TV);
    }

  }

};

bool Unifier::unifyField(Type* A, Type* B, bool DidSwap) {
  if (A->isAbsent() && B->isAbsent()) {
    return true;
  }
  if (B->isAbsent()) {
    std::swap(A, B);
    DidSwap = !DidSwap;
  }
  if (A->isAbsent()) {
    auto& Present = B->asPresent();
    C.DE.add<FieldNotFoundDiagnostic>(CurrentFieldName, C.solveType(getLeft()), LeftPath, getSource());
    return false;
  }
  auto& Present1 = A->asPresent();
  auto& Present2 = B->asPresent();
  return unify(Present1.Ty, Present2.Ty, DidSwap);
};

bool Unifier::unify(Type* A, Type* B, bool DidSwap) {

  A = A->find();
  B = B->find();

  auto unifyError = [&]() {
    C.DE.add<UnificationErrorDiagnostic>(
      Left,
      Right,
      LeftPath,
      RightPath,
      Source
    );
  };

  auto pushLeft = [&](TypeIndex I) {
    if (DidSwap) {
      RightPath.push_back(I);
    } else {
      LeftPath.push_back(I);
    }
  };

  auto popLeft = [&]() {
    if (DidSwap) {
      RightPath.pop_back();
    } else {
      LeftPath.pop_back();
    }
  };

  auto pushRight = [&](TypeIndex I) {
    if (DidSwap) {
      LeftPath.push_back(I);
    } else {
      RightPath.push_back(I);
    }
  };

  auto popRight = [&]() {
    if (DidSwap) {
      LeftPath.pop_back();
    } else {
      RightPath.pop_back();
    }
  };

  auto swap = [&]() {
    std::swap(A, B);
    DidSwap = !DidSwap;
  };

  if (A->isVar() && B->isVar()) {
    auto& Var1 = A->asVar();
    auto& Var2 = B->asVar();
    if (Var1.isRigid() && Var2.isRigid()) {
      if (Var1.Id != Var2.Id) {
        unifyError();
        return false;
      }
      return true;
    }
    Type* To;
    Type* From;
    if (Var1.isRigid() && Var2.isUni()) {
      To = A;
      From = B;
    } else {
      // Only cases left are Var1 = Unification, Var2 = Rigid and Var1 = Unification, Var2 = Unification
      // Either way, Var1, being Unification, is a good candidate for being unified away
      To = B;
      From = A;
    }
    if (From->asVar().Id != To->asVar().Id) {
      join(From, To);
    }
    return true;
  }

  if (B->isVar()) {
    swap();
  }

  if (A->isVar()) {

    auto& TV = A->asVar();

    // Rigid type variables can never unify with antything else than what we
    // have already handled in the previous if-statement, so issue an error.
    if (TV.isRigid()) {
      unifyError();
      return false;
    }

    // Occurs check
    if (B->hasTypeVar(A)) {
      // NOTE Just like GHC, we just display an error message indicating that
      //      A cannot match B, e.g. a cannot match [a]. It looks much better
      //      than obsure references to an occurs check
      unifyError();
      return false;
    }

    join(A, B);

    return true;
  }

  if (A->isArrow() && B->isArrow()) {
    auto& Arrow1 = A->asArrow();
    auto& Arrow2 = B->asArrow();
    bool Success = true;
    LeftPath.push_back(TypeIndex::forArrowParamType());
    RightPath.push_back(TypeIndex::forArrowParamType());
    if (!unify(Arrow1.ParamType, Arrow2.ParamType, DidSwap)) {
      Success = false;
    }
    LeftPath.pop_back();
    RightPath.pop_back();
    LeftPath.push_back(TypeIndex::forArrowReturnType());
    RightPath.push_back(TypeIndex::forArrowReturnType());
    if (!unify(Arrow1.ReturnType, Arrow2.ReturnType, DidSwap)) {
      Success = false;
    }
    LeftPath.pop_back();
    RightPath.pop_back();
    return Success;
  }

  if (A->isApp() && B->isApp()) {
    auto& App1 = A->asApp();
    auto& App2 = B->asApp();
    bool Success = true;
    LeftPath.push_back(TypeIndex::forAppOpType());
    RightPath.push_back(TypeIndex::forAppOpType());
    if (!unify(App1.Op, App2.Op, DidSwap)) {
      Success = false;
    }
    LeftPath.pop_back();
    RightPath.pop_back();
    LeftPath.push_back(TypeIndex::forAppArgType());
    RightPath.push_back(TypeIndex::forAppArgType());
    if (!unify(App1.Arg, App2.Arg, DidSwap)) {
      Success = false;
    }
    LeftPath.pop_back();
    RightPath.pop_back();
    return Success;
  }

  if (A->isTuple() && B->isTuple()) {
    auto& Tuple1 = A->asTuple();
    auto& Tuple2 = B->asTuple();
    if (Tuple1.ElementTypes.size() != Tuple2.ElementTypes.size()) {
      unifyError();
      return false;
    }
    auto Count = Tuple1.ElementTypes.size();
    bool Success = true;
    for (size_t I = 0; I < Count; I++) {
      LeftPath.push_back(TypeIndex::forTupleElement(I));
      RightPath.push_back(TypeIndex::forTupleElement(I));
      if (!unify(Tuple1.ElementTypes[I], Tuple2.ElementTypes[I], DidSwap)) {
        Success = false;
      }
      LeftPath.pop_back();
      RightPath.pop_back();
    }
    return Success;
  }

  // if (A->isTupleIndex() || B->isTupleIndex()) {
  //   // Type(s) could not be simplified at the beginning of this function,
  //   // so we have to re-visit the constraint when there is more information.
  //   C.Queue.push_back(Constraint);
  //   return true;
  // }

  // This does not work because it ignores the indices
  // if (A->isTupleIndex() && B->isTupleIndex()) {
  //   auto Index1 = static_cast<TTupleIndex*>(A);  
  //   auto Index2 = static_cast<TTupleIndex*>(B);
  //   return unify(Index1->Ty, Index2->Ty, Source);
  // }

  if (A->isCon() && B->isCon()) {
    auto& Con1 = A->asCon();
    auto& Con2 = B->asCon();
    if (Con1.Id != Con2.Id) {
      unifyError();
      return false;
    }
    return true;
  }

  if (A->isNil() && B->isNil()) {
    return true;
  }

  if (A->isField() && B->isField()) {
    auto& Field1 = A->asField();
    auto& Field2 = B->asField();
    bool Success = true;
    if (Field1.Name == Field2.Name) {
      LeftPath.push_back(TypeIndex::forFieldType());
      RightPath.push_back(TypeIndex::forFieldType());
      CurrentFieldName = Field1.Name;
      if (!unifyField(Field1.Ty, Field2.Ty, DidSwap)) {
        Success = false;
      }
      LeftPath.pop_back();
      RightPath.pop_back();
      LeftPath.push_back(TypeIndex::forFieldRest());
      RightPath.push_back(TypeIndex::forFieldRest());
      if (!unify(Field1.RestTy, Field2.RestTy, DidSwap)) {
        Success = false;
      }
      LeftPath.pop_back();
      RightPath.pop_back();
      return Success;
    }
    auto NewRestTy = new Type(TVar(VarKind::Unification, C.NextTypeVarId++));
    pushLeft(TypeIndex::forFieldRest());
    if (!unify(Field1.RestTy, new Type(TField(Field2.Name, Field2.Ty, NewRestTy)), DidSwap)) {
      Success = false;
    }
    popLeft();
    pushRight(TypeIndex::forFieldRest());
    if (!unify(new Type(TField(Field1.Name, Field1.Ty, NewRestTy)), Field2.RestTy, DidSwap)) {
      Success = false;
    }
    popRight();
    return Success;
  }

  if (A->isNil() && B->isField()) {
    swap();
  }

  if (A->isField() && B->isNil()) {
    auto& Field = A->asField();
    bool Success = true;
    pushLeft(TypeIndex::forFieldType());
    CurrentFieldName = Field.Name;
    if (!unifyField(Field.Ty, new Type(TAbsent()), DidSwap)) {
      Success = false;
    }
    popLeft();
    pushLeft(TypeIndex::forFieldRest());
    if (!unify(Field.RestTy, B, DidSwap)) {
      Success = false;
    }
    popLeft();
    return Success;
  }

  unifyError();
  return false;
}

bool Checker::unify(Type* Left, Type* Right, Node* Source) {
  // std::cerr << describe(C->Left) << " ~ " << describe(C->Right) << std::endl;
  Unifier A { *this, Left, Right, Source };
  A.unify();
  return A.DidJoin;
}

}

