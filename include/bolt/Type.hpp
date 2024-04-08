
#pragma once

#include <functional>
#include <optional>
#include <unistd.h>
#include <unordered_set>
#include <unordered_map>
#include <vector>

#include "zen/config.hpp"

#include "bolt/CST.hpp"
#include "bolt/ByteString.hpp"

namespace bolt {

class Type;
class TCon;

using TypeclassId = ByteString;

using TypeclassContext = std::unordered_set<TypeclassId>;

struct TypeclassSignature {

  using TypeclassId = ByteString;
  TypeclassId Id;
  std::vector<Type*> Params;

  bool operator<(const TypeclassSignature& Other) const;
  bool operator==(const TypeclassSignature& Other) const;

};

struct TypeSig {
  Type* Orig;
  Type* Op;
  std::vector<Type*> Args;
};

enum class TypeIndexKind {
  AppOpType,
  AppArgType,
  ArrowParamType,
  ArrowReturnType,
  TupleElement,
  FieldType,
  FieldRestType,
  PresentType,
  End,
};

class TypeIndex {
protected:

  friend class Type;
  friend class TypeIterator;

  TypeIndexKind Kind;

  union {
    std::size_t I;
  };

  TypeIndex(TypeIndexKind Kind):
    Kind(Kind) {}

  TypeIndex(TypeIndexKind Kind, std::size_t I):
    Kind(Kind), I(I) {}

public:

  bool operator==(const TypeIndex& Other) const noexcept;

  void advance(const Type* Ty);

  static TypeIndex forFieldType() {
    return { TypeIndexKind::FieldType };
  }

  static TypeIndex forFieldRest() {
    return { TypeIndexKind::FieldRestType };
  }

  static TypeIndex forArrowParamType() {
    return { TypeIndexKind::ArrowParamType };
  }

  static TypeIndex forArrowReturnType() {
    return { TypeIndexKind::ArrowReturnType };
  }

  static TypeIndex forTupleElement(std::size_t I) {
    return { TypeIndexKind::TupleElement, I };
  }

  static TypeIndex forAppOpType() {
    return { TypeIndexKind::AppOpType };
  }

  static TypeIndex forAppArgType() {
    return { TypeIndexKind::AppArgType };
  }

  static TypeIndex forPresentType() {
    return { TypeIndexKind::PresentType };
  }

};

class TypeIterator {

  friend class Type;

  Type* Ty;
  TypeIndex Index;

  TypeIterator(Type* Ty, TypeIndex Index):
    Ty(Ty), Index(Index) {}

public:

  TypeIterator& operator++() noexcept {
    Index.advance(Ty);
    return *this;
  }

  bool operator==(const TypeIterator& Other) const noexcept {
    return Ty == Other.Ty && Index == Other.Index;
  }

  Type* operator*() {
    return Ty;
  }

  TypeIndex getIndex() const noexcept {
    return Index;
  }

};

using TypePath = std::vector<TypeIndex>;

using TVSub = std::unordered_map<Type*, Type*>;
using TVSet = std::unordered_set<Type*>;

enum class TypeKind : unsigned char {
  Var,
  Con,
  App,
  Arrow,
  Tuple,
  Field,
  Nil,
  Absent,
  Present,
};

class Type;

struct TCon {
  size_t Id;
  ByteString DisplayName;

  bool operator==(const TCon& Other) const;

};

struct TApp {
  Type* Op;
  Type* Arg;

  bool operator==(const TApp& Other) const;

};

enum class VarKind {
  Rigid,
  Unification,
};

struct TVar {
  VarKind VK;
  size_t Id;
  TypeclassContext Context;
  std::optional<ByteString> Name;
  std::optional<TypeclassContext> Provided;

  VarKind getKind() const {
    return VK;
  }

  bool isUni() const {
    return VK == VarKind::Unification;
  }

  bool isRigid() const {
    return VK == VarKind::Rigid;
  }

  bool operator==(const TVar& Other) const;

};

struct TArrow {
  Type* ParamType;
  Type* ReturnType;

  bool operator==(const TArrow& Other) const;

};

struct TTuple {
  std::vector<Type*> ElementTypes;

  bool operator==(const TTuple& Other) const;

};

struct TNil {
  bool operator==(const TNil& Other) const;
};

struct TField {
  ByteString Name;
  Type* Ty;
  Type* RestTy;
  bool operator==(const TField& Other) const;
};

struct TAbsent {
  bool operator==(const TAbsent& Other) const;
};

struct TPresent {
  Type* Ty;
  bool operator==(const TPresent& Other) const;
};

struct Type {

