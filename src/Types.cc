
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
      case TypeIndexKind::AppOpType:
        Kind = TypeIndexKind::AppArgType;
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
      case TypeIndexKind::FieldType:
        Kind = TypeIndexKind::FieldRestType;
        break;
      case TypeIndexKind::FieldRestType:
      case TypeIndexKind::TupleIndexType:
      case TypeIndexKind::PresentType:
      case TypeIndexKind::AppArgType:
      case TypeIndexKind::ArrowReturnType:
        Kind = TypeIndexKind::End;
        break;
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

  Type* Type::rewrite(std::function<Type*(Type*)> Fn, bool Recursive) {
    auto Ty2 = Fn(this);
    if (!Recursive && this != Ty2) {
      return Ty2;
    }
    switch (Kind) {
      case TypeKind::Var:
        return Ty2;
      case TypeKind::Arrow:
      {
        auto Arrow = static_cast<TArrow*>(Ty2);
        bool Changed = false;
        std::vector<Type*> NewParamTypes;
        for (auto Ty: Arrow->ParamTypes) {
          auto NewParamType = Ty->rewrite(Fn);
          if (NewParamType != Ty) {
            Changed = true;
          }
          NewParamTypes.push_back(NewParamType);
        }
        auto NewRetTy = Arrow->ReturnType->rewrite(Fn);
        if (NewRetTy != Arrow->ReturnType) {
          Changed = true;
        }
        return Changed ? new TArrow(NewParamTypes, NewRetTy) : Ty2;
      }
      case TypeKind::Con:
        return Ty2;
      case TypeKind::App:
      {
        auto App = static_cast<TApp*>(Ty2);
        auto NewOp = App->Op->rewrite(Fn);
        auto NewArg = App->Arg->rewrite(Fn);
        if (NewOp == App->Op && NewArg == App->Arg) {
          return App;
        }
        return new TApp(NewOp, NewArg);
      }
      case TypeKind::TupleIndex:
      {
        auto Tuple = static_cast<TTupleIndex*>(Ty2);
        auto NewTy = Tuple->Ty->rewrite(Fn);
        return NewTy != Tuple->Ty ? new TTupleIndex(NewTy, Tuple->I) : Tuple;
      }
      case TypeKind::Tuple:
      {
        auto Tuple = static_cast<TTuple*>(Ty2);
        bool Changed = false;
        std::vector<Type*> NewElementTypes;
        for (auto Ty: Tuple->ElementTypes) {
          auto NewElementType = Ty->rewrite(Fn);
          if (NewElementType != Ty) {
            Changed = true;
          }
          NewElementTypes.push_back(NewElementType);
        }
        return Changed ? new TTuple(NewElementTypes) : Ty2;
      }
      case TypeKind::Nil:
        return Ty2;
      case TypeKind::Absent:
        return Ty2;
      case TypeKind::Field:
      {
        auto Field = static_cast<TField*>(Ty2);
        bool Changed = false;
        auto NewTy = Field->Ty->rewrite(Fn);
        if (NewTy != Field->Ty) {
          Changed = true;
        }
        auto NewRestTy = Field->RestTy->rewrite(Fn);
        if (NewRestTy != Field->RestTy) {
          Changed = true;
        }
        return Changed ? new TField(Field->Name, NewTy, NewRestTy) : Ty2;
      }
      case TypeKind::Present:
      {
        auto Present = static_cast<TPresent*>(Ty2);
        auto NewTy = Present->Ty->rewrite(Fn);
        if (NewTy == Present->Ty) {
          return Ty2;
        }
        return new TPresent(NewTy);
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
        break;
      case TypeKind::App:
      {
        auto App = static_cast<TApp*>(this);
        App->Op->addTypeVars(TVs);
        App->Arg->addTypeVars(TVs);
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
      case TypeKind::Nil:
        break;
      case TypeKind::Field:
      {
        auto Field = static_cast<TField*>(this);
        Field->Ty->addTypeVars(TVs);
        Field->Ty->addTypeVars(TVs);
        break;
      }
      case TypeKind::Present:
      {
        auto Present = static_cast<TPresent*>(this);
        Present->Ty->addTypeVars(TVs);
        break;
      }
      case TypeKind::Absent:
        break;
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
        return false;
      case TypeKind::App:
      {
        auto App = static_cast<TApp*>(this);
        return App->Op->hasTypeVar(TV) || App->Arg->hasTypeVar(TV);
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
      case TypeKind::Nil:
        return false;
      case TypeKind::Field:
      {
        auto Field = static_cast<TField*>(this);
        return Field->Ty->hasTypeVar(TV) || Field->RestTy->hasTypeVar(TV);
      }
      case TypeKind::Present:
      {
        auto Present = static_cast<TPresent*>(this);
        return Present->Ty->hasTypeVar(TV);
      }
      case TypeKind::Absent:
        return false;
    }
  }

  Type* Type::solve() {
    return rewrite([](auto Ty) {
      if (Ty->getKind() == TypeKind::Var) {
        return static_cast<TVar*>(Ty)->find();
      }
      return Ty;
    });
  }

  Type* Type::substitute(const TVSub &Sub) {
    return rewrite([&](auto Ty) {
      if (llvm::isa<TVar>(Ty)) {
        auto TV = static_cast<TVar*>(Ty);
        auto Match = Sub.find(TV);
        return Match != Sub.end() ? Match->second->substitute(Sub) : Ty;
      }
      return Ty;
    });
  }

  Type* Type::resolve(const TypeIndex& Index) const noexcept {
    switch (Index.Kind) {
      case TypeIndexKind::PresentType:
        return llvm::cast<TPresent>(this)->Ty;
      case TypeIndexKind::AppOpType:
        return llvm::cast<TApp>(this)->Op;
      case TypeIndexKind::AppArgType:
        return llvm::cast<TApp>(this)->Arg;
      case TypeIndexKind::TupleIndexType:
        return llvm::cast<TTupleIndex>(this)->Ty;
      case TypeIndexKind::TupleElement:
        return llvm::cast<TTuple>(this)->ElementTypes[Index.I];
      case TypeIndexKind::ArrowParamType:
        return llvm::cast<TArrow>(this)->ParamTypes[Index.I];
      case TypeIndexKind::ArrowReturnType:
        return llvm::cast<TArrow>(this)->ReturnType;
      case TypeIndexKind::FieldType:
        return llvm::cast<TField>(this)->Ty;
      case TypeIndexKind::FieldRestType:
        return llvm::cast<TField>(this)->RestTy;
      case TypeIndexKind::End:
        ZEN_UNREACHABLE
    }
    ZEN_UNREACHABLE
  }

  // bool Type::operator==(const Type& Other) const noexcept {
  //   switch (Kind) {
  //     case TypeKind::Var:
  //       if (Other.Kind != TypeKind::Var) {
  //         return false;
  //       }
  //       return static_cast<const TVar*>(this)->Id == static_cast<const TVar&>(Other).Id;
  //     case TypeKind::Tuple:
  //     {
  //       if (Other.Kind != TypeKind::Tuple) {
  //         return false;
  //       }
  //       auto A = static_cast<const TTuple&>(*this);
  //       auto B = static_cast<const TTuple&>(Other);
  //       if (A.ElementTypes.size() != B.ElementTypes.size()) {
  //         return false;
  //       }
  //       for (auto [T1, T2]: zen::zip(A.ElementTypes, B.ElementTypes)) {
  //         if (*T1 != *T2) {
  //           return false;
  //         }
  //       }
  //       return true;
  //     }
  //     case TypeKind::TupleIndex:
  //     {
  //       if (Other.Kind != TypeKind::TupleIndex) {
  //         return false;
  //       }
  //       auto A = static_cast<const TTupleIndex&>(*this);
  //       auto B = static_cast<const TTupleIndex&>(Other);
  //       return A.I == B.I && *A.Ty == *B.Ty;
  //     }
  //     case TypeKind::Con:
  //     {
  //       if (Other.Kind != TypeKind::Con) {
  //         return false;
  //       }
  //       auto A = static_cast<const TCon&>(*this);
  //       auto B = static_cast<const TCon&>(Other);
  //       if (A.Id != B.Id) {
  //         return false;
  //       }
  //       if (A.Args.size() != B.Args.size()) {
  //         return false;
  //       }
  //       for (auto [T1, T2]: zen::zip(A.Args, B.Args)) {
  //         if (*T1 != *T2) {
  //           return false;
  //         }
  //       }
  //       return true;
  //     }
  //     case TypeKind::Arrow:
  //     {
  //       if (Other.Kind != TypeKind::Arrow) {
  //         return false;
  //       }
  //       auto A = static_cast<const TArrow&>(*this);
  //       auto B = static_cast<const TArrow&>(Other);
  //       /* ArrowCursor C1 { &A }; */
  //       /* ArrowCursor C2 { &B }; */
  //       /* for (;;) { */
  //       /*   auto T1 = C1.next(); */
  //       /*   auto T2 = C2.next(); */
  //       /*   if (T1 == nullptr && T2 == nullptr) { */
  //       /*     break; */
  //       /*   } */
  //       /*   if (T1 == nullptr || T2 == nullptr || *T1 != *T2) { */
  //       /*     return false; */
  //       /*   } */
  //       /* } */
  //       if (A.ParamTypes.size() != B.ParamTypes.size()) {
  //         return false;
  //       }
  //       for (auto [T1, T2]: zen::zip(A.ParamTypes, B.ParamTypes)) {
  //         if (*T1 != *T2) {
  //           return false;
  //         }
  //       }
  //       return A.ReturnType != B.ReturnType;
  //     }
  //     case TypeKind::Absent:
  //       if (Other.Kind != TypeKind::Absent) {
  //         return false;
  //       }
  //       return true;
  //     case TypeKind::Nil:
  //       if (Other.Kind != TypeKind::Nil) {
  //         return false;
  //       }
  //       return true;
  //     case TypeKind::Present:
  //     {
  //       if (Other.Kind != TypeKind::Present) {
  //         return false;
  //       }
  //       auto A = static_cast<const TPresent&>(*this);
  //       auto B = static_cast<const TPresent&>(Other);
  //       return *A.Ty == *B.Ty;
  //     }
  //     case TypeKind::Field:
  //     {
  //       if (Other.Kind != TypeKind::Field) {
  //         return false;
  //       }
  //       auto A = static_cast<const TField&>(*this);
  //       auto B = static_cast<const TField&>(Other);
  //       return *A.Ty == *B.Ty && *A.RestTy == *B.RestTy;
  //     }
  //   }
  // }

  TypeIterator Type::begin() {
    return TypeIterator { this, getStartIndex() };
  }

  TypeIterator Type::end() {
    return TypeIterator { this, getEndIndex() };
  }

  TypeIndex Type::getStartIndex() {
    switch (Kind) {
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
      case TypeKind::Field:
        return TypeIndex::forFieldType();
      default:
        return TypeIndex(TypeIndexKind::End);
    }
  }

  TypeIndex Type::getEndIndex() {
    return TypeIndex(TypeIndexKind::End);
  }

  
  inline Type* TVar::find() {
    TVar* Curr = this;
    for (;;) {
      auto Keep = Curr->Parent;
      if (Keep->getKind() != TypeKind::Var || Keep == Curr) {
        return Keep;
      }
      auto TV = static_cast<TVar*>(Keep);
      Curr->Parent = TV->Parent;
      Curr = TV;
    }
  }

  void TVar::set(Type* Ty) {
    auto Root = find();
    // It is not possible to set a solution twice.
    ZEN_ASSERT(Root->getKind() == TypeKind::Var);
    static_cast<TVar*>(Root)->Parent = Ty;
  }

}
