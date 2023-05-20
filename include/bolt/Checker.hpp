
#pragma once

#include "zen/config.hpp"

#include "bolt/ByteString.hpp"
#include "bolt/CST.hpp"
#include "bolt/Diagnostics.hpp"

#include <istream>
#include <unordered_map>
#include <unordered_set>
#include <vector>
#include <optional>

namespace bolt {

  class LanguageConfig {

    enum ConfigFlags {
      ConfigFlags_TypeVarsRequireForall = 1 << 0,
    };

    unsigned Flags;

  public:

    void setTypeVarsRequireForall(bool Enable) {
      if (Enable) {
        Flags |= ConfigFlags_TypeVarsRequireForall;
      } else {
        Flags |= ~ConfigFlags_TypeVarsRequireForall;
      }
    }

    bool typeVarsRequireForall() const noexcept {
      return Flags & ConfigFlags_TypeVarsRequireForall;
    }

  };

  class DiagnosticEngine;
  class Node;

  class Type;
  class TVar;

  using TVSub = std::unordered_map<TVar*, Type*>;
  using TVSet = std::unordered_set<TVar*>;

  using TypeclassContext = std::unordered_set<TypeclassId>;

  enum class TypeKind : unsigned char {
    Var,
    Con,
    Arrow,
    Tuple,
  };

  class Type {

    const TypeKind Kind;

  protected:

    inline Type(TypeKind Kind):
      Kind(Kind) {}

  public:

    bool hasTypeVar(const TVar* TV);

    void addTypeVars(TVSet& TVs);

    inline TVSet getTypeVars() {
      TVSet Out;
      addTypeVars(Out);
      return Out;
    }

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

    static bool classof(const Type* Ty) {
      return Ty->getKind() == TypeKind::Con;
    }

  };

  enum class VarKind {
    Rigid,
    Unification,
  };

  class TVar : public Type {
  public:

    const size_t Id;
    VarKind VK;

    TypeclassContext Contexts;

    inline TVar(size_t Id, VarKind VK):
      Type(TypeKind::Var), Id(Id), VK(VK) {}

    inline VarKind getVarKind() const noexcept {
      return VK;
    }

    static bool classof(const Type* Ty) {
      return Ty->getKind() == TypeKind::Var;
    }

  };

  class TVarRigid : public TVar {
  public:

    ByteString Name;

    inline TVarRigid(size_t Id, ByteString Name):
      TVar(Id, VarKind::Rigid), Name(Name) {}

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

    static bool classof(const Type* Ty) {
      return Ty->getKind() == TypeKind::Arrow;
    }

  };

  class TTuple : public Type {
  public:

    std::vector<Type*> ElementTypes;

    inline TTuple(std::vector<Type*> ElementTypes):
      Type(TypeKind::Tuple), ElementTypes(ElementTypes) {}

    static bool classof(const Type* Ty) {
      return Ty->getKind() == TypeKind::Tuple;
    }

  };

  // template<typename T>
  // struct DerefHash {
  //   std::size_t operator()(const T& Value) const noexcept {
  //     return std::hash<decltype(*Value)>{}(*Value);
  //   }
  // };

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

/*   class Scheme { */

/*     const SchemeKind Kind; */

/*   public: */

/*     inline Scheme(Forall F): */
/*       Kind(SchemeKind::Forall), F(F) {} */

/*     inline Scheme(const Scheme& Other): */
/*       Kind(Other.Kind) { */
/*         switch (Kind) { */
/*           case SchemeKind::Forall: */
/*             F = Other.F; */
/*             break; */
/*         } */
/*       } */


/*     inline Scheme(Scheme&& Other): */
/*       Kind(std::move(Other.Kind)) { */
/*         switch (Kind) { */
/*           case SchemeKind::Forall: */
/*             F = std::move(Other.F); */
/*             break; */
/*         } */
/*       } */

/*     inline SchemeKind getKind() const noexcept { */
/*       return Kind; */
/*     } */

/*     template<typename T> */
/*     T& as(); */

/*     template<> */
/*     Forall& as<Forall>() { */
/*       ZEN_ASSERT(Kind == SchemeKind::Forall); */
/*       return F; */
/*     } */

/*     ~Scheme() { */
/*       switch (Kind) { */
/*         case SchemeKind::Forall: */
/*           F.~Forall(); */
/*           break; */
/*       } */
/*     } */

/*   }; */

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

  enum {
    /**
     * Indicates that the typing environment of the current context will not
     * hold on to any bindings.
     *
     * Concretely, bindings that are assigned fall through to the parent
     * context, where this process is repeated until an environment is found
     * that is not pervious.
     */
    InferContextFlags_PerviousEnv = 1 << 0,
  };

  using InferContextFlagsMask = unsigned;

  class InferContext {

    InferContextFlagsMask Flags = 0;

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

    inline void setIsEnvPervious(bool Enable) noexcept {
      if (Enable) {
        Flags |= InferContextFlags_PerviousEnv;
      } else {
        Flags &= ~InferContextFlags_PerviousEnv;
      }
    }

    inline bool isEnvPervious() const noexcept {
      return Flags & InferContextFlags_PerviousEnv;
    }

    //inline InferContext(InferContext* Parent, TVSet& TVs, ConstraintSet& Constraints, TypeEnv& Env, Type* ReturnType):
    //  Parent(Parent), TVs(TVs), Constraints(Constraints), Env(Env), ReturnType(ReturnType) {}

  };

  class Checker {

    const LanguageConfig& Config;
    DiagnosticEngine& DE;

    size_t NextConTypeId = 0;
    size_t NextTypeVarId = 0;

    std::unordered_map<Node*, InferContext*> CallGraph;

    Type* BoolType;
    Type* IntType;
    Type* StringType;

    TVSub Solution;

    std::vector<InferContext*> Contexts;

    /**
     * Holds the current inferred type class contexts in a given LetDeclaration body.
     */
    // std::vector<TypeclassContext*> TCCs;

    InferContext& getContext();

    void addConstraint(Constraint* Constraint);
    void addClass(TypeclassSignature Sig);

    void forwardDeclare(Node* Node);

    Type* inferExpression(Expression* Expression);
    Type* inferTypeExpression(TypeExpression* TE);

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

    /* void addToTypeclassContexts(Node* N, std::vector<TypeclassContext>& Contexts); */

    std::unordered_map<ByteString, std::vector<InstanceDeclaration*>> InstanceMap;
    std::vector<TypeclassContext> findInstanceContext(TCon* Ty, TypeclassId& Class, Node* Source);
    void propagateClasses(TypeclassContext& Classes, Type* Ty, Node* Source);
    void propagateClassTycon(TypeclassId& Class, TCon* Ty, Node* Source);

    void checkTypeclassSigs(Node* N);

    bool unify(Type* A, Type* B, Node* Source);

    void solveCEqual(CEqual* C);
    void solve(Constraint* Constraint, TVSub& Solution);

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
