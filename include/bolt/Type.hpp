
#pragma once

#include <functional>
#include <type_traits>
#include <vector>
#include <unordered_map>
#include <unordered_set>

#include "bolt/ByteString.hpp"

namespace bolt {

  class Type;
  class TVar;

  using TypeclassId = ByteString;

  using TypeclassContext = std::unordered_set<TypeclassId>;

  struct TypeclassSignature {

    using TypeclassId = ByteString;
    TypeclassId Id;
    std::vector<TVar*> Params;

    bool operator<(const TypeclassSignature& Other) const;
    bool operator==(const TypeclassSignature& Other) const;

  };

  enum class TypeIndexKind {
    AppOpType,
    AppArgType,
    ArrowParamType,
    ArrowReturnType,
    TupleElement,
    FieldType,
    FieldRestType,
    TupleIndexType,
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

    static TypeIndex forArrowParamType(std::size_t I) {
      return { TypeIndexKind::ArrowParamType, I };
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

    static TypeIndex forTupleIndexType() {
      return { TypeIndexKind::TupleIndexType };
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

  using TVSub = std::unordered_map<TVar*, Type*>;
  using TVSet = std::unordered_set<TVar*>;

  enum class TypeKind : unsigned char {
    Var,
    Con,
    App,
    Arrow,
    Tuple,
    TupleIndex,
    Field,
    Nil,
    Absent,
    Present,
  };

  class Type {

    const TypeKind Kind;

  protected:

    inline Type(TypeKind Kind):
      Kind(Kind) {}

  public:

    inline TypeKind getKind() const noexcept {
      return Kind;
    }

    bool hasTypeVar(const TVar* TV);

    void addTypeVars(TVSet& TVs);

    inline TVSet getTypeVars() {
      TVSet Out;
      addTypeVars(Out);
      return Out;
    }

    /**
     * Rewrites the entire substructure of a type to another one.
     *
     * \param Recursive If true, a succesfull local rewritten type will be again
     *                  rewriten until it encounters some terminals.
     */
    Type* rewrite(std::function<Type*(Type*)> Fn, bool Recursive = false);

    Type* substitute(const TVSub& Sub);

    Type* solve();

    TypeIterator begin();
    TypeIterator end();

    TypeIndex getStartIndex();
    TypeIndex getEndIndex();

    Type* resolve(const TypeIndex& Index) const noexcept;

    Type* resolve(const TypePath& Path) noexcept {
      Type* Ty = this;
      for (auto El: Path) {
        Ty = Ty->resolve(El);
      }
      return Ty;
    }

    bool operator==(const Type& Other) const noexcept;

    bool operator!=(const Type& Other) const noexcept {
      return !(*this == Other);
    }

  };

  class TCon : public Type {
  public:

    const size_t Id;
    ByteString DisplayName;

    inline TCon(const size_t Id, ByteString DisplayName):
      Type(TypeKind::Con), Id(Id), DisplayName(DisplayName) {}

    static bool classof(const Type* Ty) {
      return Ty->getKind() == TypeKind::Con;
    }

  };

  class TApp : public Type {
  public:

    Type* Op;
    Type* Arg;

    inline TApp(Type* Op, Type* Arg):
      Type(TypeKind::App), Op(Op), Arg(Arg) {}

    static bool classof(const Type* Ty) {
      return Ty->getKind() == TypeKind::App;
    }

  };

  enum class VarKind {
    Rigid,
    Unification,
  };

  class TVar : public Type {

    Type* Parent = this;

  public:

    const size_t Id;
    VarKind VK;

    TypeclassContext Contexts;

    inline TVar(size_t Id, VarKind VK):
      Type(TypeKind::Var), Id(Id), VK(VK) {}

    inline VarKind getVarKind() const noexcept {
      return VK;
    }

    Type* find();

    void set(Type* Ty);

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

  class TTupleIndex : public Type {
  public:

    Type* Ty;
    std::size_t I;

    inline TTupleIndex(Type* Ty, std::size_t I):
      Type(TypeKind::TupleIndex), Ty(Ty), I(I) {}

    static bool classof(const Type* Ty) {
      return Ty->getKind() == TypeKind::TupleIndex;
    }

  };

  class TNil : public Type {
  public:

    inline TNil():
      Type(TypeKind::Nil) {}

    static bool classof(const Type* Ty) {
      return Ty->getKind() == TypeKind::Nil;
    }

  };

  class TField : public Type {
  public:

    ByteString Name;
    Type* Ty;
    Type* RestTy;

    inline TField(
      ByteString Name,
      Type* Ty,
      Type* RestTy
    ): Type(TypeKind::Field),
       Name(Name),
       Ty(Ty),
       RestTy(RestTy) {}

    static bool classof(const Type* Ty) {
      return Ty->getKind() == TypeKind::Field;
    }

  };

  class TAbsent : public Type {
  public:

    inline TAbsent():
      Type(TypeKind::Absent) {}

    static bool classof(const Type* Ty) {
      return Ty->getKind() == TypeKind::Absent;
    }

  };

  class TPresent : public Type {
  public:

    Type* Ty;

    inline TPresent(Type* Ty):
      Type(TypeKind::Present), Ty(Ty) {}

    static bool classof(const Type* Ty) {
      return Ty->getKind() == TypeKind::Present;
    }

  };

  template<bool IsConst>
  class TypeVisitorBase {
  protected:

    template<typename T>
    using C = std::conditional<IsConst, const T, T>::type;

    virtual void enterType(C<Type>* Ty) {}
    virtual void exitType(C<Type>* Ty) {}

    virtual void visitVarType(C<TVar>* Ty) {
      visitEachChild(Ty);
    }

    virtual void visitAppType(C<TApp>* Ty) {
      visitEachChild(Ty);
    }

    virtual void visitPresentType(C<TPresent>* Ty) {
      visitEachChild(Ty);
    }

    virtual void visitConType(C<TCon>* Ty) {
      visitEachChild(Ty);
    }

    virtual void visitArrowType(C<TArrow>* Ty) {
      visitEachChild(Ty);
    }

    virtual void visitTupleType(C<TTuple>* Ty) {
      visitEachChild(Ty);
    }

    virtual void visitTupleIndexType(C<TTupleIndex>* Ty) {
      visitEachChild(Ty);
    }

    virtual void visitAbsentType(C<TAbsent>* Ty) {
      visitEachChild(Ty);
    }

    virtual void visitFieldType(C<TField>* Ty) {
      visitEachChild(Ty);
    }

    virtual void visitNilType(C<TNil>* Ty) {
      visitEachChild(Ty);
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
          auto Arrow = static_cast<C<TArrow>*>(Ty);
          for (auto I = 0; I < Arrow->ParamTypes.size(); ++I) {
            visit(Arrow->ParamTypes[I]);
          }
          visit(Arrow->ReturnType);
          break;
        }
        case TypeKind::Tuple:
        {
          auto Tuple = static_cast<C<TTuple>*>(Ty);
          for (auto I = 0; I < Tuple->ElementTypes.size(); ++I) {
            visit(Tuple->ElementTypes[I]);
          }
          break;
        }
        case TypeKind::App:
        {
          auto App = static_cast<C<TApp>*>(Ty);
          visit(App->Op);
          visit(App->Arg);
          break;
        }
        case TypeKind::Field:
        {
          auto Field = static_cast<C<TField>*>(Ty);
          visit(Field->Ty);
          visit(Field->RestTy);
          break;
        }
        case TypeKind::Present:
        {
          auto Present = static_cast<C<TPresent>*>(Ty);
          visit(Present->Ty);
          break;
        }
        case TypeKind::TupleIndex:
        {
          auto Index = static_cast<C<TTupleIndex>*>(Ty);
          visit(Index->Ty);
          break;
        }
      }
    }

    void visit(C<Type>* Ty) {
      enterType(Ty);
      switch (Ty->getKind()) {
        case TypeKind::Present:
          visitPresentType(static_cast<C<TPresent>*>(Ty));
          break;
        case TypeKind::Absent:
          visitAbsentType(static_cast<C<TAbsent>*>(Ty));
          break;
        case TypeKind::Nil:
          visitNilType(static_cast<C<TNil>*>(Ty));
          break;
        case TypeKind::Field:
          visitFieldType(static_cast<C<TField>*>(Ty));
          break;
        case TypeKind::Con:
          visitConType(static_cast<C<TCon>*>(Ty));
          break;
        case TypeKind::Arrow:
          visitArrowType(static_cast<C<TArrow>*>(Ty));
          break;
        case TypeKind::Var:
          visitVarType(static_cast<C<TVar>*>(Ty));
          break;
        case TypeKind::Tuple:
          visitTupleType(static_cast<C<TTuple>*>(Ty));
          break;
        case TypeKind::App:
          visitAppType(static_cast<C<TApp>*>(Ty));
          break;
        case TypeKind::TupleIndex:
          visitTupleIndexType(static_cast<C<TTupleIndex>*>(Ty));
          break;
      }
      exitType(Ty);
    }

    virtual ~TypeVisitorBase() {}

  };

  using TypeVisitor = TypeVisitorBase<false>;
  using ConstTypeVisitor = TypeVisitorBase<true>;

  // template<typename T>
  // struct DerefHash {
  //   std::size_t operator()(const T& Value) const noexcept {
  //     return std::hash<decltype(*Value)>{}(*Value);
  //   }
  // };

}