  TypeKind Kind;

  Type* Parent = this;

  union {
    TCon Con;
    TApp App;
    TVar Var;
    TArrow Arrow;
    TTuple Tuple;
    TNil Nil;
    TField Field;
    TAbsent Absent;
    TPresent Present;
  };

  Type(TCon&& Con):
    Kind(TypeKind::Con), Con(std::move(Con)) {};

  Type(TApp&& App):
    Kind(TypeKind::App), App(std::move(App)) {};

  Type(TVar&& Var):
    Kind(TypeKind::Var), Var(std::move(Var)) {};

  Type(TArrow&& Arrow):
    Kind(TypeKind::Arrow), Arrow(std::move(Arrow)) {};

  Type(TTuple&& Tuple):
    Kind(TypeKind::Tuple), Tuple(std::move(Tuple)) {};

  Type(TNil&& Nil):
    Kind(TypeKind::Nil), Nil(std::move(Nil)) {};

  Type(TField&& Field):
    Kind(TypeKind::Field), Field(std::move(Field)) {};

  Type(TAbsent&& Absent):
    Kind(TypeKind::Absent), Absent(std::move(Absent)) {};

  Type(TPresent&& Present):
    Kind(TypeKind::Present), Present(std::move(Present)) {};

  Type(const Type& Other): Kind(Other.Kind) {
    switch (Kind) {
      case TypeKind::Con:
        new (&Con)TCon(Other.Con);
        break;
      case TypeKind::App:
        new (&App)TApp(Other.App);
        break;
      case TypeKind::Var:
        new (&Var)TVar(Other.Var);
        break;
      case TypeKind::Arrow:
        new (&Arrow)TArrow(Other.Arrow);
        break;
      case TypeKind::Tuple:
        new (&Tuple)TTuple(Other.Tuple);
        break;
      case TypeKind::Nil:
        new (&Nil)TNil(Other.Nil);
        break;
      case TypeKind::Field:
        new (&Field)TField(Other.Field);
        break;
      case TypeKind::Absent:
        new (&Absent)TAbsent(Other.Absent);
        break;
      case TypeKind::Present:
        new (&Present)TPresent(Other.Present);
        break;
    }
  }

  Type(Type&& Other): Kind(std::move(Other.Kind)) {
    switch (Kind) {
      case TypeKind::Con:
        new (&Con)TCon(std::move(Other.Con));
        break;
      case TypeKind::App:
        new (&App)TApp(std::move(Other.App));
        break;
      case TypeKind::Var:
        new (&Var)TVar(std::move(Other.Var));
        break;
      case TypeKind::Arrow:
        new (&Arrow)TArrow(std::move(Other.Arrow));
        break;
      case TypeKind::Tuple:
        new (&Tuple)TTuple(std::move(Other.Tuple));
        break;
      case TypeKind::Nil:
        new (&Nil)TNil(std::move(Other.Nil));
        break;
      case TypeKind::Field:
        new (&Field)TField(std::move(Other.Field));
        break;
      case TypeKind::Absent:
        new (&Absent)TAbsent(std::move(Other.Absent));
        break;
      case TypeKind::Present:
        new (&Present)TPresent(std::move(Other.Present));
        break;
    }
  }

  TypeKind getKind() const {
    return Kind;
  }

  bool isVarRigid() const {
    return Kind == TypeKind::Var
        && asVar().getKind() == VarKind::Rigid;
  }

  bool isVar() const {
    return Kind == TypeKind::Var;
  }

  TVar& asVar() {
    ZEN_ASSERT(Kind == TypeKind::Var);
    return Var;
  }

  const TVar& asVar() const {
    ZEN_ASSERT(Kind == TypeKind::Var);
    return Var;
  }

  bool isApp() const {
    return Kind == TypeKind::App;
  }

  TApp& asApp() {
    ZEN_ASSERT(Kind == TypeKind::App);
    return App;
  }

  const TApp& asApp() const {
    ZEN_ASSERT(Kind == TypeKind::App);
    return App;
  }

  bool isCon() const {
    return Kind == TypeKind::Con;
  }

  TCon& asCon() {
    ZEN_ASSERT(Kind == TypeKind::Con);
    return Con;
  }

