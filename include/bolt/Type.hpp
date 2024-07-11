
#pragma once

#include <cstddef>
#include <cwchar>
#include <vector>
#include <unordered_set>

#include "zen/config.hpp"

#include "bolt/ByteString.hpp"

namespace bolt {

enum class TypeIndexKind {
  AppOp,
  AppArg,
  ArrowLeft,
  ArrowRight,
  TupleElement,
  FieldElement,
  FieldRest,
  PresentElement,
  End,
};

class TypeIndex {

  friend class Type;

  TypeIndexKind Kind;

  union {
    std::size_t I;
  };

  TypeIndex(TypeIndexKind Kind):
    Kind(Kind) {}

  TypeIndex(TypeIndexKind Kind, std::size_t I):
    Kind(Kind), I(I) {}

public:

  static TypeIndex forAppOp() {
    return { TypeIndexKind::AppOp };
  }

  static TypeIndex forAppArg() {
    return { TypeIndexKind::AppArg };
  }

  static TypeIndex forArrowLeft() {
    return { TypeIndexKind::ArrowLeft };
  }

  static TypeIndex forArrowRight() {
    return { TypeIndexKind::ArrowRight };
  }

  static TypeIndex forTupleIndex(std::size_t I) {
    return { TypeIndexKind::TupleElement, I };
  }

};

using TypePath = std::vector<TypeIndex>;

enum class TypeKind {
  Var,
  Con,
  Fun,
  App,
};

class TVar;
class TCon;
class TFun;
class TApp;

class Type {
protected:

  TypeKind TK;

  Type(TypeKind TK):
    TK(TK) {}

public:

  virtual Type* find() const {
    return const_cast<Type*>(this);
  }

  inline TypeKind getKind() const {
    return TK;
  }

  bool isVar() const { 
    return TK == TypeKind::Var;
  }

  bool operator==(const Type& Other) const;

  std::string toString() const;

  Type* resolve(const TypePath& P);

  TVar* asVar();
  const TVar* asVar() const;

  TFun* asFun();
  const TFun* asFun() const;

  TCon* asCon();
  const TCon* asCon() const;

};

class TVar : public Type {

  Type* Parent = this;

public:

  TVar():
    Type(TypeKind::Var) {}

  void set(Type* Ty) {
    auto Root = find();
    // It is not possible to set a solution twice.
    ZEN_ASSERT(Root->isVar());
    static_cast<TVar*>(Root)->Parent = Ty;
  }

  Type* find() const override {
    TVar* Curr = const_cast<TVar*>(this);
    for (;;) {
      auto Keep = Curr->Parent;
      if (Keep == Curr || !Keep->isVar()) {
        return Keep;
      }
      auto Keep2 = static_cast<TVar*>(Keep);
      Curr->Parent = Keep2->Parent;
      Curr = Keep2;
    }
  }

  static constexpr const TypeKind Kind = TypeKind::Var;

};

class TCon : public Type {

  ByteString Name;

public:

  TCon(ByteString Name):
    Type(TypeKind::Con), Name(Name) {}

  ByteStringView getName() const {
    return Name;
  }

  static constexpr const TypeKind Kind = TypeKind::Con;

};

class TFun : public Type {

  Type* Left;
  Type* Right;

public:

  TFun(Type* Left, Type* Right):
    Type(TypeKind::Fun), Left(Left), Right(Right) {}

  Type* getLeft() const {
    return Left;
  }

  Type* getRight() const {
    return Right;
  }

  static constexpr const TypeKind Kind = TypeKind::Fun;

};

class TApp : public Type {

  Type* Left;
  Type* Right;

public:

  TApp(Type* Left, Type* Right):
    Type(TypeKind::App), Left(Left), Right(Right) {}

  Type* getLeft() const {
    return Left;
  }

  Type* getRight() const {
    return Right;
  }

  static constexpr const TypeKind Kind = TypeKind::App;

};

struct TypeScheme {

  std::unordered_set<TVar*> Unbound;
  Type* Ty;

  Type* getType() const {
    return Ty;
  }

};

class TypeVisitor {
public:

  void visit(Type* Ty);

  virtual void visitVar(TVar* TV) {

  }

  virtual void visitApp(TApp* App) {
    visit(App->getLeft());
    visit(App->getRight());
  }

  virtual void visitCon(TCon* Con) {

  }

  virtual void visitFun(TFun* Fun) {
    visit(Fun->getLeft());
    visit(Fun->getRight());
  }

};

}

