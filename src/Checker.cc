
// TODO Add list of CST variable names to TVar and unify them so that e.g. the typeclass checker may pick one when displaying a diagnostic

// TODO make sure that if we have Eq Int, Eq a ~ Eq Int such that an instance binding eq has the correct type

// TODO make unficiation work like union-find in find()

#include <algorithm>
#include <iterator>
#include <stack>

#include "llvm/Support/Casting.h"

#include "zen/config.hpp"
#include "zen/range.hpp"

#include "bolt/CSTVisitor.hpp"
#include "bolt/Diagnostics.hpp"
#include "bolt/CST.hpp"
#include "bolt/Checker.hpp"

namespace bolt {

  std::string describe(const Type* Ty);

  bool TypeclassSignature::operator<(const TypeclassSignature& Other) const {
    if (Id < Other.Id) {
      return true;
    }
    ZEN_ASSERT(Params.size() == 1);
    ZEN_ASSERT(Other.Params.size() == 1);
    return Params[0]->Id < Other.Params[0]->Id;
  }

  bool TypeclassSignature::operator==(const TypeclassSignature& Other) const {
    ZEN_ASSERT(Params.size() == 1);
    ZEN_ASSERT(Other.Params.size() == 1);
    return Id == Other.Id && Params[0]->Id == Other.Params[0]->Id;
  }

  void Type::addTypeVars(TVSet& TVs) {
    switch (Kind) {
      case TypeKind::Var:
        TVs.emplace(static_cast<TVar*>(this));
        break;
      case TypeKind::Arrow:
      {
        auto Arrow = static_cast<TArrow*>(this);
        for (auto Ty: Arrow->ParamTypes) {
          Ty->addTypeVars(TVs);
        }
        Arrow->ReturnType->addTypeVars(TVs);
        break;
      }
      case TypeKind::Con:
      {
        auto Con = static_cast<TCon*>(this);
        for (auto Ty: Con->Args) {
          Ty->addTypeVars(TVs);
        }
        break;
      }
      case TypeKind::Tuple:
      {
        auto Tuple = static_cast<TTuple*>(this);
        for (auto Ty: Tuple->ElementTypes) {
          Ty->addTypeVars(TVs);
        }
        break;
      }
    }
  }

  bool Type::hasTypeVar(const TVar* TV) {
    switch (Kind) {
      case TypeKind::Var:
        return static_cast<TVar*>(this)->Id == TV->Id;
      case TypeKind::Arrow:
      {
        auto Arrow = static_cast<TArrow*>(this);
        for (auto Ty: Arrow->ParamTypes) {
          if (Ty->hasTypeVar(TV)) {
            return true;
          }
        }
        return Arrow->ReturnType->hasTypeVar(TV);
      }
      case TypeKind::Con:
      {
        auto Con = static_cast<TCon*>(this);
        for (auto Ty: Con->Args) {
          if (Ty->hasTypeVar(TV)) {
            return true;
          }
        }
        return false;
      }
      case TypeKind::Tuple:
      {
        auto Tuple = static_cast<TTuple*>(this);
        for (auto Ty: Tuple->ElementTypes) {
          if (Ty->hasTypeVar(TV)) {
            return true;
          }
        }
        return false;
      }
    }
  }

