
#pragma once

#include "zen/config.hpp"

#include "bolt/ByteString.hpp"
#include "bolt/Common.hpp"
#include "bolt/CST.hpp"
#include "bolt/Type.hpp"
#include "bolt/Support/Graph.hpp"

#include <cstdlib>
#include <unordered_map>
#include <unordered_set>
#include <vector>
#include <deque>

namespace bolt {

  std::string describe(const Type* Ty); // For debugging only

  class DiagnosticEngine;

  class Constraint;

  using ConstraintSet = std::vector<Constraint*>;

  enum class SchemeKind : unsigned char {
    Forall,
  };

  class Scheme {

    const SchemeKind Kind;

  protected:

    inline Scheme(SchemeKind Kind):
      Kind(Kind) {}

  public:

    inline SchemeKind getKind() const noexcept {
      return Kind;
    }

    virtual ~Scheme() {}

  };

  class Forall : public Scheme {
  public:

    TVSet* TVs;
    ConstraintSet* Constraints;
    class Type* Type;

    inline Forall(class Type* Type):
      Scheme(SchemeKind::Forall), TVs(new TVSet), Constraints(new ConstraintSet), Type(Type) {}

    inline Forall(
      TVSet* TVs,
      ConstraintSet* Constraints,
      class Type* Type
    ): Scheme(SchemeKind::Forall),
       TVs(TVs),
       Constraints(Constraints),
       Type(Type) {}

    static bool classof(const Scheme* Scm) {
      return Scm->getKind() == SchemeKind::Forall;
    }

  };

  using TypeEnv = std::unordered_map<ByteString, Scheme*>;

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

    inline CMany(ConstraintSet& Elements):
      Constraint(ConstraintKind::Many), Elements(Elements) {}

  };

  class CEmpty : public Constraint {
  public:

    inline CEmpty():
      Constraint(ConstraintKind::Empty) {}

  };

  using InferContextFlagsMask = unsigned;

  class InferContext {
  public:

    /**
     * A heap-allocated list of type variables that eventually will become part of a Forall scheme.
     */
    TVSet* TVs;

    /**
     * A heap-allocated list of constraints that eventually will become part of a Forall scheme.
     */
    ConstraintSet* Constraints;

    TypeEnv Env;

    void add(ByteString Name, Scheme* Scm) {
      // auto F = static_cast<Forall*>(Scm);
      // std::cerr << Name << " : forall ";
      // for (auto TV: *F->TVs) {
      //   std::cerr << describe(TV) << " ";
      // }
      // std::cerr << ". " << describe(F->Type) << "\n";
      Env.emplace(Name, Scm);
    }

    Type* ReturnType = nullptr;

    InferContext* Parent = nullptr;

  };

  class Checker {

    friend class Unifier;
    friend class UnificationFrame;

    const LanguageConfig& Config;
    DiagnosticEngine& DE;

    size_t NextConTypeId = 0;
    size_t NextTypeVarId = 0;

    Type* BoolType;
    Type* ListType;
    Type* IntType;
    Type* StringType;
    Type* UnitType;

    Graph<Node*> RefGraph;

    std::unordered_map<ByteString, std::vector<InstanceDeclaration*>> InstanceMap;

    /// Inference context management

    InferContext* ActiveContext;

    InferContext& getContext();
    void setContext(InferContext* Ctx);
    void popContext();

    void makeEqual(Type* A, Type* B, Node* Source);

    void addConstraint(Constraint* Constraint);

    /**
     * Get the return type for the current context. If none could be found, the
     * program will abort.
     */
    Type* getReturnType();

    /// Type inference

    void forwardDeclare(Node* Node);
    void forwardDeclareFunctionDeclaration(LetDeclaration* N, TVSet* TVs, ConstraintSet* Constraints);

    Type* inferExpression(Expression* Expression);
    Type* inferTypeExpression(TypeExpression* TE, bool IsPoly = true);
    Type* inferLiteral(Literal* Lit);
    Type* inferPattern(Pattern* Pattern, ConstraintSet* Constraints = new ConstraintSet, TVSet* TVs = new TVSet);

    void infer(Node* node);
    void inferFunctionDeclaration(LetDeclaration* N);
    void inferConstraintExpression(ConstraintExpression* C);

    /// Factory methods 

    Type* createConType(ByteString Name);
    Type* createTypeVar();
    Type* createRigidVar(ByteString Name);
    InferContext* createInferContext(
      InferContext* Parent = nullptr,
      TVSet* TVs = new TVSet,
      ConstraintSet* Constraints = new ConstraintSet
    );

    /// Environment manipulation

    Scheme* lookup(ByteString Name);

    /**
     * Looks up a type/variable and  ensures that it is a monomorphic type.
     *
     * This method is mainly syntactic sugar to make it clear in the code when a
     * monomorphic type is expected.
     *
     * Note that if the type is not monomorphic the program will abort with a
     * stack trace. It wil **not** print a user-friendly error message.
     *
     * \returns If the type/variable could not be found `nullptr` is returned.
     *          Otherwise, a [Type] is returned.
     */
    Type* lookupMono(ByteString Name);

    void addBinding(ByteString Name, Scheme* Scm);

    /// Constraint solving

    /**
     * The queue that is used during solving to store any unsolved constraints.
     */
    std::deque<class Constraint*> Queue;

    void solveEqual(CEqual* C);

    void solve(Constraint* Constraint);

    /// Helpers

    void populate(SourceFile* SF);

    /**
     * Verifies that type class signatures on type asserts in let-declarations
     * correctly declare the right type classes.
     */
    void checkTypeclassSigs(Node* N);

    Type* instantiate(Scheme* S, Node* Source);

    void initialize(Node* N);

  public:

    Checker(const LanguageConfig& Config, DiagnosticEngine& DE);

    /**
     * \internal
     */
    Type* simplifyType(Type* Ty);

    /**
     * \internal
     */
    Type* solveType(Type* Ty);

    void check(SourceFile* SF);

    inline Type* getBoolType() const {
      return BoolType;
    }

    inline Type* getStringType() const {
      return StringType;
    }

    inline Type* getIntType() const {
      return IntType;
    }

    Type* getType(TypedNode* Node);

  };

}
