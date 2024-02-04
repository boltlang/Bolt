
#include "bolt/Type.hpp"
#include <cwchar>
#include <sys/wait.h>
#include <vector>

#include "zen/range.hpp"

namespace bolt {

  bool TypeclassSignature::operator<(const TypeclassSignature& Other) const {
    if (Id < Other.Id) {
      return true;
    }
    ZEN_ASSERT(Params.size() == 1);
    ZEN_ASSERT(Other.Params.size() == 1);
    return Params[0]->asCon().Id < Other.Params[0]->asCon().Id;
  }

  bool TypeclassSignature::operator==(const TypeclassSignature& Other) const {
    ZEN_ASSERT(Params.size() == 1);
    ZEN_ASSERT(Other.Params.size() == 1);
    return Id == Other.Id && Params[0]->asCon().Id == Other.Params[0]->asCon().Id;
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

  bool TCon::operator==(const TCon& Other) const {
    return Id == Other.Id;
  }

  bool TApp::operator==(const TApp& Other) const {
    return *Op == *Other.Op && *Arg == *Other.Arg;
  }

  bool TVar::operator==(const TVar& Other) const {
    return Id == Other.Id;
  }

  bool TArrow::operator==(const TArrow& Other) const {
    return *ParamType == *Other.ParamType 
        && *ReturnType == *Other.ReturnType;
  }

  bool TTuple::operator==(const TTuple& Other) const {
    for (auto [T1, T2]: zen::zip(ElementTypes, Other.ElementTypes)) {
      if (*T1 != *T2) {
        return false;
      }
    }
    return true;
  }

  bool TNil::operator==(const TNil& Other) const {
    return true;
  }

  bool TField::operator==(const TField& Other) const {
    return Name == Other.Name && *Ty == *Other.Ty && *RestTy == *Other.RestTy;
  }

  bool TAbsent::operator==(const TAbsent& Other) const {
    return true;
  }

  bool TPresent::operator==(const TPresent& Other) const {
    return *Ty == *Other.Ty;
  }

  bool Type::operator==(const Type& Other) const {
    if (Kind != Other.Kind) {
      return false;
    }
    switch (Kind) {
      case TypeKind::Var:
        return Var == Other.Var;
      case TypeKind::Con:
        return Con == Other.Con;
      case TypeKind::Present:
        return Present == Other.Present;
      case TypeKind::Absent:
        return Absent == Other.Absent;
      case TypeKind::Arrow:
        return Arrow == Other.Arrow;
      case TypeKind::Field:
        return Field == Other.Field;
      case TypeKind::Nil:
        return Nil == Other.Nil;
      case TypeKind::Tuple:
        return Tuple == Other.Tuple;
      case TypeKind::App:
        return App == Other.App;
    }
    ZEN_UNREACHABLE
  }

  void Type::visitEachChild(std::function<void(Type*)> Proc) {
    switch (Kind) {
      case TypeKind::Var:
      case TypeKind::Absent:
      case TypeKind::Nil:
      case TypeKind::Con:
        break;
      case TypeKind::Arrow:
      {
        Proc(Arrow.ParamType);
        Proc(Arrow.ReturnType);
        break;
      }
      case TypeKind::Tuple:
      {
        for (auto I = 0; I < Tuple.ElementTypes.size(); ++I) {
          Proc(Tuple.ElementTypes[I]);
        }
        break;
      }
      case TypeKind::App:
      {
        Proc(App.Op);
        Proc(App.Arg);
        break;
      }
      case TypeKind::Field:
      {
        Proc(Field.Ty);
        Proc(Field.RestTy);
        break;
      }
      case TypeKind::Present:
      {
        Proc(Present.Ty);
        break;
      }
    }
  }

  Type* Type::rewrite(std::function<Type*(Type*)> Fn, bool Recursive) {
    auto Ty2 = Fn(this);
    if (this != Ty2) {
      if (Recursive) {
        return Ty2->rewrite(Fn, Recursive);
      }
      return Ty2;
    }
    switch (Kind) {
      case TypeKind::Var:
        return Ty2;
      case TypeKind::Arrow:
      {
        auto Arrow = Ty2->asArrow();
        bool Changed = false;
        Type* NewParamType = Arrow.ParamType->rewrite(Fn, Recursive);
        if (NewParamType != Arrow.ParamType) {
          Changed = true;
        }
        auto NewRetTy = Arrow.ReturnType->rewrite(Fn, Recursive);
        if (NewRetTy != Arrow.ReturnType) {
          Changed = true;
        }
        return Changed ? new Type(TArrow(NewParamType, NewRetTy)) : Ty2;
      }
      case TypeKind::Con:
        return Ty2;
      case TypeKind::App:
      {
        auto App = Ty2->asApp();
        auto NewOp = App.Op->rewrite(Fn, Recursive);
        auto NewArg = App.Arg->rewrite(Fn, Recursive);
        if (NewOp == App.Op && NewArg == App.Arg) {
          return Ty2;
        }
        return new Type(TApp(NewOp, NewArg));
      }
      case TypeKind::Tuple:
      {
        auto Tuple = Ty2->asTuple();
        bool Changed = false;
        std::vector<Type*> NewElementTypes;
        for (auto Ty: Tuple.ElementTypes) {
          auto NewElementType = Ty->rewrite(Fn, Recursive);
          if (NewElementType != Ty) {
            Changed = true;
          }
          NewElementTypes.push_back(NewElementType);
        }
        return Changed ? new Type(TTuple(NewElementTypes)) : Ty2;
      }
      case TypeKind::Nil:
        return Ty2;
      case TypeKind::Absent:
        return Ty2;
      case TypeKind::Field:
      {
        auto Field = Ty2->asField();
        bool Changed = false;
        auto NewTy = Field.Ty->rewrite(Fn, Recursive);
        if (NewTy != Field.Ty) {
          Changed = true;
        }
        auto NewRestTy = Field.RestTy->rewrite(Fn, Recursive);
        if (NewRestTy != Field.RestTy) {
          Changed = true;
        }
        return Changed ? new Type(TField(Field.Name, NewTy, NewRestTy)) : Ty2;
      }
      case TypeKind::Present:
      {
        auto Present = Ty2->asPresent();
        auto NewTy = Present.Ty->rewrite(Fn, Recursive);
        if (NewTy == Present.Ty) {
          return Ty2;
        }
        return new Type(TPresent(NewTy));
      }
    }
    ZEN_UNREACHABLE
  }

  Type* Type::substitute(const TVSub &Sub) {
    return rewrite([&](auto Ty) {
      if (Ty->isVar()) {
        auto Match = Sub.find(Ty);
        return Match != Sub.end() ? Match->second->substitute(Sub) : Ty;
      }
      return Ty;
    }, false);
  }

  Type* Type::resolve(const TypeIndex& Index) const noexcept {
    switch (Index.Kind) {
      case TypeIndexKind::PresentType:
        return this->asPresent().Ty;
      case TypeIndexKind::AppOpType:
        return this->asApp().Op;
      case TypeIndexKind::AppArgType:
        return this->asApp().Arg;
      case TypeIndexKind::TupleElement:
        return this->asTuple().ElementTypes[Index.I];
      case TypeIndexKind::ArrowParamType:
        return this->asArrow().ParamType;
      case TypeIndexKind::ArrowReturnType:
        return this->asArrow().ReturnType;
      case TypeIndexKind::FieldType:
        return this->asField().Ty;
      case TypeIndexKind::FieldRestType:
        return this->asField().RestTy;
      case TypeIndexKind::End:
        ZEN_UNREACHABLE
    }
    ZEN_UNREACHABLE
  }

  TVSet Type::getTypeVars() {
    TVSet Out;
    std::function<void(Type*)> visit = [&](Type* Ty) {
      if (Ty->isVar()) {
        Out.emplace(Ty);
        return;
      }
      Ty->visitEachChild(visit);
    };
    visit(this);
    return Out;
  }

  TypeIterator Type::begin() {
    return TypeIterator { this, getStartIndex() };
  }

  TypeIterator Type::end() {
    return TypeIterator { this, getEndIndex() };
  }

  TypeIndex Type::getStartIndex() const {
    switch (Kind) {
      case TypeKind::Arrow:
        return TypeIndex::forArrowParamType();
      case TypeKind::Tuple:
      {
        if (asTuple().ElementTypes.empty()) {
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

  TypeIndex Type::getEndIndex() const {
    return TypeIndex(TypeIndexKind::End);
  }

  bool Type::hasTypeVar(Type* TV) const {
    switch (Kind) {
      case TypeKind::Var:
        return Var.Id == TV->asVar().Id;
      case TypeKind::Con:
      case TypeKind::Absent:
      case TypeKind::Nil:
        return false;
      case TypeKind::App:
        return App.Op->hasTypeVar(TV) || App.Arg->hasTypeVar(TV);
      case TypeKind::Tuple:
        for (auto Ty: Tuple.ElementTypes) {
          if (Ty->hasTypeVar(TV)) {
            return true;
          }
        }
        return false;
      case TypeKind::Field:
        return Field.Ty->hasTypeVar(TV) || Field.RestTy->hasTypeVar(TV);
      case TypeKind::Arrow:
        return Arrow.ParamType->hasTypeVar(TV) || Arrow.ReturnType->hasTypeVar(TV);
      case TypeKind::Present:
        return Present.Ty->hasTypeVar(TV);
    }
    ZEN_UNREACHABLE
  }

}


