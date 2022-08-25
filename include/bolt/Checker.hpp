
#pragma once

#include "zen/config.hpp"

#include "bolt/ByteString.hpp"

#include <stack>
#include <unordered_map>
#include <unordered_set>
#include <vector>
#include <optional>

namespace bolt {

  class DiagnosticEngine;
  class Node;
  class Expression;
  class TypeExpression;
  class Pattern;
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
    Tuple,
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

  class TTuple : public Type {
  public:

    std::vector<Type*> ElementTypes;

    inline TTuple(std::vector<Type*> ElementTypes):
      Type(TypeKind::Tuple), ElementTypes(ElementTypes) {}

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

  using ConstraintSet = std::vector<Constraint*>;

  class Forall {
  public:

    TVSet* TVs;
    ConstraintSet* Constraints;
    Type* Type;

    inline Forall(class Type* Type):
      TVs(new TVSet), Constraints(new ConstraintSet), Type(Type) {}

    inline Forall(
      TVSet& TVs,
      ConstraintSet& Constraints,
      class Type* Type
    ): TVs(&TVs),
       Constraints(&Constraints),
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

  using TypeEnv = std::unordered_map<ByteString, Scheme>;

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

    Constraint* substitute(const TVSub& Sub);

    virtual ~Constraint() {}

  };

  class CEqual : public Constraint {
  public:

    Type* Left;
    Type* Right;
    Node* Source;

    inline CEqual(Type* Left, Type* Right, Node* Source = nullptr):
      Constraint(ConstraintKind::Equal), Left(Left), Right(Right), Source(Source) {}

  };

  class CMany : public Constraint {
  public:

    ConstraintSet& Elements;

    inline CMany(ConstraintSet& Constraints):
      Constraint(ConstraintKind::Many), Elements(Constraints) {}

  };

  class CEmpty : public Constraint {
  public:

    inline CEmpty():
      Constraint(ConstraintKind::Empty) {}

  };

  class InferContext {
  public:

    TVSet TVs;
    ConstraintSet Constraints;
    TypeEnv Env;
    Type* ReturnType;

    InferContext* Parent;

    inline InferContext(InferContext* Parent, TVSet& TVs, ConstraintSet& Constraints, TypeEnv& Env, Type* ReturnType):
      Parent(Parent), TVs(TVs), Constraints(Constraints), Env(Env), ReturnType(ReturnType) {}

    inline InferContext(InferContext* Parent = nullptr):
      Parent(Parent), ReturnType(nullptr) {}

    void addConstraint(Constraint* C);

    void addBinding(ByteString Name, Scheme Scm);

    Type* lookupMono(ByteString Name);

    Scheme* lookup(ByteString Name);

  };

  class Checker {

    DiagnosticEngine& DE;

    size_t nextConTypeId = 0;
    size_t nextTypeVarId = 0;

    Type* BoolType;
    Type* IntType;
    Type* StringType;

    std::stack<InferContext> Contexts;

    void addConstraint(Constraint* Constraint);

    Type* inferExpression(Expression* Expression, InferContext& Ctx);
    Type* inferTypeExpression(TypeExpression* TE, InferContext& Ctx);

    void inferBindings(Pattern* Pattern, Type* T, InferContext& Ctx, ConstraintSet& Constraints, TVSet& Tvs);

    void infer(Node* node, InferContext& Ctx);

    TCon* createPrimConType();

    TVar* createTypeVar(InferContext& Ctx);

    Type* instantiate(Scheme& S, InferContext& Ctx, Node* Source);

    bool unify(Type* A, Type* B, TVSub& Solution);

    void solve(Constraint* Constraint);

  public:

    Checker(DiagnosticEngine& DE);

    void check(SourceFile* SF);

  };

}
