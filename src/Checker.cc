
// TODO create the constraint in addConstraint, not the other way round

#include <algorithm>
#include <iterator>
#include <stack>
#include <map>

#include "llvm/Support/Casting.h"

#include "bolt/Type.hpp"
#include "zen/config.hpp"
#include "zen/range.hpp"

#include "bolt/CSTVisitor.hpp"
#include "bolt/DiagnosticEngine.hpp"
#include "bolt/Diagnostics.hpp"
#include "bolt/CST.hpp"
#include "bolt/Checker.hpp"

namespace bolt {

  std::string describe(const Type* Ty);

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
      case ConstraintKind::Empty:
        return this;
    }
  }

  Type* Checker::simplifyType(Type* Ty) {

    return Ty->rewrite([&](auto Ty) {

      if (Ty->getKind() == TypeKind::Var) {
        Ty = static_cast<TVar*>(Ty)->find();
      }

      if (Ty->getKind() == TypeKind::TupleIndex) {
        auto Index = static_cast<TTupleIndex*>(Ty);
        auto MaybeTuple = simplifyType(Index->Ty);
        if (MaybeTuple->getKind() == TypeKind::Tuple) {
          auto Tuple = static_cast<TTuple*>(MaybeTuple);
          if (Index->I >= Tuple->ElementTypes.size()) {
            DE.add<TupleIndexOutOfRangeDiagnostic>(Tuple, Index->I);
          } else {
            Ty = simplifyType(Tuple->ElementTypes[Index->I]);
          }
        }
      }

      return Ty;

    }, /*Recursive=*/true);

  }

  Checker::Checker(const LanguageConfig& Config, DiagnosticEngine& DE):
    Config(Config), DE(DE) {
      BoolType = createConType("Bool");
      IntType = createConType("Int");
      StringType = createConType("String");
      ListType = createConType("List");
    }

  Scheme* Checker::lookup(ByteString Name) {
    auto Curr = &getContext();
    for (;;) {
      auto Match = Curr->Env.find(Name);
      if (Match != Curr->Env.end()) {
        return Match->second;
      }
      Curr = Curr->Parent;
      if (!Curr) {
        break;
      }
    }
    return nullptr;
  }

  Type* Checker::lookupMono(ByteString Name) {
    auto Scm = lookup(Name);
    if (Scm == nullptr) {
      return nullptr;
    }
    auto F = static_cast<Forall*>(Scm);
    ZEN_ASSERT(F->TVs == nullptr || F->TVs->empty());
    return F->Type;
  }

  void Checker::addBinding(ByteString Name, Scheme* Scm) {
    getContext().Env.emplace(Name, Scm);
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
          solveEqual(Y);
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

        auto Match = InstanceMap.find(Decl->Name->getCanonicalText());
        if (Match == InstanceMap.end()) {
          InstanceMap.emplace(Decl->Name->getCanonicalText(), std::vector { Decl });
        } else {
          Match->second.push_back(Decl);
        }

        for (auto Element: Decl->Elements) {
          forwardDeclare(Element);
        }

        break;
      }

      case NodeKind::FunctionDeclaration:
        // These declarations will be handled separately in check()
        break;

      case NodeKind::VariableDeclaration:
        // All of this node's semantics will be handled in infer()
        break;

      case NodeKind::VariantDeclaration:
      {
        auto Decl = static_cast<VariantDeclaration*>(X);

        setContext(Decl->Ctx);

        std::vector<TVar*> Vars;
        for (auto TE: Decl->TVs) {
          auto TV = createRigidVar(TE->Name->getCanonicalText());
          Decl->Ctx->TVs->emplace(TV);
          Vars.push_back(TV);
        }

        Type* Ty = createConType(Decl->Name->getCanonicalText());

        // Must be added early so we can create recursive types
        Decl->Ctx->Parent->Env.emplace(Decl->Name->getCanonicalText(), new Forall(Ty));

        for (auto Member: Decl->Members) {
          switch (Member->getKind()) {
            case NodeKind::TupleVariantDeclarationMember:
            {
              auto TupleMember = static_cast<TupleVariantDeclarationMember*>(Member);
              auto RetTy = Ty;
              for (auto Var: Vars) {
                RetTy = new TApp(RetTy, Var);
              }
              std::vector<Type*> ParamTypes;
              for (auto Element: TupleMember->Elements) {
                ParamTypes.push_back(inferTypeExpression(Element));
              }
              Decl->Ctx->Parent->Env.emplace(TupleMember->Name->getCanonicalText(), new Forall(Decl->Ctx->TVs, Decl->Ctx->Constraints, new TArrow(ParamTypes, RetTy)));
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

        std::vector<TVar*> Vars;
        for (auto TE: Decl->Vars) {
          auto TV = createRigidVar(TE->Name->getCanonicalText());
          Vars.push_back(TV);
        }

        auto Name = Decl->Name->getCanonicalText();
        auto Ty = createConType(Name);

        // Must be added early so we can create recursive types
        Decl->Ctx->Parent->Env.emplace(Name, new Forall(Ty));

        // Corresponds to the logic of one branch of a VariantDeclarationMember
        Type* FieldsTy = new TNil();
        for (auto Field: Decl->Fields) {
          FieldsTy = new TField(Field->Name->getCanonicalText(), new TPresent(inferTypeExpression(Field->TypeExpression)), FieldsTy);
        }
        Type* RetTy = Ty;
        for (auto TV: Vars) {
          RetTy = new TApp(RetTy, TV);
        }
        Decl->Ctx->Parent->Env.emplace(Name, new Forall(Decl->Ctx->TVs, Decl->Ctx->Constraints, new TArrow({ FieldsTy }, RetTy)));
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

      void visitFunctionDeclaration(FunctionDeclaration* Let) {
        Let->Ctx = createDerivedContext();
        Contexts.push(Let->Ctx);
        visitEachChild(Let);
        Contexts.pop();
      }

      // void visitVariableDeclaration(VariableDeclaration* Var) {
      //   Var->Ctx = Contexts.top();
      //   visitEachChild(Var);
      // }

    };

    Init I { {}, *this };
    I.visit(N);

  }

  void Checker::forwardDeclareFunctionDeclaration(FunctionDeclaration* Let, TVSet* TVs, ConstraintSet* Constraints) {

    setContext(Let->Ctx);

    auto addClassVars = [&](ClassDeclaration* Class, bool IsRigid) {
      auto Id = Class->Name->getCanonicalText();
      auto Ctx = &getContext();
      std::vector<TVar*> Out;
      for (auto TE: Class->TypeVars) {
        auto Name = TE->Name->getCanonicalText();
        auto TV = IsRigid ? createRigidVar(Name) : createTypeVar();
        TV->Contexts.emplace(Id);
        Ctx->Env.emplace(Name, new Forall(TV));
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
    Let->Ty = Ty;

    // If declaring a let-declaration inside a type instance declaration,
    // we need to perform some work to make sure the type asserts of the
    // corresponding let-declaration in the type class declaration are
    // accounted for.
    if (Let->isInstance()) {

      auto Instance = static_cast<InstanceDeclaration*>(Let->Parent);
      auto Class = llvm::cast<ClassDeclaration>(Instance->getScope()->lookup({ {}, Instance->Name->getCanonicalText() }, SymbolKind::Class));
      auto SigLet = llvm::cast<FunctionDeclaration>(Class->getScope()->lookupDirect({ {}, Let->Name->getCanonicalText() }, SymbolKind::Var));

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
      //   Sub.emplace(llvm::cast<TVar>(TE->getType()), TV);
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
      Let->Ctx->Parent->Env.emplace(Let->Name->getCanonicalText(), new Forall(Let->Ctx->TVs, Let->Ctx->Constraints, Ty));
    }

  }

  void Checker::inferFunctionDeclaration(FunctionDeclaration* Decl) {

    if (Decl->isSignature()) {
      return;
    }

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

    makeEqual(Decl->Ty, new TArrow(ParamTypes, RetType), Decl);

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

      case NodeKind::FunctionDeclaration:
        break;

      case NodeKind::ReturnStatement:
      {
        auto RetStmt = static_cast<ReturnStatement*>(N);
        Type* ReturnType;
        if (RetStmt->Expression) {
          makeEqual(inferExpression(RetStmt->Expression), getReturnType(), RetStmt->Expression);
        } else {
          ReturnType = new TTuple({});
          makeEqual(new TTuple({}), getReturnType(), N);
        }
        break;
      }

      case NodeKind::VariableDeclaration:
      {
        auto Decl = static_cast<VariableDeclaration*>(N);
        Type* Ty = nullptr;
        if (Decl->TypeAssert) {
          Ty = inferTypeExpression(Decl->TypeAssert->TypeExpression, false);
        }
        if (Decl->Body) {
          ZEN_ASSERT(Decl->Body->getKind() == NodeKind::LetExprBody);
          auto E = static_cast<LetExprBody*>(Decl->Body);
          auto Ty2 = inferExpression(E->Expression);
          if (Ty) {
            makeEqual(Ty, Ty2, Decl);
          } else {
            Ty = Ty2;
          }
        }
        auto Ty3 = inferPattern(Decl->Pattern);
        if (Ty) {
          makeEqual(Ty, Ty3, Decl);
        } else {
          Ty = Ty3;
        }
        Decl->setType(Ty);
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

  TCon* Checker::createConType(ByteString Name) {
    return new TCon(NextConTypeId++, Name);
  }

  TVarRigid* Checker::createRigidVar(ByteString Name) {
    auto TV = new TVarRigid(NextTypeVarId++, Name);
    getContext().TVs->emplace(TV);
    return TV;
  }

  TVar* Checker::createTypeVar() {
    auto TV = new TVar(NextTypeVarId++, VarKind::Unification);
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
          Fresh->Contexts = TV->Contexts;
          Sub[TV] = Fresh;
        }

        for (auto Constraint: *F->Constraints) {

          // FIXME improve this
          if (Constraint->getKind() == ConstraintKind::Equal) {
            auto Eq = static_cast<CEqual*>(Constraint);
            Eq->Left = simplifyType(Eq->Left);
            Eq->Right = simplifyType(Eq->Right);
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

        // Note the call to simplify? This is because constraints may have already
        // been solved, with some unification variables being erased. To make
        // sure we instantiate unification variables that are still in use
        // we solve before substituting.
        return simplifyType(F->Type)->substitute(Sub);
      }

    }

  }

  void Checker::inferConstraintExpression(ConstraintExpression* C) {
    switch (C->getKind()) {
      case NodeKind::TypeclassConstraintExpression:
      {
        auto D = static_cast<TypeclassConstraintExpression*>(C);
        std::vector<Type*> Types;
        for (auto TE: D->TEs) {
          auto TV = static_cast<TVarRigid*>(inferTypeExpression(TE));
          TV->Provided.emplace(D->Name->getCanonicalText());
          Types.push_back(TV);
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

  Type* Checker::inferTypeExpression(TypeExpression* N, bool IsPoly) {

    switch (N->getKind()) {

      case NodeKind::ReferenceTypeExpression:
      {
        auto RefTE = static_cast<ReferenceTypeExpression*>(N);
        auto Scm = lookup(RefTE->Name->getCanonicalText());
        Type* Ty;
        if (Scm == nullptr) {
          DE.add<BindingNotFoundDiagnostic>(RefTE->Name->getCanonicalText(), RefTE->Name);
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
        Type* Ty = inferTypeExpression(AppTE->Op, IsPoly);
        for (auto Arg: AppTE->Args) {
          Ty = new TApp(Ty, inferTypeExpression(Arg, IsPoly));
        }
        return Ty;
      }

      case NodeKind::VarTypeExpression:
      {
        auto VarTE = static_cast<VarTypeExpression*>(N);
        auto Ty = lookupMono(VarTE->Name->getCanonicalText());
        if (Ty == nullptr) {
          if (IsPoly && Config.typeVarsRequireForall()) {
            DE.add<BindingNotFoundDiagnostic>(VarTE->Name->getCanonicalText(), VarTE->Name);
          }
          Ty = IsPoly ? createRigidVar(VarTE->Name->getCanonicalText()) : createTypeVar();
          addBinding(VarTE->Name->getCanonicalText(), new Forall(Ty));
        }
        ZEN_ASSERT(Ty->getKind() == TypeKind::Var);
        N->setType(Ty);
        return static_cast<TVar*>(Ty);
      }

      case NodeKind::TupleTypeExpression:
      {
        auto TupleTE = static_cast<TupleTypeExpression*>(N);
        std::vector<Type*> ElementTypes;
        for (auto [TE, Comma]: TupleTE->Elements) {
          ElementTypes.push_back(inferTypeExpression(TE, IsPoly));
        }
        auto Ty = new TTuple(ElementTypes);
        N->setType(Ty);
        return Ty;
      }

      case NodeKind::NestedTypeExpression:
      {
        auto NestedTE = static_cast<NestedTypeExpression*>(N);
        auto Ty = inferTypeExpression(NestedTE->TE, IsPoly);
        N->setType(Ty);
        return Ty;
      }

      case NodeKind::ArrowTypeExpression:
      {
        auto ArrowTE = static_cast<ArrowTypeExpression*>(N);
        std::vector<Type*> ParamTypes;
        for (auto ParamType: ArrowTE->ParamTypes) {
          ParamTypes.push_back(inferTypeExpression(ParamType, IsPoly));
        }
        auto ReturnType = inferTypeExpression(ArrowTE->ReturnType, IsPoly);
        auto Ty = new TArrow(ParamTypes, ReturnType);
        N->setType(Ty);
        return Ty;
      }

      case NodeKind::QualifiedTypeExpression:
      {
        auto QTE = static_cast<QualifiedTypeExpression*>(N);
        for (auto [C, Comma]: QTE->Constraints) {
          inferConstraintExpression(C);
        }
        auto Ty = inferTypeExpression(QTE->TE, IsPoly);
        N->setType(Ty);
        return Ty;
      }

      default:
        ZEN_UNREACHABLE

    }
  }

  Type* sortRow(Type* Ty) {
    std::map<ByteString, TField*> Fields;
    while (Ty->getKind() == TypeKind::Field) {
      auto Field = static_cast<TField*>(Ty);
      Fields.emplace(Field->Name, Field);
      Ty = Field->RestTy;
    }
    for (auto [Name, Field]: Fields) {
      Ty = new TField(Name, Field->Ty, Ty);
    }
    return Ty;
  }

  Type* Checker::inferExpression(Expression* X) {

    Type* Ty;

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
          Ty = new TArrow({ ValTy }, Ty);
        }
        break;
      }

      case NodeKind::RecordExpression:
      {
        auto Record = static_cast<RecordExpression*>(X);
        Ty = new TNil();
        for (auto [Field, Comma]: Record->Fields) {
          Ty = new TField(Field->Name->getCanonicalText(), new TPresent(inferExpression(Field->getExpression())), Ty);
        }
        Ty = sortRow(Ty);
        break;
      }

      case NodeKind::ConstantExpression:
      {
        auto Const = static_cast<ConstantExpression*>(X);
        Ty = inferLiteral(Const->Token);
        break;
      }

      case NodeKind::ReferenceExpression:
      {
        auto Ref = static_cast<ReferenceExpression*>(X);
        ZEN_ASSERT(Ref->ModulePath.empty());
        auto Target = Ref->getScope()->lookup(Ref->getSymbolPath());
        if (Target && llvm::isa<FunctionDeclaration>(Target)) {
          auto Let = static_cast<FunctionDeclaration*>(Target);
          if (Let->IsCycleActive) {
            return Let->Ty;
          }
        }
        auto Scm = lookup(Ref->Name->getCanonicalText());
        if (Scm == nullptr) {
          DE.add<BindingNotFoundDiagnostic>(Ref->Name->getCanonicalText(), Ref->Name);
          return createTypeVar();
        }
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
        makeEqual(OpTy, new TArrow(ArgTypes, Ty), X);
        break;
      }

      case NodeKind::InfixExpression:
      {
        auto Infix = static_cast<InfixExpression*>(X);
        auto Scm = lookup(Infix->Operator->getText());
        if (Scm == nullptr) {
          DE.add<BindingNotFoundDiagnostic>(Infix->Operator->getText(), Infix->Operator);
          return createTypeVar();
        }
        auto OpTy = instantiate(Scm, Infix->Operator);
        Ty = createTypeVar();
        std::vector<Type*> ArgTys;
        ArgTys.push_back(inferExpression(Infix->LHS));
        ArgTys.push_back(inferExpression(Infix->RHS));
        makeEqual(new TArrow(ArgTys, Ty), OpTy, X);
        break;
      }

      case NodeKind::TupleExpression:
      {
        auto Tuple = static_cast<TupleExpression*>(X);
        std::vector<Type*> Types;
        for (auto [E, Comma]: Tuple->Elements) {
          Types.push_back(inferExpression(E));
        }
        Ty = new TTuple(Types);
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
            Ty = new TTupleIndex(ExprTy, I->getInteger());
            break;
          }
          case NodeKind::Identifier:
          {
            auto K = static_cast<Identifier*>(Member->Name);
            Ty = createTypeVar();
            auto RestTy = createTypeVar();
            makeEqual(new TField(K->getCanonicalText(), Ty, RestTy), ExprTy, Member);
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
        addBinding(P->Name->getCanonicalText(), new Forall(TVs, Constraints, Ty));
        return Ty;
      }

      case NodeKind::NamedPattern:
      {
        auto P = static_cast<NamedPattern*>(Pattern);
        auto Scm = lookup(P->Name->getCanonicalText());
        std::vector<Type*> ParamTypes;
        for (auto P2: P->Patterns) {
          ParamTypes.push_back(inferPattern(P2, Constraints, TVs));
        }
        if (!Scm) {
          DE.add<BindingNotFoundDiagnostic>(P->Name->getCanonicalText(), P->Name);
          return createTypeVar();
        }
        auto Ty = instantiate(Scm, P);
        auto RetTy = createTypeVar();
        makeEqual(Ty, new TArrow(ParamTypes, RetTy), P);
        return RetTy;
      }

      case NodeKind::TuplePattern:
      {
        auto P = static_cast<TuplePattern*>(Pattern);
        std::vector<Type*> ElementTypes;
        for (auto [Element, Comma]: P->Elements) {
          ElementTypes.push_back(inferPattern(Element));
        }
        return new TTuple(ElementTypes);
      }

      case NodeKind::ListPattern:
      {
        auto P = static_cast<ListPattern*>(Pattern);
        auto ElementType = createTypeVar();
        for (auto [Element, Separator]: P->Elements) {
          makeEqual(ElementType, inferPattern(Element), P);
        }
        return new TApp(ListType, ElementType);
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
        Ty = lookupMono("Int");
        break;
      case NodeKind::StringLiteral:
        Ty = lookupMono("String");
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

      void visitFunctionDeclaration(FunctionDeclaration* N) {
        RefGraph.addVertex(N);
        Stack.push(N);
        visitEachChild(N);
        Stack.pop();
      }

      void visitReferenceExpression(ReferenceExpression* N) {
        auto Y = static_cast<ReferenceExpression*>(N);
        auto Def = Y->getScope()->lookup(Y->getSymbolPath());
        // Name lookup failures will be reported directly in inferExpression().
        if (Def == nullptr || Def->getKind() == NodeKind::SourceFile) {
          return;
        }
        // This case ensures that a deeply nested structure that references a
        // parameter of a parent node but is not referenced itself is correctly handled.
        // Note that the edge goes from the parent let to the parameter. This is normal.
        if (Def->getKind() == NodeKind::Parameter) {
          RefGraph.addEdge(Stack.top(), Def->Parent);
          return;
        }
        ZEN_ASSERT(Def->getKind() == NodeKind::FunctionDeclaration || Def->getKind() == NodeKind::VariableDeclaration);
        if (!Stack.empty()) {
          RefGraph.addEdge(Def, Stack.top());
        }
      }

    };

    Visitor V { {}, RefGraph };
    V.visit(SF);

  }

  Type* Checker::getType(TypedNode *Node) {
    return Node->getType()->solve();
  }

  void Checker::check(SourceFile *SF) {
    initialize(SF);
    setContext(SF->Ctx);
    addBinding("String", new Forall(StringType));
    addBinding("Int", new Forall(IntType));
    addBinding("Bool", new Forall(BoolType));
    addBinding("List", new Forall(ListType));
    addBinding("True", new Forall(BoolType));
    addBinding("False", new Forall(BoolType));
    auto A = createTypeVar();
    addBinding("==", new Forall(new TVSet { A }, new ConstraintSet, new TArrow({ A, A }, BoolType)));
    addBinding("+", new Forall(new TArrow({ IntType, IntType }, IntType)));
    addBinding("-", new Forall(new TArrow({ IntType, IntType }, IntType)));
    addBinding("*", new Forall(new TArrow({ IntType, IntType }, IntType)));
    addBinding("/", new Forall(new TArrow({ IntType, IntType }, IntType)));
    populate(SF);
    forwardDeclare(SF);
    auto SCCs = RefGraph.strongconnect();
    for (auto Nodes: SCCs) {
      auto TVs = new TVSet;
      auto Constraints = new ConstraintSet;
      for (auto N: Nodes) {
        if (N->getKind() != NodeKind::FunctionDeclaration) {
          continue;
        }
        auto Decl = static_cast<FunctionDeclaration*>(N);
        forwardDeclareFunctionDeclaration(Decl, TVs, Constraints);
      }
    }
    for (auto Nodes: SCCs) {
      for (auto N: Nodes) {
        if (N->getKind() != NodeKind::FunctionDeclaration) {
          continue;
        }
        auto Decl = static_cast<FunctionDeclaration*>(N);
        Decl->IsCycleActive = true;
      }
      for (auto N: Nodes) {
        if (N->getKind() != NodeKind::FunctionDeclaration) {
          continue;
        }
        auto Decl = static_cast<FunctionDeclaration*>(N);
        inferFunctionDeclaration(Decl);
      }
      for (auto N: Nodes) {
        if (N->getKind() != NodeKind::FunctionDeclaration) {
          continue;
        }
        auto Decl = static_cast<FunctionDeclaration*>(N);
        Decl->IsCycleActive = false;
      }
    }
    setContext(SF->Ctx);
    infer(SF);

    // Important because otherwise some logic for some optimisations will kick in that are no longer active.
    ActiveContext = nullptr;

    solve(new CMany(*SF->Ctx->Constraints));
  }

  void Checker::solve(Constraint* Constraint) {

    Queue.push_back(Constraint);

    while (!Queue.empty()) {

      auto Constraint = Queue.front();
      Queue.pop_front();

      switch (Constraint->getKind()) {

        case ConstraintKind::Empty:
          break;

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
          solveEqual(static_cast<CEqual*>(Constraint));
          break;
        }

      }

    }

  }

  bool assignableTo(Type* A, Type* B) {
    if (llvm::isa<TCon>(A) && llvm::isa<TCon>(B)) {
      auto Con1 = llvm::cast<TCon>(A);
      auto Con2 = llvm::cast<TCon>(B);
      if (Con1->Id != Con2-> Id) {
        return false;
      }
      return true;
    }
    // TODO must handle a TApp
    ZEN_UNREACHABLE
  }

  class ArrowCursor {

    std::stack<std::tuple<TArrow*, bool>> Stack;
    TypePath& Path;
    std::size_t I;

  public:

    ArrowCursor(TArrow* Arr, TypePath& Path):
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
        if (llvm::isa<TArrow>(Ty)) {
          auto NewIndex = Arrow->getStartIndex();
          Stack.push({ static_cast<TArrow*>(Ty), true });
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
    CEqual* Constraint;

    // Internal state used by the unifier
    ByteString CurrentFieldName;
    TypePath LeftPath;
    TypePath RightPath;

    Type* getLeft() const {
      return Constraint->Left;
    }

    Type* getRight() const {
      return Constraint->Right;
    }

    Node* getSource() const {
      return Constraint->Source;
    }

    bool unifyField(Type* A, Type* B, bool DidSwap);

    bool unify(Type* A, Type* B, bool DidSwap);

    bool unify() {
      return unify(Constraint->Left, Constraint->Right, false);
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
      struct Visitor : TypeVisitor {
        Type* Op = nullptr;
        std::vector<Type*> Args;
        void visitType(Type* Ty) override {
          if (!Op) {
            Op = Ty;
          } else {
            Args.push_back(Ty);
          }
        }
        void visitAppType(TApp* Ty) override {
          visitEachChild(Ty);
        }
      };
      Visitor V;
      V.visit(Ty);
      return TypeSig { Ty, V.Op, V.Args };
    }

    void propagateClasses(std::unordered_set<TypeclassId>& Classes, Type* Ty) {
      if (llvm::isa<TVar>(Ty)) {
        auto TV = llvm::cast<TVar>(Ty);
        for (auto Class: Classes) {
          TV->Contexts.emplace(Class);
        }
        if (TV->isRigid()) {
          auto RV = static_cast<TVarRigid*>(Ty);
          for (auto Id: RV->Contexts) {
            if (!RV->Provided.count(Id)) {
              C.DE.add<TypeclassMissingDiagnostic>(TypeclassSignature { Id, { RV } }, getSource());
            }
          }
        }
      } else if (llvm::isa<TCon>(Ty) || llvm::isa<TApp>(Ty)) {
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
    void join(TVar* TV, Type* Ty) {

      // std::cerr << describe(TV) << " => " << describe(Ty) << std::endl;

      TV->set(Ty);

      propagateClasses(TV->Contexts, Ty);

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
    if (llvm::isa<TAbsent>(A) && llvm::isa<TAbsent>(B)) {
      return true;
    }
    if (llvm::isa<TAbsent>(B)) {
      std::swap(A, B);
      DidSwap = !DidSwap;
    }
    if (llvm::isa<TAbsent>(A)) {
      auto Present = static_cast<TPresent*>(B);
      C.DE.add<FieldNotFoundDiagnostic>(CurrentFieldName, C.simplifyType(getLeft()), LeftPath, getSource());
      return false;
    }
    auto Present1 = static_cast<TPresent*>(A);
    auto Present2 = static_cast<TPresent*>(B);
    return unify(Present1->Ty, Present2->Ty, DidSwap);
  };

  bool Unifier::unify(Type* A, Type* B, bool DidSwap) {

    A = C.simplifyType(A);
    B = C.simplifyType(B);

    auto unifyError = [&]() {
      C.DE.add<UnificationErrorDiagnostic>(
        C.simplifyType(Constraint->Left),
        C.simplifyType(Constraint->Right),
        LeftPath,
        RightPath,
        Constraint->Source
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

    if (llvm::isa<TVar>(A) && llvm::isa<TVar>(B)) {
      auto Var1 = static_cast<TVar*>(A);
      auto Var2 = static_cast<TVar*>(B);
      if (Var1->getVarKind() == VarKind::Rigid && Var2->getVarKind() == VarKind::Rigid) {
        if (Var1->Id != Var2->Id) {
          unifyError();
          return false;
        }
        return true;
      }
      TVar* To;
      TVar* From;
      if (Var1->getVarKind() == VarKind::Rigid && Var2->getVarKind() == VarKind::Unification) {
        To = Var1;
        From = Var2;
      } else {
        // Only cases left are Var1 = Unification, Var2 = Rigid and Var1 = Unification, Var2 = Unification
        // Either way, Var1, being Unification, is a good candidate for being unified away
        To = Var2;
        From = Var1;
      }
      if (From->Id != To->Id) {
        join(From, To);
      }
      return true;
    }

    if (llvm::isa<TVar>(B)) {
      swap();
    }

    if (llvm::isa<TVar>(A)) {

      auto TV = static_cast<TVar*>(A);

      // Rigid type variables can never unify with antything else than what we
      // have already handled in the previous if-statement, so issue an error.
      if (TV->getVarKind() == VarKind::Rigid) {
        unifyError();
        return false;
      }

      // Occurs check
      if (B->hasTypeVar(TV)) {
        // NOTE Just like GHC, we just display an error message indicating that
        //      A cannot match B, e.g. a cannot match [a]. It looks much better
        //      than obsure references to an occurs check
        unifyError();
        return false;
      }

      join(TV, B);

      return true;
    }

    if (llvm::isa<TArrow>(A) && llvm::isa<TArrow>(B)) {
      auto C1 = ArrowCursor(static_cast<TArrow*>(A), DidSwap ? RightPath : LeftPath);
      auto C2 = ArrowCursor(static_cast<TArrow*>(B), DidSwap ? LeftPath : RightPath);
      bool Success = true;
      for (;;) {
        auto T1 = C1.next();
        auto T2 = C2.next();
        if (T1 == nullptr && T2 == nullptr) {
          break;
        }
        if (T1 == nullptr || T2 == nullptr) {
          unifyError();
          Success = false;
          break;
        }
        if (!unify(T1, T2, DidSwap)) {
          Success = false;
        }
      }
      return Success;
      /* if (Arr1->ParamTypes.size() != Arr2->ParamTypes.size()) { */
      /*   return false; */
      /* } */
      /* auto Count = Arr1->ParamTypes.size(); */
      /* for (std::size_t I = 0; I < Count; I++) { */
      /*   if (!unify(Arr1->ParamTypes[I], Arr2->ParamTypes[I], Solution)) { */
      /*     return false; */
      /*   } */
      /* } */
      /* return unify(Arr1->ReturnType, Arr2->ReturnType, Solution); */
    }

    if (llvm::isa<TApp>(A) && llvm::isa<TApp>(B)) {
      auto App1 = static_cast<TApp*>(A);
      auto App2 = static_cast<TApp*>(B);
      bool Success = true;
      if (!unify(App1->Op, App2->Op, DidSwap)) {
        Success = false;
      }
      if (!unify(App1->Arg, App2->Arg, DidSwap)) {
        Success = false;
      }
      return Success;
    }

    if (llvm::isa<TArrow>(B)) {
      swap();
    }

    if (llvm::isa<TArrow>(A)) {
      auto Arr = static_cast<TArrow*>(A);
      if (Arr->ParamTypes.empty()) {
        auto Success = unify(Arr->ReturnType, B, DidSwap);
        return Success;
      }
    }

    if (llvm::isa<TTuple>(A) && llvm::isa<TTuple>(B)) {
      auto Tuple1 = static_cast<TTuple*>(A);
      auto Tuple2 = static_cast<TTuple*>(B);
      if (Tuple1->ElementTypes.size() != Tuple2->ElementTypes.size()) {
        unifyError();
        return false;
      }
      auto Count = Tuple1->ElementTypes.size();
      bool Success = true;
      for (size_t I = 0; I < Count; I++) {
        LeftPath.push_back(TypeIndex::forTupleElement(I));
        RightPath.push_back(TypeIndex::forTupleElement(I));
        if (!unify(Tuple1->ElementTypes[I], Tuple2->ElementTypes[I], DidSwap)) {
          Success = false;
        }
        LeftPath.pop_back();
        RightPath.pop_back();
      }
      return Success;
    }

    if (llvm::isa<TTupleIndex>(A) || llvm::isa<TTupleIndex>(B)) {
      // Type(s) could not be simplified at the beginning of this function,
      // so we have to re-visit the constraint when there is more information.
      C.Queue.push_back(Constraint);
      return true;
    }

    // if (llvm::isa<TTupleIndex>(A) && llvm::isa<TTupleIndex>(B)) {
    //   auto Index1 = static_cast<TTupleIndex*>(A);  
    //   auto Index2 = static_cast<TTupleIndex*>(B);
    //   return unify(Index1->Ty, Index2->Ty, Source);
    // }

    if (llvm::isa<TCon>(A) && llvm::isa<TCon>(B)) {
      auto Con1 = static_cast<TCon*>(A);
      auto Con2 = static_cast<TCon*>(B);
      if (Con1->Id != Con2->Id) {
        unifyError();
        return false;
      }
      return true;
    }

    if (llvm::isa<TNil>(A) && llvm::isa<TNil>(B)) {
      return true;
    }

    if (llvm::isa<TField>(A) && llvm::isa<TField>(B)) {
      auto Field1 = static_cast<TField*>(A);
      auto Field2 = static_cast<TField*>(B);
      bool Success = true;
      if (Field1->Name == Field2->Name) {
        LeftPath.push_back(TypeIndex::forFieldType());
        RightPath.push_back(TypeIndex::forFieldType());
        CurrentFieldName = Field1->Name;
        if (!unifyField(Field1->Ty, Field2->Ty, DidSwap)) {
          Success = false;
        }
        LeftPath.pop_back();
        RightPath.pop_back();
        LeftPath.push_back(TypeIndex::forFieldRest());
        RightPath.push_back(TypeIndex::forFieldRest());
        if (!unify(Field1->RestTy, Field2->RestTy, DidSwap)) {
          Success = false;
        }
        LeftPath.pop_back();
        RightPath.pop_back();
        return Success;
      }
      auto NewRestTy = new TVar(C.NextTypeVarId++, VarKind::Unification);
      pushLeft(TypeIndex::forFieldRest());
      if (!unify(Field1->RestTy, new TField(Field2->Name, Field2->Ty, NewRestTy), DidSwap)) {
        Success = false;
      }
      popLeft();
      pushRight(TypeIndex::forFieldRest());
      if (!unify(new TField(Field1->Name, Field1->Ty, NewRestTy), Field2->RestTy, DidSwap)) {
        Success = false;
      }
      popRight();
      return Success;
    }

    if (llvm::isa<TNil>(A) && llvm::isa<TField>(B)) {
      swap();
    }

    if (llvm::isa<TField>(A) && llvm::isa<TNil>(B)) {
      auto Field = static_cast<TField*>(A);
      bool Success = true;
      pushLeft(TypeIndex::forFieldType());
      CurrentFieldName = Field->Name;
      if (!unifyField(Field->Ty, new TAbsent, DidSwap)) {
        Success = false;
      }
      popLeft();
      pushLeft(TypeIndex::forFieldRest());
      if (!unify(Field->RestTy, B, DidSwap)) {
        Success = false;
      }
      popLeft();
      return Success;
    }

    unifyError();
    return false;
  }

  void Checker::solveEqual(CEqual* C) {
    // std::cerr << describe(C->Left) << " ~ " << describe(C->Right) << std::endl;
    Unifier A { *this, C };
    A.unify();
  }


}

