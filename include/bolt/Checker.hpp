
#pragma once

#include <cstdlib>
#include <unordered_map>
#include <unordered_set>

#include "zen/tuple_hash.hpp"

#include "bolt/ByteString.hpp"
#include "bolt/CST.hpp"
#include "bolt/DiagnosticEngine.hpp"
#include "bolt/Type.hpp"

namespace bolt {

enum class ConstraintKind {
  TypesEqual,
};

class Constraint {

  ConstraintKind Kind;

protected:

  Constraint(ConstraintKind Kind):
    Kind(Kind) {}

public:

  inline ConstraintKind getKind() const {
    return Kind;
  }

};

class CTypesEqual : public Constraint {

  Type* A;
  Type* B;
  Node* Origin;

public:

  CTypesEqual(Type* A, Type* B, Node* Origin):
    Constraint(ConstraintKind::TypesEqual), A(A), B(B), Origin(Origin) {}

  Type* getLeft() const {
    return A;
  }

  Type* getRight() const {
    return B;
  }

  Node* getOrigin() const {
    return Origin;
  }

};

class TypeEnv {

  TypeEnv* Parent;

  std::unordered_map<std::tuple<ByteString, SymbolKind>, TypeScheme*> Mapping;

public:

  TypeEnv(TypeEnv* Parent = nullptr):
    Parent(Parent) {}

  void add(ByteString Name, Type* Ty, SymbolKind Kind);
  void add(ByteString Name, TypeScheme* Ty, SymbolKind Kind);

  bool hasVar(TVar* TV) const;

  TypeScheme* lookup(ByteString Name, SymbolKind Kind);

};

using ConstraintSet = std::vector<Constraint*>;

class Checker {

  DiagnosticEngine& DE;

  Type* IntType;
  Type* BoolType;
  Type* StringType;
  Type* UnitType;

public:

  Checker(DiagnosticEngine& DE);

  Type* getIntType() const {
    return IntType;
  }

  Type* getBoolType() const {
    return BoolType;
  }

  Type* getStringType() const {
    return StringType;
  }

  Type* getUnitType() const {
    return UnitType;
  }

  TVar* createTVar() {
     return new TVar();
  }

  Type* instantiate(TypeScheme* Scm);

  ConstraintSet visitPattern(Pattern* P, Type* Ty, TypeEnv& Out);

  ConstraintSet inferSourceFile(TypeEnv& Env, SourceFile* SF);

  ConstraintSet inferFunctionDeclaration(TypeEnv& Env, FunctionDeclaration* D);

  ConstraintSet inferVariableDeclaration(TypeEnv& Env, VariableDeclaration* Decl, Type* RetTy);

  ConstraintSet inferMany(TypeEnv& Env, std::vector<Node*>& N, Type* RetTy);

  ConstraintSet inferElement(TypeEnv& Env, Node* N, Type* RetTy);

  std::tuple<ConstraintSet, Type*> inferTypeExpr(TypeEnv& Env, TypeExpression* TE);

  std::tuple<ConstraintSet, Type*> inferExpr(TypeEnv& Env, Expression* Expr, Type* RetTy);

  ConstraintSet checkExpr(TypeEnv& Env, Expression* Expr, Type* Expected, Type* RetTy);

  void solve(const std::vector<Constraint*>& Constraints);

  void unifyTypeType(Type* A, Type* B, Node* Source);

  void run(SourceFile* SF);

  Type* getTypeOfNode(Node* N);

};

}
