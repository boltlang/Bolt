
#pragma once

#include "zen/config.hpp"

#include "bolt/ByteString.hpp"
#include "bolt/Common.hpp"
#include "bolt/CST.hpp"
#include "bolt/Type.hpp"

#include <unordered_map>
#include <unordered_set>
#include <vector>
#include <deque>

namespace bolt {

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
    Class,
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

  class CClass : public Constraint {
  public:

    ByteString Name;
    std::vector<Type*> Types;

    inline CClass(ByteString Name, std::vector<Type*> Types):
      Constraint(ConstraintKind::Class), Name(Name), Types(Types) {}

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

    Type* ReturnType = nullptr;
    std::vector<TypeclassSignature> Classes;

    //inline InferContext(InferContext* Parent, TVSet& TVs, ConstraintSet& Constraints, TypeEnv& Env, Type* ReturnType):
    //  Parent(Parent), TVs(TVs), Constraints(Constraints), Env(Env), ReturnType(ReturnType) {}

  };

  class Checker {

    const LanguageConfig& Config;
    DiagnosticEngine& DE;

    size_t NextConTypeId = 0;
    size_t NextTypeVarId = 0;

    std::unordered_map<Node*, InferContext*> CallGraph;

    std::unordered_map<ByteString, std::vector<InstanceDeclaration*>> InstanceMap;

    Type* BoolType;
    Type* IntType;
    Type* StringType;

    TVSub Solution;

    /**
     * The queue that is used during solving to store any unsolved constraints.
     */
    std::deque<class Constraint*> Queue;

    /**
     * Pointer to the current constraint being unified.
     */
    CEqual* C;

    std::vector<InferContext*> Contexts;

    InferContext& getContext();

    void addConstraint(Constraint* Constraint);
    void addClass(TypeclassSignature Sig);

    void forwardDeclare(Node* Node);

    Type* inferExpression(Expression* Expression);
    Type* inferTypeExpression(TypeExpression* TE);
    Type* inferLiteral(Literal* Lit);

    void inferBindings(Pattern* Pattern, Type* T, ConstraintSet* Constraints, TVSet* TVs);
    void inferBindings(Pattern* Pattern, Type* T);

    void infer(Node* node);

    Constraint* convertToConstraint(ConstraintExpression* C);

    TCon* createPrimConType();
    TVar* createTypeVar();
    TVarRigid* createRigidVar(ByteString Name);
    InferContext* createInferContext();

    void addBinding(ByteString Name, Scheme* Scm);

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

    InferContext* lookupCall(Node* Source, SymbolPath Path);

    /**
     * Get the return type for the current context. If none could be found, the program will abort.
     */
    Type* getReturnType();

    Type* instantiate(Scheme* S, Node* Source);

    std::vector<TypeclassContext> findInstanceContext(TCon* Ty, TypeclassId& Class);
    void propagateClasses(TypeclassContext& Classes, Type* Ty);
    void propagateClassTycon(TypeclassId& Class, TCon* Ty);

    Type* simplify(Type* Ty);

    Type* find(Type* Ty);

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
    void join(TVar* A, Type* B);

    Type* OrigLeft;
    Type* OrigRight;
    TypePath LeftPath;
    TypePath RightPath;
    Node* Source;

    bool unify(Type* A, Type* B);

    void unifyError();
    void solveCEqual(CEqual* C);

    void solve(Constraint* Constraint, TVSub& Solution);

    /**
     * Verifies that type class signatures on type asserts in let-declarations
     * correctly declare the right type classes.
     */
    void checkTypeclassSigs(Node* N);

  public:

    Checker(const LanguageConfig& Config, DiagnosticEngine& DE);

    void check(SourceFile* SF);

    inline Type* getBoolType() {
      return BoolType;
    }

    inline Type* getStringType() {
      return StringType;
    }

    inline Type* getIntType() {
      return IntType;
    }

    Type* getType(TypedNode* Node);

  };

}
