
// TODO Add list of CST variable names to TVar and unify them so that e.g. the typeclass checker may pick one when displaying a diagnostic

// TODO (maybe) make unficiation work like union-find in find()

// TODO make simplify() rewrite the types in-place such that a reference too (Bool, Int).0 becomes Bool

// TODO Fix TVSub to use TVar.Id instead of the pointer address

// TODO Deferred diagnostics

#include <algorithm>
#include <iterator>
#include <stack>

#include "llvm/Support/Casting.h"

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
      case ConstraintKind::Class:
      {
        auto Class = static_cast<CClass*>(this);
        std::vector<Type*> NewTypes;
        for (auto Ty: Class->Types) {
          NewTypes.push_back(Ty->substitute(Sub));
        }
        return new CClass(Class->Name, NewTypes);
      }
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

  Checker::Checker(const LanguageConfig& Config, DiagnosticEngine& DE):
    Config(Config), DE(DE) {
      BoolType = new TCon(NextConTypeId++, {}, "Bool");
      IntType = new TCon(NextConTypeId++, {}, "Int");
      StringType = new TCon(NextConTypeId++, {}, "String");
    }

  Scheme* Checker::lookup(ByteString Name) {
    for (auto Iter = Contexts.rbegin(); Iter != Contexts.rend(); Iter++) {
      auto Curr = *Iter;
      auto Match = Curr->Env.find(Name);
      if (Match != Curr->Env.end()) {
        return Match->second;
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
    Contexts.back()->Env.emplace(Name, Scm);
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

  InferContext& Checker::getContext() {
    ZEN_ASSERT(!Contexts.empty());
    return *Contexts.back();
  }

  void Checker::addConstraint(Constraint* C) {
    switch (C->getKind()) {
      case ConstraintKind::Class:
      {
        Contexts.back()->Constraints->push_back(C);
        break;
      }
      case ConstraintKind::Equal:
      {
        auto Y = static_cast<CEqual*>(C);

        std::size_t MaxLevelLeft = 0;
        for (std::size_t I = Contexts.size(); I-- > 0; ) {
          auto Ctx = Contexts[I];
          if (hasTypeVar(*Ctx->TVs, Y->Left)) {
            MaxLevelLeft = I;
            break;
          }
        }
        std::size_t MaxLevelRight = 0;
        for (std::size_t I = Contexts.size(); I-- > 0; ) {
          auto Ctx = Contexts[I];
          if (hasTypeVar(*Ctx->TVs, Y->Right)) {
            MaxLevelRight = I;
            break;
          }
        }
        auto MaxLevel = std::max(MaxLevelLeft, MaxLevelRight);

        std::size_t MinLevel = MaxLevel;
        for (std::size_t I = 0; I < Contexts.size(); I++) {
          auto Ctx = Contexts[I];
          if (hasTypeVar(*Ctx->TVs, Y->Left) || hasTypeVar(*Ctx->TVs, Y->Right)) {
            MinLevel = I;
            break;
          }
        }

        // TODO detect if MaxLevelLeft == 0 or MaxLevelRight == 0
        if (MaxLevel == MinLevel || MaxLevelLeft == 0 || MaxLevelRight == 0) {
          solveCEqual(Y);
        } else {
          Contexts[MaxLevel]->Constraints->push_back(C);
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

  void Checker::addClass(TypeclassSignature Sig) {
    getContext().Classes.push_back(Sig);
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
        for (auto TE: Class->TypeVars) {
          auto TV = new TVarRigid(NextTypeVarId++, TE->Name->getCanonicalText());
          TV->Contexts.emplace(Class->Name->getCanonicalText());
          TE->setType(TV);
        }
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

      case NodeKind::LetDeclaration:
        // These declarations will be handled separately in check()
        break;

      default:
        ZEN_UNREACHABLE

    }

  }

  void Checker::forwardDeclareLetDeclaration(LetDeclaration* N, TVSet* TVs, ConstraintSet* Constraints) {

    auto Let = static_cast<LetDeclaration*>(N);
    bool IsFunc = !Let->Params.empty();
    bool IsInstance = llvm::isa<InstanceDeclaration>(Let->Parent);
    bool IsClass = llvm::isa<ClassDeclaration>(Let->Parent);
    bool HasContext = IsFunc || IsInstance || IsClass;

    if (HasContext) {
      Let->Ctx = createInferContext(TVs, Constraints);
      Contexts.push_back(Let->Ctx);
    }

    // If declaring a let-declaration inside a type class declaration,
    // we need to mark that the let-declaration requires this class.
    // This marking is set on the rigid type variables of the class, which
    // are then added to this local type environment.
    if (IsClass) {
      auto Class = static_cast<ClassDeclaration*>(Let->Parent);
      for (auto TE: Class->TypeVars) {
        auto TV = llvm::cast<TVar>(TE->getType());
        Let->Ctx->Env.emplace(TE->Name->getCanonicalText(), new Forall(TV));
        Let->Ctx->TVs->emplace(TV);
      }
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
    if (IsInstance) {

      auto Instance = static_cast<InstanceDeclaration*>(Let->Parent);
      auto Class = llvm::cast<ClassDeclaration>(Instance->getScope()->lookup({ {}, Instance->Name->getCanonicalText() }, SymbolKind::Class));

      // The type asserts in the type class declaration might make use of
      // the type parameters of the type class declaration, so it is
      // important to make them available in the type environment. Moreover,
      // we will be unifying them with the actual types declared in the
      // instance declaration, so we keep track of them.
      std::vector<TVar *> Params;
      TVSub Sub;
      for (auto TE: Class->TypeVars) {
        auto TV = createTypeVar();
        Sub.emplace(llvm::cast<TVar>(TE->getType()), TV);
        Params.push_back(TV);
      }

      auto SigLet = llvm::cast<LetDeclaration>(Class->getScope()->lookupDirect({ {}, llvm::cast<BindPattern>(Let->Pattern)->Name->getCanonicalText() }, SymbolKind::Var));

      // It would be very strange if there was no type assert in the type
      // class let-declaration but we rather not let the compiler crash if that happens.
      if (SigLet->TypeAssert) {
        addConstraint(new CEqual(Ty, inferTypeExpression(SigLet->TypeAssert->TypeExpression)->substitute(Sub), Let));
      }

      // Here we do the actual unification of e.g. Eq a with Eq Bool. The
      // unification variables we created previously will be unified with
      // e.g. Bool, which causes the type assert to also collapse to e.g.
      // Bool -> Bool -> Bool.
      for (auto [Param, TE] : zen::zip(Params, Instance->TypeExps)) {
        addConstraint(new CEqual(Param, TE->getType()));
      }

    }

    if (Let->Body) {
      switch (Let->Body->getKind()) {
        case NodeKind::LetExprBody:
          break;
        case NodeKind::LetBlockBody:
        {
          auto Block = static_cast<LetBlockBody*>(Let->Body);
          if (IsFunc) {
            Let->Ctx->ReturnType = createTypeVar();
          }
          for (auto Element: Block->Elements) {
            forwardDeclare(Element);
          }
          break;
        }
        default:
          ZEN_UNREACHABLE
      }
    }

    if (HasContext) {
      Contexts.pop_back();
      inferBindings(Let->Pattern, Ty, Let->Ctx->Constraints, Let->Ctx->TVs);
    } else {
      inferBindings(Let->Pattern, Ty);
    }

  }

  void Checker::inferLetDeclaration(LetDeclaration* N) {

    auto Decl = static_cast<LetDeclaration*>(N);
    bool IsFunc = !Decl->Params.empty();
    bool IsInstance = llvm::isa<InstanceDeclaration>(Decl->Parent);
    bool IsClass = llvm::isa<ClassDeclaration>(Decl->Parent);
    bool HasContext = IsFunc || IsInstance || IsClass;

    if (HasContext) {
      Contexts.push_back(Decl->Ctx);
    }

    std::vector<Type*> ParamTypes;
    Type* RetType;

    for (auto Param: Decl->Params) {
      // TODO incorporate Param->TypeAssert or make it a kind of pattern
      TVar* TV = createTypeVar();
      inferBindings(Param->Pattern, TV);
      ParamTypes.push_back(TV);
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
          ZEN_ASSERT(HasContext);
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

    if (HasContext) {
      Contexts.pop_back();
    }

    if (IsFunc) {
      addConstraint(new CEqual { Decl->Ty, new TArrow(ParamTypes, RetType), N });
    } else {
      // Declaration is a plain (typed) variable
      addConstraint(new CEqual { Decl->Ty, RetType, N });
    }

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

      case NodeKind::IfStatement:
      {
        auto IfStmt = static_cast<IfStatement*>(N);
        for (auto Part: IfStmt->Parts) {
          if (Part->Test != nullptr) {
            addConstraint(new CEqual { BoolType, inferExpression(Part->Test), Part->Test });
          }
          for (auto Element: Part->Elements) {
            infer(Element);
          }
        }
        break;
      }

      case NodeKind::LetDeclaration:
        break;

      case NodeKind::ReturnStatement:
      {
        auto RetStmt = static_cast<ReturnStatement*>(N);
        Type* ReturnType;
        if (RetStmt->Expression) {
          addConstraint(new CEqual { inferExpression(RetStmt->Expression), getReturnType(), RetStmt->Expression });
        } else {
          ReturnType = new TTuple({});
          addConstraint(new CEqual { new TTuple({}), getReturnType(), N });
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

  TVarRigid* Checker::createRigidVar(ByteString Name) {
    auto TV = new TVarRigid(NextTypeVarId++, Name);
    Contexts.back()->TVs->emplace(TV);
    return TV;
  }

  TVar* Checker::createTypeVar() {
    auto TV = new TVar(NextTypeVarId++, VarKind::Unification);
    Contexts.back()->TVs->emplace(TV);
    return TV;
  }

  InferContext* Checker::createInferContext(TVSet* TVs, ConstraintSet* Constraints) {
    auto Ctx = new InferContext;
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

          auto NewConstraint = Constraint->substitute(Sub);

          // This makes error messages prettier by relating the typing failure
          // to the call site rather than the definition.
          if (NewConstraint->getKind() == ConstraintKind::Equal) {
            static_cast<CEqual*>(NewConstraint)->Source = Source;
          }

          addConstraint(NewConstraint);
        }

        // Note the call to simplify? This is because constraints may have already
        // been solved, with some unification variables being erased. To make
        // sure we instantiate unification variables that are still in use
        // we solve before substituting.
        return simplify(F->Type)->substitute(Sub);
      }

    }

  }

  Constraint* Checker::convertToConstraint(ConstraintExpression* C) {
    switch (C->getKind()) {
      case NodeKind::TypeclassConstraintExpression:
      {
        auto D = static_cast<TypeclassConstraintExpression*>(C);
        std::vector<Type*> Types;
        for (auto TE: D->TEs) {
          Types.push_back(inferTypeExpression(TE));
        }
        return new CClass(D->Name->getCanonicalText(), Types);
      }
      case NodeKind::EqualityConstraintExpression:
      {
        auto D = static_cast<EqualityConstraintExpression*>(C);
        return new CEqual(inferTypeExpression(D->Left), inferTypeExpression(D->Right), C);
      }
      default:
        ZEN_UNREACHABLE
    }
  }

  Type* Checker::inferTypeExpression(TypeExpression* N) {

    switch (N->getKind()) {

      case NodeKind::ReferenceTypeExpression:
      {
        auto RefTE = static_cast<ReferenceTypeExpression*>(N);
        auto Ty = lookupMono(RefTE->Name->getCanonicalText());
        if (Ty == nullptr) {
          DE.add<BindingNotFoundDiagnostic>(RefTE->Name->getCanonicalText(), RefTE->Name);
          Ty = createTypeVar();
        }
        N->setType(Ty);
        return Ty;
      }

      case NodeKind::VarTypeExpression:
      {
        auto VarTE = static_cast<VarTypeExpression*>(N);
        auto Ty = lookupMono(VarTE->Name->getCanonicalText());
        if (Ty == nullptr) {
          if (Config.typeVarsRequireForall()) {
            DE.add<BindingNotFoundDiagnostic>(VarTE->Name->getCanonicalText(), VarTE->Name);
          }
          Ty = createRigidVar(VarTE->Name->getCanonicalText());
          addBinding(VarTE->Name->getCanonicalText(), new Forall(Ty));
        }
        N->setType(Ty);
        return Ty;
      }

      case NodeKind::TupleTypeExpression:
      {
        auto TupleTE = static_cast<TupleTypeExpression*>(N);
        std::vector<Type*> ElementTypes;
        for (auto [TE, Comma]: TupleTE->Elements) {
          ElementTypes.push_back(inferTypeExpression(TE));
        }
        auto Ty = new TTuple(ElementTypes);
        N->setType(Ty);
        return Ty;
      }

      case NodeKind::NestedTypeExpression:
      {
        auto NestedTE = static_cast<NestedTypeExpression*>(N);
        auto Ty = inferTypeExpression(NestedTE->TE);
        N->setType(Ty);
        return Ty;
      }

      case NodeKind::ArrowTypeExpression:
      {
        auto ArrowTE = static_cast<ArrowTypeExpression*>(N);
        std::vector<Type*> ParamTypes;
        for (auto ParamType: ArrowTE->ParamTypes) {
          ParamTypes.push_back(inferTypeExpression(ParamType));
        }
        auto ReturnType = inferTypeExpression(ArrowTE->ReturnType);
        auto Ty = new TArrow(ParamTypes, ReturnType);
        N->setType(Ty);
        return Ty;
      }

      case NodeKind::QualifiedTypeExpression:
      {
        auto QTE = static_cast<QualifiedTypeExpression*>(N);
        for (auto [C, Comma]: QTE->Constraints) {
          addConstraint(convertToConstraint(C));
        }
        auto Ty = inferTypeExpression(QTE->TE);
        N->setType(Ty);
        return Ty;
      }

      default:
        ZEN_UNREACHABLE

    }
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
          auto NewCtx = createInferContext();
          Contexts.push_back(NewCtx);
          inferBindings(Case->Pattern, ValTy);
          auto ResTy = inferExpression(Case->Expression);
          addConstraint(new CEqual(ResTy, Ty, Case->Expression));
          Contexts.pop_back();
        }
        if (!Match->Value) {
          Ty = new TArrow({ ValTy }, Ty);
        }
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
        if (Target && llvm::isa<LetDeclaration>(Target)) {
          auto Let = static_cast<LetDeclaration*>(Target);
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
        addConstraint(new CEqual { OpTy, new TArrow(ArgTypes, Ty), X });
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
        addConstraint(new CEqual { new TArrow(ArgTys, Ty), OpTy, X });
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
        switch (Member->Name->getKind()) {
          case NodeKind::IntegerLiteral:
          {
            auto I = static_cast<IntegerLiteral*>(Member->Name);
            Ty = new TTupleIndex(inferExpression(Member->E), I->getInteger());
            break;
          }
          case NodeKind::Identifier:
          {
            // TODO
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

  void Checker::inferBindings(
    Pattern* Pattern,
    Type* Type,
    ConstraintSet* Constraints,
    TVSet* TVs
  ) {

    switch (Pattern->getKind()) {

      case NodeKind::BindPattern:
      {
        auto P = static_cast<BindPattern*>(Pattern);
        addBinding(P->Name->getCanonicalText(), new Forall(TVs, Constraints, Type));
        break;
      }

      case NodeKind::LiteralPattern:
      {
        auto P = static_cast<LiteralPattern*>(Pattern);
        addConstraint(new CEqual(inferLiteral(P->Literal), Type, P));
        break;
      }

      default:
        ZEN_UNREACHABLE

    }

  }

  void Checker::inferBindings(Pattern* Pattern, Type* Type) {
    inferBindings(Pattern, Type, new ConstraintSet, new TVSet);
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
        // Parameters are clearly no let-decarations. They never have their own
        // inference context, so we have to skip them.
        if (Def == nullptr || Def->getKind() == NodeKind::Parameter) {
          return;
        }
        ZEN_ASSERT(Def->getKind() == NodeKind::LetDeclaration || Def->getKind() == NodeKind::SourceFile);
        RefGraph.addEdge(Stack.top(), Def);
      }

    };

    RefGraph.addVertex(SF);
    Visitor V { {}, RefGraph };
    V.Stack.push(SF);
    V.visit(SF);

  }

  void Checker::checkTypeclassSigs(Node* N) {

    struct LetVisitor : CSTVisitor<LetVisitor> {

      Checker& C;

      void visitLetDeclaration(LetDeclaration* Decl) {

        // Only inspect those let-declarations that look like a function
        if (Decl->Params.empty()) {
          return;
        }

        // Will contain the type classes that were specified in the type assertion by the user.
        // There might be some other signatures as well, but those are an implementation detail.
        std::vector<TypeclassSignature> Expected;

        // We must add the type class itself to Expected because in order for
        // propagation to work the rigid type variables expect this class to be
        // present even inside the current class. By adding it to Expected, we
        // are effectively cancelling out the default behavior of requiring the
        // presence of this type classes.
        if (llvm::isa<ClassDeclaration>(Decl->Parent)) {
            auto Class = llvm::cast<ClassDeclaration>(Decl->Parent);
            std::vector<TVar *> Tys;
            for (auto TE : Class->TypeVars) {
                Tys.push_back(llvm::cast<TVar>(TE->getType()));
            }
            Expected.push_back(
                TypeclassSignature{Class->Name->getCanonicalText(), Tys});
        }

        // Here we scan the type signature for type classes that user expects to be there.
        if (Decl->TypeAssert != nullptr) {
          if (llvm::isa<QualifiedTypeExpression>(Decl->TypeAssert->TypeExpression)) {
            auto QTE = static_cast<QualifiedTypeExpression*>(Decl->TypeAssert->TypeExpression);
            for (auto [C, Comma]: QTE->Constraints) {
              if (llvm::isa<TypeclassConstraintExpression>(C)) {
                auto TCE = static_cast<TypeclassConstraintExpression*>(C);
                std::vector<TVar*> Tys;
                for (auto TE: TCE->TEs) {
                  auto TV = TE->getType();
                  ZEN_ASSERT(llvm::isa<TVar>(TV));
                  Tys.push_back(static_cast<TVar*>(TV));
                }
                Expected.push_back(TypeclassSignature { TCE->Name->getCanonicalText(), Tys });
              }
            }
          }
        }

        // Sort them lexically and remove any duplicates
        std::sort(Expected.begin(), Expected.end());
        Expected.erase(std::unique(Expected.begin(), Expected.end()), Expected.end());

        // Will contain the type class signatures that our program inferred that
        // at the very least should be present to make the body work.
        std::vector<TypeclassSignature> Actual;

        // This is ugly but it works. Scan all type variables local to this
        // declaration and add the classes that they require to Actual.
        for (auto Ty: *Decl->Ctx->TVs) {
          auto S = Ty->substitute(C.Solution);
          if (llvm::isa<TVar>(S)) {
            auto TV = static_cast<TVar*>(S);
            for (auto Class: TV->Contexts) {
              Actual.push_back(TypeclassSignature { Class, { TV } });
            }
          }
        }

        // Sort them lexically and remove any duplicates
        std::sort(Actual.begin(), Actual.end());
        Actual.erase(std::unique(Actual.begin(), Actual.end()), Actual.end());

        auto ActualIter = Actual.begin();
        auto ExpectedIter = Expected.begin();

        for (; ActualIter != Actual.end() || ExpectedIter != Expected.end() ;) {

          // Our program inferred no more type classes that should be present,
          // yet Expected still did find a few that the user declared in a
          // signature. No errors should be reported, and we can quit this loop.
          if (ActualIter == Actual.end()) {
            // TODO Maybe issue a warning that a type class went unused
            break;
          }

          // There are no more type classes that were expected, so any remaining
          // type classes in Actual will not have a corresponding signature.
          // This should be reported as an error.
          if (ExpectedIter == Expected.end()) {
            for (; ActualIter != Actual.end(); ActualIter++) {
              C.DE.add<TypeclassMissingDiagnostic>(*ActualIter, Decl);
            }
            break;
          }

          // If ExpectedIter is already at Show, but ActualIter is still at Eq,
          // then we clearly missed the Eq in ExpectedIter. This clearly is an
          // error, since the user missed something in a type signature.
          if (*ActualIter < *ExpectedIter) {
            C.DE.add<TypeclassMissingDiagnostic>(*ActualIter, Decl);
            ActualIter++;
            continue;
          }

          // If ActualIter is Show but ExpectedIter is still Eq, then the user
          // specified too much type classes in a type signature. This is no error,
          // but it might be worthwhile to issue a warning.
          if (*ExpectedIter < *ActualIter) {
            // DE.add<TypeclassMissingDiagnostic>(It2->Name, Decl);
            ExpectedIter++;
            continue;
          }

          // Both type class signatures are equal, cancelling each other out.
          ActualIter++;
          ExpectedIter++;
        }

      }

    };

    LetVisitor V { {}, *this };
    V.visit(N);

  }

  void Checker::check(SourceFile *SF) {
    auto RootContext = createInferContext();
    Contexts.push_back(RootContext);
    addBinding("String", new Forall(StringType));
    addBinding("Int", new Forall(IntType));
    addBinding("Bool", new Forall(BoolType));
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
      if (Nodes.size() == 1 && llvm::isa<SourceFile>(Nodes[0])) {
        continue;
      }
      auto TVs = new TVSet;
      auto Constraints = new ConstraintSet;
      for (auto N: Nodes) {
        auto Decl = static_cast<LetDeclaration*>(N);
        forwardDeclareLetDeclaration(Decl, TVs, Constraints);
      }
    }
    for (auto Nodes: SCCs) {
      if (Nodes.size() == 1 && llvm::isa<SourceFile>(Nodes[0])) {
        continue;
      }
      for (auto N: Nodes) {
        auto Decl = static_cast<LetDeclaration*>(N);
        Decl->IsCycleActive = true;
      }
      for (auto N: Nodes) {
        auto Decl = static_cast<LetDeclaration*>(N);
        inferLetDeclaration(Decl);
      }
      for (auto N: Nodes) {
        auto Decl = static_cast<LetDeclaration*>(N);
        Decl->IsCycleActive = false;
      }
    }
    infer(SF);
    Contexts.pop_back();
    solve(new CMany(*RootContext->Constraints), Solution);
    checkTypeclassSigs(SF);
  }

  void Checker::solve(Constraint* Constraint, TVSub& Solution) {

    Queue.push_back(Constraint);

    while (!Queue.empty()) {

      auto Constraint = Queue.front();
      Queue.pop_front();

      switch (Constraint->getKind()) {

        case ConstraintKind::Class:
        {
          // TODO
          break;
        }

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
          solveCEqual(static_cast<CEqual*>(Constraint));
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
      ZEN_ASSERT(Con1->Args.size() == Con2->Args.size());
      for (auto [T1, T2]: zen::zip(Con1->Args, Con2->Args)) {
        if (!assignableTo(T1, T2)) {
          return false;
        }
      }
      return true;
    }
    ZEN_UNREACHABLE
  }

  std::vector<TypeclassContext> Checker::findInstanceContext(TCon* Ty, TypeclassId& Class) {
    auto Match = InstanceMap.find(Class);
    std::vector<TypeclassContext> S;
    if (Match != InstanceMap.end()) {
      for (auto Instance: Match->second) {
        if (assignableTo(Ty, Instance->TypeExps[0]->getType())) {
          std::vector<TypeclassContext> S;
          for (auto Arg: Ty->Args) {
            TypeclassContext Classes;
            // TODO
            S.push_back(Classes);
          }
          return S;
        }
      }
    }
    DE.add<InstanceNotFoundDiagnostic>(Class, Ty, Source);
    for (auto Arg: Ty->Args) {
      S.push_back({});
    }
    return S;
  }

  void Checker::propagateClasses(std::unordered_set<TypeclassId>& Classes, Type* Ty) {
    if (llvm::isa<TVar>(Ty)) {
      auto TV = llvm::cast<TVar>(Ty);
      for (auto Class: Classes) {
        TV->Contexts.emplace(Class);
      }
    } else if (llvm::isa<TCon>(Ty)) {
      for (auto Class: Classes) {
        propagateClassTycon(Class, llvm::cast<TCon>(Ty));
      }
    } else if (!Classes.empty()) {
      DE.add<InvalidTypeToTypeclassDiagnostic>(Ty, std::vector(Classes.begin(), Classes.end()), Source);
    }
  };

  void Checker::propagateClassTycon(TypeclassId& Class, TCon* Ty) {
    auto S = findInstanceContext(Ty, Class);
    for (auto [Classes, Arg]: zen::zip(S, Ty->Args)) {
      propagateClasses(Classes, Arg);
    }
  };

  void Checker::solveCEqual(CEqual* C) {
    // std::cerr << describe(C->Left) << " ~ " << describe(C->Right) << std::endl;
    OrigLeft = C->Left;
    OrigRight = C->Right;
    Source = C->Source;
    unify(C->Left, C->Right);
    LeftPath = {};
    RightPath = {};
  }

  Type* Checker::find(Type* Ty) {
    while (Ty->getKind() == TypeKind::Var) {
      auto Match = Solution.find(static_cast<TVar*>(Ty));
      if (Match == Solution.end()) {
        break;
      }
      Ty = Match->second;
    }
    return Ty;
  }

  Type* Checker::simplify(Type* Ty) {

    Ty = find(Ty);

    switch (Ty->getKind()) {

      case TypeKind::Var:
        break;

      case TypeKind::Tuple:
      {
        auto Tuple = static_cast<TTuple*>(Ty);
        bool Changed = false;
        std::vector<Type*> NewElementTypes;
        for (auto Ty: Tuple->ElementTypes) {
          auto NewElementType = simplify(Ty);
          if (NewElementType != Ty) {
            Changed = true;
          }
          NewElementTypes.push_back(NewElementType);
        }
        return Changed ? new TTuple(NewElementTypes) : Ty;
      }

      case TypeKind::Arrow:
      {
        auto Arrow = static_cast<TArrow*>(Ty);
        bool Changed = false;
        std::vector<Type*> NewParamTys;
        for (auto ParamTy: Arrow->ParamTypes) { 
          auto NewParamTy = simplify(ParamTy);
          if (NewParamTy != ParamTy) {
            Changed = true;
          }
          NewParamTys.push_back(NewParamTy);
        }
        auto NewRetTy = simplify(Arrow->ReturnType);
        if (NewRetTy != Arrow->ReturnType) {
          Changed = true;
        }
        Ty = Changed ? new TArrow(NewParamTys, NewRetTy) : Arrow;
        break;
      }

      case TypeKind::Con:
      {
        auto Con = static_cast<TCon*>(Ty);
        bool Changed = false;
        std::vector<Type*> NewArgs;
        for (auto Arg: Con->Args) {
          auto NewArg = simplify(Arg);
          if (NewArg != Arg) {
            Changed = true;
          }
          NewArgs.push_back(NewArg);
        }
        return Changed ? new TCon(Con->Id, NewArgs, Con->DisplayName) : Ty;
      }

      case TypeKind::TupleIndex:
      {
        auto Index = static_cast<TTupleIndex*>(Ty);
        auto MaybeTuple = simplify(Index->Ty);
        if (llvm::isa<TTuple>(MaybeTuple)) {
          auto Tuple = static_cast<TTuple*>(MaybeTuple);
          if (Index->I >= Tuple->ElementTypes.size()) {
            DE.add<TupleIndexOutOfRangeDiagnostic>(Tuple, Index->I);
          } else {
            Ty = simplify(Tuple->ElementTypes[Index->I]);
          }
        }
        break;
      }

    }

    return Ty;
  }

  void Checker::join(TVar* TV, Type* Ty) {

    Solution[TV] = Ty;

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
    if (!Contexts.empty()) {
      // std::cerr << "erase " << describe(TV) << std::endl;
      auto TVs = Contexts.back()->TVs;
      TVs->erase(TV);
    }

  }

  void Checker::unifyError() {
    DE.add<UnificationErrorDiagnostic>(
      simplify(OrigLeft),
      simplify(OrigRight),
      LeftPath,
      RightPath,
      Source
    );
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

  bool Checker::unify(Type* A, Type* B) {

    A = simplify(A);
    B = simplify(B);

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

    if (llvm::isa<TVar>(B)) {
      return unify(B, A);
    }

    if (llvm::isa<TArrow>(A) && llvm::isa<TArrow>(B)) {
      auto C1 = ArrowCursor(static_cast<TArrow*>(A), LeftPath);
      auto C2 = ArrowCursor(static_cast<TArrow*>(B), RightPath);
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
        if (!unify(T1, T2)) {
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

    if (llvm::isa<TArrow>(A)) {
      auto Arr = static_cast<TArrow*>(A);
      if (Arr->ParamTypes.empty()) {
        return unify(Arr->ReturnType, B);
      }
    }

    if (llvm::isa<TArrow>(B)) {
      return unify(B, A);
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
        if (!unify(Tuple1->ElementTypes[I], Tuple2->ElementTypes[I])) {
          Success = false;
        }
        LeftPath.pop_back();
        RightPath.pop_back();
      }
      return Success;
    }

    if (llvm::isa<TTupleIndex>(A) || llvm::isa<TTupleIndex>(B)) {
      Queue.push_back(C);
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
      ZEN_ASSERT(Con1->Args.size() == Con2->Args.size());
      auto Count = Con1->Args.size();
      bool Success = true;
      for (std::size_t I = 0; I < Count; I++) {
        LeftPath.push_back(TypeIndex::forConArg(I));
        RightPath.push_back(TypeIndex::forConArg(I));
        if (!unify(Con1->Args[I], Con2->Args[I])) {
          Success = false;
        }
        LeftPath.pop_back();
        RightPath.pop_back();
      }
      return Success;
    }

    unifyError();
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

  Type* Checker::getType(TypedNode *Node) {
    return Node->getType()->substitute(Solution);
  }

}