  const TCon& asCon() const {
    ZEN_ASSERT(Kind == TypeKind::Con);
    return Con;
  }

  bool isArrow() const {
    return Kind == TypeKind::Arrow;
  }

  TArrow& asArrow() {
    ZEN_ASSERT(Kind == TypeKind::Arrow);
    return Arrow;
  }

  const TArrow& asArrow() const {
    ZEN_ASSERT(Kind == TypeKind::Arrow);
    return Arrow;
  }

  bool isTuple() const {
    return Kind == TypeKind::Tuple;
  }

  TTuple& asTuple() {
    ZEN_ASSERT(Kind == TypeKind::Tuple);
    return Tuple;
  }

  const TTuple& asTuple() const {
    ZEN_ASSERT(Kind == TypeKind::Tuple);
    return Tuple;
  }

  bool isField() const {
    return Kind == TypeKind::Field;
  }

  TField& asField() {
    ZEN_ASSERT(Kind == TypeKind::Field);
    return Field;
  }

  const TField& asField() const {
    ZEN_ASSERT(Kind == TypeKind::Field);
    return Field;
  }

  bool isAbsent() const {
    return Kind == TypeKind::Absent;
  }

  TAbsent& asAbsent() {
    ZEN_ASSERT(Kind == TypeKind::Absent);
    return Absent;
  }
  const TAbsent& asAbsent() const {
    ZEN_ASSERT(Kind == TypeKind::Absent);
    return Absent;
  }

  bool isPresent() const {
    return Kind == TypeKind::Present;
  }

  TPresent& asPresent() {
    ZEN_ASSERT(Kind == TypeKind::Present);
    return Present;
  }
  const TPresent& asPresent() const {
    ZEN_ASSERT(Kind == TypeKind::Present);
    return Present;
  }

  bool isNil() const {
    return Kind == TypeKind::Nil;
  }

  TNil& asNil() {
    ZEN_ASSERT(Kind == TypeKind::Nil);
    return Nil;
  }
  const TNil& asNil() const {
    ZEN_ASSERT(Kind == TypeKind::Nil);
    return Nil;
  }

  Type* rewrite(std::function<Type*(Type*)> Fn, bool Recursive = true);

  Type* resolve(const TypeIndex& Index) const noexcept;

  Type* resolve(const TypePath& Path) noexcept {
    Type* Ty = this;
    for (auto El: Path) {
      Ty = Ty->resolve(El);
    }
    return Ty;
  }

  void set(Type* Ty) {
    auto Root = find();
    // It is not possible to set a solution twice.
    if (isVar()) {
      ZEN_ASSERT(Root->isVar());
    }
    Root->Parent = Ty;
  }

  Type* find() const {
    Type* Curr = const_cast<Type*>(this);
    for (;;) {
      auto Keep = Curr->Parent;
      if (Keep == Curr) {
        return Keep;
      }
      Curr->Parent = Keep->Parent;
      Curr = Keep;
    }
  }

  bool operator==(const Type& Other) const;

  void destroy() {
    switch (Kind) {
      case TypeKind::Con:
        App.~TApp();
        break;
      case TypeKind::App:
        App.~TApp();
        break;
      case TypeKind::Var:
        Var.~TVar();
        break;
      case TypeKind::Arrow:
        Arrow.~TArrow();
        break;
      case TypeKind::Tuple:
        Tuple.~TTuple();
        break;
      case TypeKind::Nil:
        Nil.~TNil();
        break;
      case TypeKind::Field:
        Field.~TField();
        break;
      case TypeKind::Absent:
        Absent.~TAbsent();
        break;
      case TypeKind::Present:
        Present.~TPresent();
        break;
    }
  }

  Type& operator=(Type& Other) {
    destroy();
    Kind = Other.Kind;
    switch (Kind) {
      case TypeKind::Con:
        App = Other.App;
        break;
      case TypeKind::App:
        App = Other.App;
        break;
      case TypeKind::Var:
        Var = Other.Var;
        break;
      case TypeKind::Arrow:
        Arrow = Other.Arrow;
        break;
      case TypeKind::Tuple:
        Tuple = Other.Tuple;
        break;
      case TypeKind::Nil:
        Nil = Other.Nil;
        break;
      case TypeKind::Field:
        Field = Other.Field;
        break;
      case TypeKind::Absent:
        Absent = Other.Absent;
        break;
      case TypeKind::Present:
        Present = Other.Present;
        break;
    }
    return *this;
  }