  Type* Type::substitute(const TVSub &Sub) {
    switch (Kind) {
      case TypeKind::Var:
      {
        auto TV = static_cast<TVar*>(this);
        auto Match = Sub.find(TV);
        return Match != Sub.end() ? Match->second->substitute(Sub) : this;
      }
      case TypeKind::Arrow:
      {
        auto Arrow = static_cast<TArrow*>(this);
        bool Changed = false;
        std::vector<Type*> NewParamTypes;
        for (auto Ty: Arrow->ParamTypes) {
          auto NewParamType = Ty->substitute(Sub);
          if (NewParamType != Ty) {
            Changed = true;
          }
          NewParamTypes.push_back(NewParamType);
        }
        auto NewRetTy = Arrow->ReturnType->substitute(Sub) ;
        if (NewRetTy != Arrow->ReturnType) {
          Changed = true;
        }
        return Changed ? new TArrow(NewParamTypes, NewRetTy) : this;
      }
      case TypeKind::Con:
      {
        auto Con = static_cast<TCon*>(this);
        bool Changed = false;
        std::vector<Type*> NewArgs;
        for (auto Arg: Con->Args) {
          auto NewArg = Arg->substitute(Sub);
          if (NewArg != Arg) {
            Changed = true;
          }
          NewArgs.push_back(NewArg);
        }
        return Changed ? new TCon(Con->Id, NewArgs, Con->DisplayName) : this;
      }
      case TypeKind::Tuple:
      {
        auto Tuple = static_cast<TTuple*>(this);
        bool Changed = false;
        std::vector<Type*> NewElementTypes;
        for (auto Ty: Tuple->ElementTypes) {
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
    for (auto Iter = Contexts.rbegin(); Iter != Contexts.rend(); Iter++) {
      auto& Ctx = **Iter;
      if (!Ctx.isEnvPervious()) {
        Ctx.Env.emplace(Name, Scm);
        return;
      }
    }
    ZEN_UNREACHABLE
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
        std::size_t MaxLevel = 0;
        for (std::size_t I = Contexts.size(); I-- > 0; ) {
          auto Ctx = Contexts[I];
          if (hasTypeVar(*Ctx->TVs, Y->Left) || hasTypeVar(*Ctx->TVs, Y->Right)) {
            MaxLevel = I;
            break;
          }
        }
        std::size_t MinLevel = MaxLevel;
        for (std::size_t I = 0; I < Contexts.size(); I++) {
          auto Ctx = Contexts[I];
          if (hasTypeVar(*Ctx->TVs, Y->Left) || hasTypeVar(*Ctx->TVs, Y->Right)) {
            MinLevel = I;
            break;
          }
        }
        if (MaxLevel == MinLevel) {
          solveCEqual(Y);
        } else {
          Contexts[MaxLevel]->Constraints->push_back(C);
        }
        // Contexts.front()->Constraints->push_back(C);
        //auto I = std::max(Y->Left->MaxDepth, Y->Right->MaxDepth);
        //ZEN_ASSERT(I < Contexts.size());
        //auto Ctx = Contexts[I];
        //Ctx->Constraints.push_back(Constraint);
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
          auto TV = createRigidVar(TE->Name->getCanonicalText());
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
        auto Match = InstanceMap.find(Decl->Name->getCanonicalText());
        if (Match == InstanceMap.end()) {
          InstanceMap.emplace(Decl->Name->getCanonicalText(), std::vector { Decl });
        } else {
          Match->second.push_back(Decl);
        }
        auto Ctx = createInferContext();
        Contexts.push_back(Ctx);
        for (auto Element: Decl->Elements) {
          forwardDeclare(Element);
        }
        Contexts.pop_back();
        break;
      }

      case NodeKind::LetDeclaration:
      {
        auto Let = static_cast<LetDeclaration*>(X);

        auto NewCtx = createInferContext();
        Let->Ctx = NewCtx;

        Contexts.push_back(NewCtx);

        // If declaring a let-declaration inside a type class declaration,
        // we need to mark that the let-declaration requires this class.
        // This marking is set on the rigid type variables of the class, which
        // are then added to this local type environment.
        if (llvm::isa<ClassDeclaration>(Let->Parent)) {
          auto Decl = static_cast<ClassDeclaration*>(Let->Parent);
          for (auto TE: Decl->TypeVars) {
            auto TV = llvm::cast<TVar>(TE->getType());
            NewCtx->Env.emplace(TE->Name->getCanonicalText(), new Forall(TV));
            NewCtx->TVs->emplace(TV);
          }
        }

        Type* Ty;
        if (Let->TypeAssert) {
          Ty = inferTypeExpression(Let->TypeAssert->TypeExpression);
        } else {
          Ty = createTypeVar();
        }
        Let->Ty = Ty;

        if (Let->Body) {
          switch (Let->Body->getKind()) {
            case NodeKind::LetExprBody:
              break;
            case NodeKind::LetBlockBody:
            {
              auto Block = static_cast<LetBlockBody*>(Let->Body);
              NewCtx->ReturnType = createTypeVar();
              for (auto Element: Block->Elements) {
                forwardDeclare(Element);
              }
              break;
            }
            default:
              ZEN_UNREACHABLE
          }
        }

        Contexts.pop_back();

        inferBindings(Let->Pattern, Ty, NewCtx->Constraints, NewCtx->TVs);

        break;
      }

      default:
        ZEN_UNREACHABLE

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

        // Needed to set the associated Type on the CST node
        for (auto TE: Decl->TypeExps) {
          inferTypeExpression(TE);
        }

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
      {
        auto Decl = static_cast<LetDeclaration*>(N);

        auto NewCtx = Decl->Ctx;
        Contexts.push_back(NewCtx);

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
              RetType = createTypeVar();
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

        if (ParamTypes.empty()) {
          // Declaration is a plain (typed) variable
          addConstraint(new CEqual { Decl->Ty, RetType, N });
        } else {
          // Declaration is a function
          addConstraint(new CEqual { Decl->Ty, new TArrow(ParamTypes, RetType), N });
        }

        Contexts.pop_back();

        break;
      }

      case NodeKind::ReturnStatement:
      {
        auto RetStmt = static_cast<ReturnStatement*>(N);
        Type* ReturnType;
        if (RetStmt->Expression) {
          ReturnType = inferExpression(RetStmt->Expression);
        } else {
          ReturnType = new TTuple({});
        }
        addConstraint(new CEqual { ReturnType, getReturnType(), N });
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

  InferContext* Checker::createInferContext() {
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
              static_cast<CEqual *>(NewConstraint)->Source = Source;
          }

          addConstraint(NewConstraint);
        }

        return F->Type->substitute(Sub);
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
          if (Config.typeVarsRequireForall()) {
            DE.add<BindingNotFoundDiagnostic>(RefTE->Name->getCanonicalText(), RefTE->Name);
          }
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
        auto ResTy = createTypeVar();
        for (auto Case: Match->Cases) {
          auto NewCtx = createInferContext();
          Contexts.push_back(NewCtx);
          inferBindings(Case->Pattern, ValTy);
          auto Ty = inferExpression(Case->Expression);
          addConstraint(new CEqual(Ty, ResTy, Case->Expression));
          Contexts.pop_back();
        }
        if (!Match->Value) {
          return new TArrow({ ValTy }, ResTy);
        }
        return ResTy;
      }

      case NodeKind::ConstantExpression:
      {
        auto Const = static_cast<ConstantExpression*>(X);
        auto Ty = inferLiteral(Const->Token);
        X->setType(Ty);
        return Ty;
      }

      case NodeKind::ReferenceExpression:
      {
        auto Ref = static_cast<ReferenceExpression*>(X);
        ZEN_ASSERT(Ref->ModulePath.empty());
        auto Ctx = lookupCall(Ref, Ref->getSymbolPath());
        if (Ctx) {
          /* std::cerr << "recursive call!\n"; */
          ZEN_ASSERT(Ctx->ReturnType != nullptr);
          return Ctx->ReturnType;
        }
        auto Scm = lookup(Ref->Name->getCanonicalText());
        if (Scm == nullptr) {
          DE.add<BindingNotFoundDiagnostic>(Ref->Name->getCanonicalText(), Ref->Name);
          return createTypeVar();
        }
        auto Ty = instantiate(Scm, X);
        X->setType(Ty);
        return Ty;
      }

      case NodeKind::CallExpression:
      {
        auto Call = static_cast<CallExpression*>(X);
        auto OpTy = inferExpression(Call->Function);
        auto RetType = createTypeVar();
        std::vector<Type*> ArgTypes;
        for (auto Arg: Call->Args) {
          ArgTypes.push_back(inferExpression(Arg));
        }
        addConstraint(new CEqual { OpTy, new TArrow(ArgTypes, RetType), X });
        X->setType(RetType);
        return RetType;
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
        auto RetTy = createTypeVar();
        std::vector<Type*> ArgTys;
        ArgTys.push_back(inferExpression(Infix->LHS));
        ArgTys.push_back(inferExpression(Infix->RHS));
        addConstraint(new CEqual { new TArrow(ArgTys, RetTy), OpTy, X });
        X->setType(RetTy);
        return RetTy;
      }

      case NodeKind::NestedExpression:
      {
        auto Nested = static_cast<NestedExpression*>(X);
        return inferExpression(Nested->Inner);
      }

      default:
        ZEN_UNREACHABLE

    }

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

  void collectTypeclasses(LetDeclaration* Decl, std::vector<TypeclassSignature>& Out) {
    if (llvm::isa<ClassDeclaration>(Decl->Parent)) {
      auto Class = llvm::cast<ClassDeclaration>(Decl->Parent);
      std::vector<TVar*> Tys;
      for (auto TE: Class->TypeVars) {
        Tys.push_back(llvm::cast<TVar>(TE->getType()));
      }
      Out.push_back(TypeclassSignature { Class->Name->getCanonicalText(), Tys });
    }
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
            Out.push_back(TypeclassSignature { TCE->Name->getCanonicalText(), Tys });
          }
        }
      }
    }
  }

  void Checker::checkTypeclassSigs(Node* N) {

    struct LetVisitor : CSTVisitor<LetVisitor> {

      Checker& C;

      void visitLetDeclaration(LetDeclaration* Decl) {

        std::vector<TypeclassSignature> Expected;
        collectTypeclasses(Decl, Expected);
        std::sort(Expected.begin(), Expected.end());
        Expected.erase(std::unique(Expected.begin(), Expected.end()), Expected.end());

        std::vector<TypeclassSignature> Actual;
        for (auto Ty: *Decl->Ctx->TVs) {
          auto S = Ty->substitute(C.Solution);
          if (llvm::isa<TVar>(S)) {
            auto TV = static_cast<TVar*>(S);
            for (auto Class: TV->Contexts) {
              Actual.push_back(TypeclassSignature { Class, { TV } });
            }
          }
        }
        std::sort(Actual.begin(), Actual.end());
        Actual.erase(std::unique(Actual.begin(), Actual.end()), Actual.end());

        auto It1 = Actual.begin();
        auto It2 = Expected.begin();

        for (; It1 != Actual.end() || It2 != Expected.end() ;) {
          if (It1 == Actual.end()) {
            // TODO Maybe issue a warning that a type class went unused
            break;
          }
          if (It2 == Expected.end()) {
            for (; It1 != Actual.end(); It1++) {
              C.DE.add<TypeclassMissingDiagnostic>(*It1, Decl);
            }
            break;
          }
          if (*It1 < *It2) {
            // FIXME It1->Ty needs to be unified with potential candidate It2->Ty
            C.DE.add<TypeclassMissingDiagnostic>(*It1, Decl);
            It1++;
            continue;
          }
          if (*It2 < *It1) {
            // DE.add<TypeclassMissingDiagnostic>(It2->Name, Decl);
            It2++;
            continue;
          }
          It1++;
          It2++;
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
    forwardDeclare(SF);
    infer(SF);
    Contexts.pop_back();
    solve(new CMany(*RootContext->Constraints), Solution);
    checkTypeclassSigs(SF);
  }

  void Checker::solve(Constraint* Constraint, TVSub& Solution) {

    std::stack<class Constraint*> Queue;
    Queue.push(Constraint);

    while (!Queue.empty()) {

      auto Constraint = Queue.top();

      Queue.pop();

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
            Queue.push(Constraint);
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

  std::vector<TypeclassContext> Checker::findInstanceContext(TCon* Ty, TypeclassId& Class, Node* Source) {
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

  void Checker::propagateClasses(std::unordered_set<TypeclassId>& Classes, Type* Ty, Node* Source) {
    if (llvm::isa<TVar>(Ty)) {
      auto TV = llvm::cast<TVar>(Ty);
      for (auto Class: Classes) {
        TV->Contexts.emplace(Class);
      }
    } else if (llvm::isa<TCon>(Ty)) {
      for (auto Class: Classes) {
        propagateClassTycon(Class, llvm::cast<TCon>(Ty), Source);
      }
    } else {
      ZEN_UNREACHABLE
      // DE.add<InvalidArgumentToTypeclassDiagnostic>(Ty);
    }
  };

  void Checker::propagateClassTycon(TypeclassId& Class, TCon* Ty, Node* Source) {
    auto S = findInstanceContext(Ty, Class, Source);
    for (auto [Classes, Arg]: zen::zip(S, Ty->Args)) {
      propagateClasses(Classes, Arg, Source);
    }
  };

  class ArrowCursor {

    std::stack<std::tuple<TArrow*, std::size_t>> Path;

  public:

    ArrowCursor(TArrow* Arr) {
      Path.push({ Arr, 0 });
    }

    Type* next() {
      while (!Path.empty()) {
        auto& [Arr, I] = Path.top();
        Type* Ty;
        if (I == -1) {
          Path.pop();
          continue;
        }
        if (I == Arr->ParamTypes.size()) {
          I = -1;
          Ty = Arr->ReturnType;
        }  else {
          Ty = Arr->ParamTypes[I];
          I++;
        }
        if (llvm::isa<TArrow>(Ty)) {
          Path.push({ static_cast<TArrow*>(Ty), 0 });
        } else {
          return Ty;
        }
      }
      return nullptr;
    }

  };

  void Checker::solveCEqual(CEqual* C) {
    /* std::cerr << describe(C->Left) << " ~ " << describe(C->Right) << std::endl; */
    if (!unify(C->Left, C->Right, C->Source)) {
      DE.add<UnificationErrorDiagnostic>(C->Left->substitute(Solution), C->Right->substitute(Solution), C->Source);
    }
  }

  bool Checker::unify(Type* A, Type* B, Node* Source) {

    auto find = [&](auto Ty) {
      while (Ty->getKind() == TypeKind::Var) {
        auto Match = Solution.find(static_cast<TVar*>(Ty));
        if (Match == Solution.end()) {
          break;
        }
        Ty = Match->second;
      }
      return Ty;
    };

    A = find(A);
    B = find(B);

    if (llvm::isa<TVar>(A) && llvm::isa<TVar>(B)) {
      auto Var1 = static_cast<TVar*>(A);
      auto Var2 = static_cast<TVar*>(B);
      if (Var1->getVarKind() == VarKind::Rigid && Var2->getVarKind() == VarKind::Rigid) {
        if (Var1->Id != Var2->Id) {
          return false;
        }
        return true;
      }
      TVar* Dest;
      TVar* From;
      if (Var1->getVarKind() == VarKind::Rigid && Var2->getVarKind() == VarKind::Unification) {
        Dest = Var1;
        From = Var2;
      } else {
        // Only cases left are Var1 = Unification, Var2 = Rigid and Var1 = Unification, Var2 = Unification
        // Either way, Var1 is a good candidate for being unified away
        Dest = Var2;
        From = Var1;
      }
      Solution[From] = Dest;
      propagateClasses(From->Contexts, Dest, Source);
      return true;
    }

    if (llvm::isa<TVar>(A)) {
      auto TV = static_cast<TVar*>(A);
      if (TV->getVarKind() == VarKind::Rigid) {
        return false;
      }
      // Occurs check
      if (B->hasTypeVar(TV)) {
        // NOTE Just like GHC, we just display an error message indicating that
        //      A cannot match B, e.g. a cannot match [a]. It looks much better
        //      than obsure references to an occurs check
        return false;
      }
      Solution[TV] = B;
      if (!TV->Contexts.empty()) {
        propagateClasses(TV->Contexts, B, Source);
      }
      return true;
    }

    if (llvm::isa<TVar>(B)) {
      return unify(B, A, Source);
    }

    if (llvm::isa<TArrow>(A) && llvm::isa<TArrow>(B)) {
      auto C1 = ArrowCursor(static_cast<TArrow*>(A));
      auto C2 = ArrowCursor(static_cast<TArrow*>(B));
      for (;;) {
        auto T1 = C1.next();
        auto T2 = C2.next();
        if (T1 == nullptr && T2 == nullptr) {
          break;
        }
        if (T1 == nullptr || T2 == nullptr) {
          return false;
        }
        if (!unify(T1, T2, Source)) {
          return false;
        }
      }
      return true;
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
        return unify(Arr->ReturnType, B, Source);
      }
    }

    if (llvm::isa<TArrow>(B)) {
      return unify(B, A, Source);
    }

    if (llvm::isa<TTuple>(A) && llvm::isa<TTuple>(B)) {
      auto Tuple1 = static_cast<TTuple*>(A);
      auto Tuple2 = static_cast<TTuple*>(B);
      if (Tuple1->ElementTypes.size() != Tuple2->ElementTypes.size()) {
        return false;
      }
      auto Count = Tuple1->ElementTypes.size();
      bool Success = true;
      for (size_t I = 0; I < Count; I++) {
        if (!unify(Tuple1->ElementTypes[I], Tuple2->ElementTypes[I], Source)) {
          Success = false;
        }
      }
      return Success;
    }

    if (llvm::isa<TCon>(A) && llvm::isa<TCon>(B)) {
      auto Con1 = static_cast<TCon*>(A);
      auto Con2 = static_cast<TCon*>(B);
      if (Con1->Id != Con2->Id) {
        return false;
      }
      ZEN_ASSERT(Con1->Args.size() == Con2->Args.size());
      auto Count = Con1->Args.size();
      for (std::size_t I = 0; I < Count; I++) {
        if (!unify(Con1->Args[I], Con2->Args[I], Source)) {
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

  Type* Checker::getType(TypedNode *Node) {
    return Node->getType()->substitute(Solution);
  }

}

