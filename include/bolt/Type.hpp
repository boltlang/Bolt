
#pragma once

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
    ArrowParamType,
    ArrowReturnType,
    ConArg,
    TupleElement,
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

    static TypeIndex forArrowReturnType() {
      return { TypeIndexKind::ArrowReturnType };
    }

    static TypeIndex forArrowParamType(std::size_t I) {
      return { TypeIndexKind::ArrowParamType, I };
    }

    static TypeIndex forConArg(std::size_t I) {
      return { TypeIndexKind::ConArg, I };
    }

    static TypeIndex forTupleElement(std::size_t I) {
      return { TypeIndexKind::TupleElement, I };
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
    Arrow,
    Tuple,
    TupleIndex,
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

    Type* substitute(const TVSub& Sub);

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

  // template<typename T>
  // struct DerefHash {
  //   std::size_t operator()(const T& Value) const noexcept {
  //     return std::hash<decltype(*Value)>{}(*Value);
  //   }
  // };

}
