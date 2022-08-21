
#pragma once

#include "bolt/Diagnostics.hpp"
#include "zen/config.hpp"

#include "bolt/ByteString.hpp"

#include <unordered_map>
#include <unordered_set>
#include <vector>
#include <optional>

namespace bolt {

  class Node;
  class Expression;
  class SourceFile;

  class Type;
  class TVar;

  using TVSub = std::unordered_map<TVar*, Type*>;
  using TVSet = std::unordered_set<TVar*>;

  enum class TypeKind : unsigned char {
    Var,
    Con,
    Arrow,
    Any,
  };

  class Type {

    const TypeKind Kind;

  protected:

    inline Type(TypeKind Kind):
      Kind(Kind) {}

  public:

    bool hasTypeVar(const TVar* TV);

    Type* substitute(const TVSub& Sub);

    inline TypeKind getKind() const noexcept {
      return Kind;
    }

  };

  class TCon : public Type {
  public:

    const size_t Id;
    std::vector<Type*> Args;
    ByteString DisplayName;

    inline TCon(const size_t Id, std::vector<Type*> Args, ByteString DisplayName):
      Type(TypeKind::Con), Id(Id), Args(Args), DisplayName(DisplayName) {}

  };

  class TVar : public Type {
  public:

    const size_t Id;

    inline TVar(size_t Id):
      Type(TypeKind::Var), Id(Id) {}

  };

  class TArrow : public Type {
  public:

    std::vector<Type*> ParamTypes;
    Type* ReturnType;

    inline TArrow(
      std::vector<Type*> ParamTypes,
      Type* ReturnType
    ): Type(TypeKind::Arrow),
       ParamTypes(ParamTypes),
       ReturnType(ReturnType) {}

  };

  class TAny : public Type {
  public:

    inline TAny():
      Type(TypeKind::Any) {}

  };

  // template<typename T>
  // struct DerefHash {
  //   std::size_t operator()(const T& Value) const noexcept {
  //     return std::hash<decltype(*Value)>{}(*Value);
  //   }
  // };

  class Constraint;

  class Forall {
  public:

    TVSet TVs;
    std::vector<Constraint*> Constraints;
    Type* Type;

    inline Forall(class Type* Type):
      Type(Type) {}

    inline Forall(
      TVSet TVs,
      std::vector<Constraint*> Constraints,
      class Type* Type
    ): TVs(TVs),
       Constraints(Constraints),
       Type(Type) {}

  };

  enum class SchemeKind : unsigned char {
    Forall,
  };

  class Scheme {

    const SchemeKind Kind;

    union {
      Forall F;
    };

  public:

    inline Scheme(Forall F):
      Kind(SchemeKind::Forall), F(F) {}

    inline Scheme(const Scheme& Other):
      Kind(Other.Kind) {
        switch (Kind) {
          case SchemeKind::Forall:
            F = Other.F;
            break;
        }
      }


    inline Scheme(Scheme&& Other):
      Kind(std::move(Other.Kind)) {
        switch (Kind) {
          case SchemeKind::Forall:
            F = std::move(Other.F);
            break;
        }
      }

    template<typename T>
    T& as();

    template<>
    Forall& as<Forall>() {
      ZEN_ASSERT(Kind == SchemeKind::Forall);
      return F;
    }
    
    inline SchemeKind getKind() const noexcept {
      return Kind;
    }

    ~Scheme() {
      switch (Kind) {
        case SchemeKind::Forall:
          F.~Forall();
          break;
      }
    }

  };

  class TypeEnv {

    std::unordered_map<ByteString, Scheme> Mapping;

  public:

    void add(ByteString Name, Scheme S);

    Scheme* lookup(ByteString Name);

    Type* lookupMono(ByteString Name);

  };

  enum class ConstraintKind {
    Equal,
    Many,
    Empty,
  };

  class Constraint {

    const ConstraintKind Kind;

  public:

    inline Constraint(ConstraintKind Kind):
      Kind(Kind) {}

    inline ConstraintKind getKind() const noexcept {
      return Kind;
    }

    virtual ~Constraint() {}

  };

  using ConstraintSet = std::vector<Constraint*>;

  class CEqual : public Constraint {
  public:

    Type* Left;
    Type* Right;

    inline CEqual(Type* Left, Type* Right):
      Constraint(ConstraintKind::Equal), Left(Left), Right(Right) {}

  };

  class CMany : public Constraint {
  public:

    ConstraintSet Constraints;

    inline CMany(ConstraintSet Constraints):
      Constraint(ConstraintKind::Many), Constraints(Constraints) {}

  };

  class CEmpty : public Constraint {
  public:

    inline CEmpty():
      Constraint(ConstraintKind::Empty) {}

  };

  class InferContext {

    ConstraintSet& Constraints;

  public:

    TypeEnv& Env;

    inline InferContext(ConstraintSet& Constraints, TypeEnv& Env):
      Constraints(Constraints), Env(Env) {}

    void addConstraint(Constraint* C);

  };

  class Checker {

    DiagnosticEngine& DE;

    size_t nextConTypeId = 0;
    size_t nextTypeVarId = 0;

    Type* inferExpression(Expression* Expression, InferContext& Env);

    void infer(Node* node, InferContext& Env);

    TCon* createPrimConType();
    TVar* createTypeVar();

    Type* instantiate(Scheme& S);

    bool unify(Type* A, Type* B, TVSub& Solution);

    void solve(Constraint* Constraint);

  public:

    Checker(DiagnosticEngine& DE);

    void check(SourceFile* SF);

  };

}
