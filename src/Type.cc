
#include "zen/config.hpp"

#include "bolt/Type.hpp"

namespace bolt {

Type* Type::resolve(const TypePath& P) {
  auto Ty = this;
  for (auto& Index: P) {
    switch (Index.Kind) {
       case TypeIndexKind::AppOp:
          Ty = static_cast<TApp*>(Ty)->getLeft();
          break;
      case TypeIndexKind::AppArg:
          Ty = static_cast<TApp*>(Ty)->getRight();
          break;
      case TypeIndexKind::ArrowLeft:
          Ty = static_cast<TFun*>(Ty)->getLeft();
          break;
      case TypeIndexKind::ArrowRight:
          Ty = static_cast<TFun*>(Ty)->getRight();
          break;
      default:
        ZEN_UNREACHABLE
    }
  }
  return Ty;
}

bool Type::operator==(const Type& Other) const {
  if (Other.getKind() != TK) {
    return false;
  }
  switch (TK) {
    case TypeKind::App:
      {
        auto A1 = static_cast<const TApp&>(*this);
        auto A2 = static_cast<const TApp&>(Other);
        return *A1.getLeft() == *A2.getLeft() && *A1.getRight() == *A2.getRight();
      }
    case TypeKind::Var:
      return this == &Other;
    case TypeKind::Fun:
      {
        auto F1 = static_cast<const TFun&>(*this);
        auto F2 = static_cast<const TFun&>(Other);
        return *F1.getLeft() == *F2.getLeft() && *F1.getRight() == *F2.getRight();
      }
    case TypeKind::Con:
      {
        auto C1 = static_cast<const TCon&>(*this);
        auto C2 = static_cast<const TCon&>(Other);
        return C1.getName() == C2.getName();
      }
  }
}

std::string Type::toString() const {
  switch (TK) {
    case TypeKind::App:
      {
        auto A = static_cast<const TApp*>(this);
        return A->getLeft()->toString() + " " + A->getRight()->toString();
      }
    case TypeKind::Con:
      {
        auto C = static_cast<const TCon*>(this);
        return std::string(C->getName());
      }
    case TypeKind::Fun:
      {
        auto F = static_cast<const TFun*>(this);
        return F->getLeft()->toString() + " -> " + F->getRight()->toString();
      }
    case TypeKind::Var:
      return "α";
  }
}

TVar* Type::asVar() {
  return static_cast<TVar*>(this);
}

void TypeVisitor::visit(Type* Ty) {
  switch (Ty->getKind()) {
    case TypeKind::App:
      visitApp(static_cast<TApp*>(Ty));
      break;
    case TypeKind::Con:
      visitCon(static_cast<TCon*>(Ty));
      break;
    case TypeKind::Fun:
      visitFun(static_cast<TFun*>(Ty));
      break;
    case TypeKind::Var:
      visitVar(static_cast<TVar*>(Ty));
      break;
  }
}

}
