
#include "llvm/Support/Casting.h"

#include "zen/config.hpp"
#include "zen/range.hpp"

#include "bolt/Type.hpp"

namespace bolt {

  bool TypeclassSignature::operator<(const TypeclassSignature& Other) const {
    if (Id < Other.Id) {
      return true;
    }
    ZEN_ASSERT(Params.size() == 1);
    ZEN_ASSERT(Other.Params.size() == 1);
    return Params[0]->Id < Other.Params[0]->Id;
  }

  bool TypeclassSignature::operator==(const TypeclassSignature& Other) const {
    ZEN_ASSERT(Params.size() == 1);
    ZEN_ASSERT(Other.Params.size() == 1);
    return Id == Other.Id && Params[0]->Id == Other.Params[0]->Id;
  }

  bool TypeIndex::operator==(const TypeIndex& Other) const noexcept {
    if (Kind != Other.Kind) {
      return false;
    }
    switch (Kind) {
      case TypeIndexKind::ConArg:
      case TypeIndexKind::ArrowParamType:
      case TypeIndexKind::TupleElement:
        return I == Other.I;
      default:
        return true;
    }
  }

  void TypeIndex::advance(const Type* Ty) {
    switch (Kind) {
      case TypeIndexKind::End:
        break;
      case TypeIndexKind::ArrowParamType:
      {
        auto Arrow = llvm::cast<TArrow>(Ty);
        if (I+1 < Arrow->ParamTypes.size()) {
          ++I;
        } else {
          Kind = TypeIndexKind::ArrowReturnType;
        }
        break;
      }
      case TypeIndexKind::ArrowReturnType:
        Kind = TypeIndexKind::End;
        break;
      case TypeIndexKind::ConArg:
      {
        auto Con = llvm::cast<TCon>(Ty);
        if (I+1 < Con->Args.size()) {
          ++I;
        } else {
          Kind = TypeIndexKind::End;
        }
        break;
      }
      case TypeIndexKind::TupleElement:
      {
        auto Tuple = llvm::cast<TTuple>(Ty);
        if (I+1 < Tuple->ElementTypes.size()) {
          ++I;
        } else {
          Kind = TypeIndexKind::End;
        }
        break;
      }
    }
  }

  void Type::addTypeVars(TVSet& TVs) {
    switch (Kind) {
      case TypeKind::Var:
        TVs.emplace(static_cast<TVar*>(this));
        break;
      case TypeKind::Arrow:
      {
        auto Arrow = static_cast<TArrow*>(this);
        for (auto Ty: Arrow->ParamTypes) {
          Ty->addTypeVars(TVs);
        }
        Arrow->ReturnType->addTypeVars(TVs);
        break;
      }
      case TypeKind::Con:
      {
        auto Con = static_cast<TCon*>(this);
        for (auto Ty: Con->Args) {
          Ty->addTypeVars(TVs);
        }
        break;
      }
      case TypeKind::TupleIndex:
      {
        auto Index = static_cast<TTupleIndex*>(this);
        Index->Ty->addTypeVars(TVs);
        break;
      }
      case TypeKind::Tuple:
      {
        auto Tuple = static_cast<TTuple*>(this);
        for (auto Ty: Tuple->ElementTypes) {
          Ty->addTypeVars(TVs);
        }
        break;
      }
    }
  }

  bool Type::hasTypeVar(const TVar* TV) {
    switch (Kind) {
      case TypeKind::Var:
        return static_cast<TVar*>(this)->Id == TV->Id;
      case TypeKind::Arrow:
      {
        auto Arrow = static_cast<TArrow*>(this);
        for (auto Ty: Arrow->ParamTypes) {
          if (Ty->hasTypeVar(TV)) {
            return true;
          }
        }
        return Arrow->ReturnType->hasTypeVar(TV);
      }
      case TypeKind::Con:
      {
        auto Con = static_cast<TCon*>(this);
        for (auto Ty: Con->Args) {
          if (Ty->hasTypeVar(TV)) {
            return true;
          }
        }
        return false;
      }
      case TypeKind::TupleIndex:
      {
        auto Index = static_cast<TTupleIndex*>(this);
        return Index->Ty->hasTypeVar(TV);
      }
      case TypeKind::Tuple:
      {
        auto Tuple = static_cast<TTuple*>(this);
        for (auto Ty: Tuple->ElementTypes) {
          if (Ty->hasTypeVar(TV)) {
            return true;
          }
        }
        return false;
      }
    }
  }

  Type* Type::substitute(const TVSub &Sub) {
    switch (Kind) {
      case TypeKind::Var:
      {
        auto TV = static_cast<TVar*>(this);
        auto Match = Sub.find(TV);
        return Match != Sub.end() ? Match->second->substitute(Sub) : this;
      }
      case TypeKind::Arrow:
      {
        auto Arrow = static_cast<TArrow*>(this);
        bool Changed = false;
        std::vector<Type*> NewParamTypes;
        for (auto Ty: Arrow->ParamTypes) {
          auto NewParamType = Ty->substitute(Sub);
          if (NewParamType != Ty) {
            Changed = true;
          }
          NewParamTypes.push_back(NewParamType);
        }
        auto NewRetTy = Arrow->ReturnType->substitute(Sub) ;
        if (NewRetTy != Arrow->ReturnType) {
          Changed = true;
        }
        return Changed ? new TArrow(NewParamTypes, NewRetTy) : this;
      }
      case TypeKind::Con:
      {
        auto Con = static_cast<TCon*>(this);
        bool Changed = false;
        std::vector<Type*> NewArgs;
        for (auto Arg: Con->Args) {
          auto NewArg = Arg->substitute(Sub);
          if (NewArg != Arg) {
            Changed = true;
          }
          NewArgs.push_back(NewArg);
        }
        return Changed ? new TCon(Con->Id, NewArgs, Con->DisplayName) : this;
      }
      case TypeKind::TupleIndex:
      {
        auto Tuple = static_cast<TTupleIndex*>(this);
        auto NewTy = Tuple->Ty->substitute(Sub);
        return NewTy != Tuple->Ty ? new TTupleIndex(NewTy, Tuple->I) : Tuple;
      }
      case TypeKind::Tuple:
      {
        auto Tuple = static_cast<TTuple*>(this);
        bool Changed = false;
        std::vector<Type*> NewElementTypes;
        for (auto Ty: Tuple->ElementTypes) {
          auto NewElementType = Ty->substitute(Sub);
          if (NewElementType != Ty) {
            Changed = true;
          }
          NewElementTypes.push_back(NewElementType);
        }
        return Changed ? new TTuple(NewElementTypes) : this;
      }
    }
  }