  bool hasTypeVar(Type* TV) const;

  TypeIterator begin();
  TypeIterator end();

  TypeIndex getStartIndex() const;
  TypeIndex getEndIndex() const;

  Type* substitute(const TVSub& Sub);

  void visitEachChild(std::function<void(Type*)> Proc);

  TVSet getTypeVars();

  ~Type() {
    destroy();
  }

  static Type* buildArrow(std::vector<Type*> ParamTypes, Type* ReturnType) {
    Type* Curr = ReturnType;
    for (auto Iter = ParamTypes.rbegin(); Iter != ParamTypes.rend(); ++Iter) {
      Curr = new Type(TArrow(*Iter, Curr));
    }
    return Curr;
  }

};

template<bool IsConst>
class TypeVisitorBase {
protected:

  template<typename T>
  using C = std::conditional<IsConst, const T, T>::type;

  virtual void enterType(C<Type>* Ty) {}
  virtual void exitType(C<Type>* Ty) {}

  // virtual void visitType(C<Type>* Ty) {
  //   visitEachChild(Ty);
  // }

  virtual void visitVarType(C<TVar>& Ty) {
  }

  virtual void visitAppType(C<TApp>& Ty) {
    visit(Ty.Op);
    visit(Ty.Arg);
  }

  virtual void visitPresentType(C<TPresent>& Ty) {
    visit(Ty.Ty);
  }

  virtual void visitConType(C<TCon>& Ty) {
  }

  virtual void visitArrowType(C<TArrow>& Ty) {
    visit(Ty.ParamType);
    visit(Ty.ReturnType);
  }

  virtual void visitTupleType(C<TTuple>& Ty) {
    for (auto ElTy: Ty.ElementTypes) {
      visit(ElTy);
    }
  }

  virtual void visitAbsentType(C<TAbsent>& Ty) {
  }

  virtual void visitFieldType(C<TField>& Ty) {
    visit(Ty.Ty);
    visit(Ty.RestTy);
  }

  virtual void visitNilType(C<TNil>& Ty) {
  }

public:

  void visitEachChild(C<Type>* Ty) {
    switch (Ty->getKind()) {
      case TypeKind::Var:
      case TypeKind::Absent:
      case TypeKind::Nil:
      case TypeKind::Con:
        break;
      case TypeKind::Arrow:
      {
        auto& Arrow = Ty->asArrow();
        visit(Arrow->ParamType);
        visit(Arrow->ReturnType);
        break;
      }
      case TypeKind::Tuple:
      {
        auto& Tuple = Ty->asTuple();
        for (auto I = 0; I < Tuple->ElementTypes.size(); ++I) {
          visit(Tuple->ElementTypes[I]);
        }
        break;
      }
      case TypeKind::App:
      {
        auto& App = Ty->asApp();
        visit(App->Op);
        visit(App->Arg);
        break;
      }
      case TypeKind::Field:
      {
        auto& Field = Ty->asField();
        visit(Field->Ty);
        visit(Field->RestTy);
        break;
      }
      case TypeKind::Present:
      {
        auto& Present = Ty->asPresent();
        visit(Present->Ty);
        break;
      }
    }
  }

  void visit(C<Type>* Ty) {

    // Always look at the most solved solution
    Ty = Ty->find();

    enterType(Ty);
    switch (Ty->getKind()) {
      case TypeKind::Present:
        visitPresentType(Ty->asPresent());
        break;
      case TypeKind::Absent:
        visitAbsentType(Ty->asAbsent());
        break;
      case TypeKind::Nil:
        visitNilType(Ty->asNil());
        break;
      case TypeKind::Field:
        visitFieldType(Ty->asField());
        break;
      case TypeKind::Con:
        visitConType(Ty->asCon());
        break;
      case TypeKind::Arrow:
        visitArrowType(Ty->asArrow());
        break;
      case TypeKind::Var:
        visitVarType(Ty->asVar());
        break;
      case TypeKind::Tuple:
        visitTupleType(Ty->asTuple());
        break;
      case TypeKind::App:
        visitAppType(Ty->asApp());
        break;
    }
    exitType(Ty);
  }

  virtual ~TypeVisitorBase() {}

};

using TypeVisitor = TypeVisitorBase<false>;
using ConstTypeVisitor = TypeVisitorBase<true>;

}
