
// TODO Add list of CST variable names to TVar and unify them so that e.g. the typeclass checker may pick one when displaying a diagnostic

// TODO (maybe) make unficiation work like union-find in find()

// TODO remove Args in TCon and just use it as a constant
// TODO make TApp traversable with TupleIndex

// TODO make simplify() rewrite the types in-place such that a reference too (Bool, Int).0 becomes Bool

// TODO Add a check for datatypes that create infinite structures.

// TODO see if we can merge UnificationError diagnostics so that we get a list of **all** types that were wrong on a given node

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

      case NodeKind::VariantDeclaration:
      {
        auto Decl = static_cast<VariantDeclaration*>(X);

        auto& ParentCtx = getContext();
        auto Ctx = createInferContext();
        Contexts.push_back(Ctx);

        std::vector<TVar*> Vars;
        for (auto TE: Decl->TVs) {
          auto TV = createRigidVar(TE->Name->getCanonicalText());
          Ctx->TVs->emplace(TV);
          Vars.push_back(TV);
        }

        Type* Ty = createConType(Decl->Name->getCanonicalText());

        // Must be added early so we can create recursive types
        ParentCtx.Env.emplace(Decl->Name->getCanonicalText(), new Forall(Ty));

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
              ParentCtx.Env.emplace(TupleMember->Name->getCanonicalText(), new Forall(Ctx->TVs, Ctx->Constraints, new TArrow(ParamTypes, RetTy)));
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

        Contexts.pop_back();

        break;
      }

      case NodeKind::RecordDeclaration:
      {
        auto Decl = static_cast<RecordDeclaration*>(X);

        auto& ParentCtx = getContext();
        auto Ctx = createInferContext();
        Contexts.push_back(Ctx);
        std::vector<TVar*> Vars;
        for (auto TE: Decl->Vars) {
          auto TV = createRigidVar(TE->Name->getCanonicalText());
          Ctx->TVs->emplace(TV);
          Vars.push_back(TV);
        }

        auto Name = Decl->Name->getCanonicalText();
        auto Ty = createConType(Name);

        // Must be added early so we can create recursive types
        ParentCtx.Env.emplace(Name, new Forall(Ty));

        // Corresponds to the logic of one branch of a VaraintDeclarationMember
        Type* FieldsTy = new TNil();
        for (auto Field: Decl->Fields) {
          FieldsTy = new TField(Field->Name->getCanonicalText(), new TPresent(inferTypeExpression(Field->TypeExpression)), FieldsTy);
        }
        Type* RetTy = Ty;
        for (auto TV: Vars) {
          RetTy = new TApp(RetTy, TV);
        }
        Contexts.pop_back();
        addBinding(Name, new Forall(Ctx->TVs, Ctx->Constraints, new TArrow({ FieldsTy }, RetTy)));

        break;
      }

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
        addConstraint(new CEqual(Param, TE->getType(), TE));
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

    Type* BindTy;
    if (HasContext) {
      Contexts.pop_back();
      BindTy = inferPattern(Let->Pattern, Let->Ctx->Constraints, Let->Ctx->TVs);
    } else {
      BindTy = inferPattern(Let->Pattern);
    }
    addConstraint(new CEqual(BindTy, Ty, Let));

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

      case NodeKind::VariantDeclaration:
      case NodeKind::RecordDeclaration:
        // Nothing to do for a type-level declaration
        break;

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

  TCon* Checker::createConType(ByteString Name) {
    return new TCon(NextConTypeId++, Name);
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
        return simplifyType(F->Type)->substitute(Sub);
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
        Type* Ty = inferTypeExpression(AppTE->Op);
        for (auto Arg: AppTE->Args) {
          Ty = new TApp(Ty, inferTypeExpression(Arg));
        }
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
        ZEN_ASSERT(Ty->getKind() == TypeKind::Var);
        N->setType(Ty);
        return static_cast<TVar*>(Ty);
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
          auto NewCtx = createInferContext();
          Contexts.push_back(NewCtx);
          auto PattTy = inferPattern(Case->Pattern);
          addConstraint(new CEqual(PattTy, ValTy, X));
          auto ExprTy = inferExpression(Case->Expression);
          addConstraint(new CEqual(ExprTy, Ty, Case->Expression));
          Contexts.pop_back();
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
            addConstraint(new CEqual(new TField(K->getCanonicalText(), Ty, RestTy), ExprTy, Member));
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
        addConstraint(new CEqual(Ty, new TArrow(ParamTypes, RetTy), P));
        return RetTy;
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
          auto S = Ty->solve();
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

  Type* Checker::getType(TypedNode *Node) {
    return Node->getType()->solve();
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
    solve(new CMany(*RootContext->Constraints));
    checkTypeclassSigs(SF);
  }

  void Checker::solve(Constraint* Constraint) {

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
      // TODO must handle a TApp
      // ZEN_ASSERT(Con1->Args.size() == Con2->Args.size());
      // for (auto [T1, T2]: zen::zip(Con1->Args, Con2->Args)) {
      //   if (!assignableTo(T1, T2)) {
      //     return false;
      //   }
      // }
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
          // TODO handle TApp
          // for (auto Arg: Ty->Args) {
          //   TypeclassContext Classes;
          //   // TODO
          //   S.push_back(Classes);
          // }
          return S;
        }
      }
    }
    DE.add<InstanceNotFoundDiagnostic>(Class, Ty, Source);
    // TODO handle TApp
    // for (auto Arg: Ty->Args) {
    //   S.push_back({});
    // }
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
    // TODO handle TApp
    // for (auto [Classes, Arg]: zen::zip(S, Ty->Args)) {
    //   propagateClasses(Classes, Arg);
    // }
  };

  void Checker::join(TVar* TV, Type* Ty) {

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
    if (!Contexts.empty()) {
      // std::cerr << "erase " << describe(TV) << std::endl;
      auto TVs = Contexts.back()->TVs;
      TVs->erase(TV);
    }

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

    bool unify(Type* A, Type* B);

    bool unifyField(Type* A, Type* B);

    bool unify() {
      return unify(Constraint->Left, Constraint->Right);
    }

  };

  class UnificationFrame {

    Unifier& U;
    Type* A;
    Type* B;
    bool DidSwap = false;

  public:

    UnificationFrame(Unifier& U, Type* A, Type* B):
      U(U), A(U.C.simplifyType(A)), B(U.C.simplifyType(B)) {}

    void unifyError() {
      U.C.DE.add<UnificationErrorDiagnostic>(
        U.C.simplifyType(U.Constraint->Left),
        U.C.simplifyType(U.Constraint->Right),
        U.LeftPath,
        U.RightPath,
        U.Constraint->Source
      );
    }

    void pushLeft(TypeIndex I) {
      if (DidSwap) {
        U.RightPath.push_back(I);
      } else {
        U.LeftPath.push_back(I);
      }
    }

    void popLeft() {
      if (DidSwap) {
        U.RightPath.pop_back();
      } else {
        U.LeftPath.pop_back();
      }
    }

    void pushRight(TypeIndex I) {
      if (DidSwap) {
        U.LeftPath.push_back(I);
      } else {
        U.RightPath.push_back(I);
      }
    }

    void popRight() {
      if (DidSwap) {
        U.LeftPath.pop_back();
      } else {
        U.RightPath.pop_back();
      }
    }

    void swap() {
      std::swap(A, B);
      DidSwap = !DidSwap;
    }

    bool unifyField() {
      if (llvm::isa<TAbsent>(A) && llvm::isa<TAbsent>(B)) {
        return true;
      }
      if (llvm::isa<TAbsent>(B)) {
        swap();
      }
      if (llvm::isa<TAbsent>(A)) {
        auto Present = static_cast<TPresent*>(B);
        U.C.DE.add<FieldNotFoundDiagnostic>(U.CurrentFieldName, U.C.simplifyType(U.getLeft()), U.LeftPath, U.getSource());
        return false;
      }
      auto Present1 = static_cast<TPresent*>(A);
      auto Present2 = static_cast<TPresent*>(B);
      return U.unify(Present1->Ty, Present2->Ty);
    }

    bool unify() {

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
          U.C.join(From, To);
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

        U.C.join(TV, B);

        return true;
      }

      if (llvm::isa<TArrow>(A) && llvm::isa<TArrow>(B)) {
        auto C1 = ArrowCursor(static_cast<TArrow*>(A), DidSwap ? U.RightPath : U.LeftPath);
        auto C2 = ArrowCursor(static_cast<TArrow*>(B), DidSwap ? U.LeftPath : U.RightPath);
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
          if (!U.unify(T1, T2)) {
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
        if (!U.unify(App1->Op, App2->Op)) {
          Success = false;
        }
        if (!U.unify(App1->Arg, App2->Arg)) {
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
          auto Success = U.unify(Arr->ReturnType, B);
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
          U.LeftPath.push_back(TypeIndex::forTupleElement(I));
          U.RightPath.push_back(TypeIndex::forTupleElement(I));
          if (!U.unify(Tuple1->ElementTypes[I], Tuple2->ElementTypes[I])) {
            Success = false;
          }
          U.LeftPath.pop_back();
          U.RightPath.pop_back();
        }
        return Success;
      }

      if (llvm::isa<TTupleIndex>(A) || llvm::isa<TTupleIndex>(B)) {
        // Type(s) could not be simplified at the beginning of this function,
        // so we have to re-visit the constraint when there is more information.
        U.C.Queue.push_back(U.Constraint);
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
          U.LeftPath.push_back(TypeIndex::forFieldType());
          U.RightPath.push_back(TypeIndex::forFieldType());
          U.CurrentFieldName = Field1->Name;
          if (!U.unifyField(Field1->Ty, Field2->Ty)) {
            Success = false;
          }
          U.LeftPath.pop_back();
          U.RightPath.pop_back();
          U.LeftPath.push_back(TypeIndex::forFieldRest());
          U.RightPath.push_back(TypeIndex::forFieldRest());
          if (!U.unify(Field1->RestTy, Field2->RestTy)) {
            Success = false;
          }
          U.LeftPath.pop_back();
          U.RightPath.pop_back();
          return Success;
        }
        auto NewRestTy = new TVar(U.C.NextTypeVarId++, VarKind::Unification);
        pushLeft(TypeIndex::forFieldRest());
        if (!U.unify(Field1->RestTy, new TField(Field2->Name, Field2->Ty, NewRestTy))) {
          Success = false;
        }
        popLeft();
        pushRight(TypeIndex::forFieldRest());
        if (!U.unify(new TField(Field1->Name, Field1->Ty, NewRestTy), Field2->RestTy)) {
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
        U.CurrentFieldName = Field->Name;
        if (!U.unifyField(Field->Ty, new TAbsent)) {
          Success = false;
        }
        popLeft();
        pushLeft(TypeIndex::forFieldRest());
        if (!U.unify(Field->RestTy, B)) {
          Success = false;
        }
        popLeft();
        return Success;
      }

      unifyError();
      return false;
    }

  };

  bool Unifier::unify(Type* A, Type* B) {
    UnificationFrame Frame { *this, A, B };
    return Frame.unify();
  }

  bool Unifier::unifyField(Type* A, Type* B) {
    UnificationFrame Frame { *this, A, B };
    return Frame.unifyField();
  }

  void Checker::solveCEqual(CEqual* C) {
    // std::cerr << describe(C->Left) << " ~ " << describe(C->Right) << std::endl;
    Unifier A { *this, C };
    A.unify();
  }


}