  Type* Type::resolve(const TypeIndex& Index) const noexcept {
    switch (Index.Kind) {
      case TypeIndexKind::ConArg:
        return llvm::cast<TCon>(this)->Args[Index.I];
      case TypeIndexKind::TupleElement:
        return llvm::cast<TTuple>(this)->ElementTypes[Index.I];
      case TypeIndexKind::ArrowParamType:
        return llvm::cast<TArrow>(this)->ParamTypes[Index.I];
      case TypeIndexKind::ArrowReturnType:
        return llvm::cast<TArrow>(this)->ReturnType;
      case TypeIndexKind::End:
        ZEN_UNREACHABLE
    }
    ZEN_UNREACHABLE
  }

  bool Type::operator==(const Type& Other) const noexcept {
    switch (Kind) {
      case TypeKind::Var:
        if (Other.Kind != TypeKind::Var) {
          return false;
        }
        return static_cast<const TVar*>(this)->Id == static_cast<const TVar&>(Other).Id;
      case TypeKind::Tuple:
      {
        if (Other.Kind != TypeKind::Tuple) {
          return false;
        }
        auto A = static_cast<const TTuple&>(*this);
        auto B = static_cast<const TTuple&>(Other);
        if (A.ElementTypes.size() != B.ElementTypes.size()) {
          return false;
        }
        for (auto [T1, T2]: zen::zip(A.ElementTypes, B.ElementTypes)) {
          if (*T1 != *T2) {
            return false;
          }
        }
        return true;
      }
      case TypeKind::TupleIndex:
      {
        if (Other.Kind != TypeKind::TupleIndex) {
          return false;
        }
        auto A = static_cast<const TTupleIndex&>(*this);
        auto B = static_cast<const TTupleIndex&>(Other);
        return A.I == B.I && *A.Ty == *B.Ty;
      }
      case TypeKind::Con:
      {
        if (Other.Kind != TypeKind::Con) {
          return false;
        }
        auto A = static_cast<const TCon&>(*this);
        auto B = static_cast<const TCon&>(Other);
        if (A.Id != B.Id) {
          return false;
        }
        if (A.Args.size() != B.Args.size()) {
          return false;
        }
        for (auto [T1, T2]: zen::zip(A.Args, B.Args)) {
          if (*T1 != *T2) {
            return false;
          }
        }
        return true;
      }
      case TypeKind::Arrow:
      {
        // FIXME Do we really need to 'curry' this type?
        if (Other.Kind != TypeKind::Arrow) {
          return false;
        }
        auto A = static_cast<const TArrow&>(*this);
        auto B = static_cast<const TArrow&>(Other);
        /* ArrowCursor C1 { &A }; */
        /* ArrowCursor C2 { &B }; */
        /* for (;;) { */
        /*   auto T1 = C1.next(); */
        /*   auto T2 = C2.next(); */
        /*   if (T1 == nullptr && T2 == nullptr) { */
        /*     break; */
        /*   } */
        /*   if (T1 == nullptr || T2 == nullptr || *T1 != *T2) { */
        /*     return false; */
        /*   } */
        /* } */
        if (A.ParamTypes.size() != B.ParamTypes.size()) {
          return false;
        }
        for (auto [T1, T2]: zen::zip(A.ParamTypes, B.ParamTypes)) {
          if (*T1 != *T2) {
            return false;
          }
        }
        return A.ReturnType != B.ReturnType;
      }
    }
  }

  TypeIterator Type::begin() {
    return TypeIterator { this, getStartIndex() };
  }

  TypeIterator Type::end() {
    return TypeIterator { this, getEndIndex() };
  }

  TypeIndex Type::getStartIndex() {
    switch (Kind) {
      case TypeKind::Con:
      {
        auto Con = static_cast<TCon*>(this);
        if (Con->Args.empty()) {
          return TypeIndex(TypeIndexKind::End);
        }
        return TypeIndex::forConArg(0);
      }
      case TypeKind::Arrow:
      {
        auto Arrow = static_cast<TArrow*>(this);
        if (Arrow->ParamTypes.empty()) {
          return TypeIndex::forArrowReturnType();
        }
        return TypeIndex::forArrowParamType(0);
      }
      case TypeKind::Tuple:
      {
        auto Tuple = static_cast<TTuple*>(this);
        if (Tuple->ElementTypes.empty()) {
          return TypeIndex(TypeIndexKind::End);
        }
        return TypeIndex::forTupleElement(0);
      }
      default:
        return TypeIndex(TypeIndexKind::End);
    }
  }

  TypeIndex Type::getEndIndex() {
    return TypeIndex(TypeIndexKind::End);
  }

}
