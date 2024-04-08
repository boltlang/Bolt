
#include "bolt/CST.hpp"

namespace bolt {

Scope::Scope(Node* Source):
  Source(Source) {
    scan(Source);
  }

void Scope::addSymbol(ByteString Name, Node* Decl, SymbolKind Kind) {
  Mapping.emplace(Name, std::make_tuple(Decl, Kind));
}

void Scope::scan(Node* X) {
  switch (X->getKind()) {
    case NodeKind::SourceFile:
    {
      auto File = static_cast<SourceFile*>(X);
      for (auto Element: File->Elements) {
        scanChild(Element);
      }
      break;
    }
    case NodeKind::MatchCase:
    {
      auto Case = static_cast<MatchCase*>(X);
      visitPattern(Case->Pattern, Case);
      break;
    }
    case NodeKind::PrefixFunctionDeclaration:
    case NodeKind::InfixFunctionDeclaration:
    case NodeKind::SuffixFunctionDeclaration:
    case NodeKind::NamedFunctionDeclaration:
    {
      auto Decl = static_cast<FunctionDeclaration*>(X);
      for (auto Param: Decl->getParams()) {
        visitPattern(Param->Pattern, Param);
      }
      auto Body = Decl->getBody();
      if (Body) {
        scanChild(Body);
      }
      break;
    }
    default:
      ZEN_UNREACHABLE
  }
}

void Scope::scanChild(Node* X) {
  switch (X->getKind()) {
    case NodeKind::LetExprBody:
    case NodeKind::ExpressionStatement:
    case NodeKind::IfStatement:
    case NodeKind::ReturnStatement:
      break;
    case NodeKind::LetBlockBody:
    {
      auto Block = static_cast<LetBlockBody*>(X);
      for (auto Element: Block->Elements) {
        scanChild(Element);
      }
      break;
    }
    case NodeKind::InstanceDeclaration:
      // We ignore let-declarations inside instance-declarations for now
      break;
    case NodeKind::ClassDeclaration:
    {
      auto Decl = static_cast<ClassDeclaration*>(X);
      addSymbol(Decl->Name->getCanonicalText(), Decl, SymbolKind::Class);
      for (auto Element: Decl->Elements) {
        scanChild(Element);
      }
      break;
    }
    case NodeKind::PrefixFunctionDeclaration:
    case NodeKind::InfixFunctionDeclaration:
    case NodeKind::SuffixFunctionDeclaration:
    case NodeKind::NamedFunctionDeclaration:
    {
      auto Decl = static_cast<FunctionDeclaration*>(X);
      addSymbol(Decl->getNameAsString(), Decl, SymbolKind::Var);
      break;
    }
    case NodeKind::VariableDeclaration:
    {
      auto Decl = static_cast<VariableDeclaration*>(X);
      visitPattern(Decl->Pattern, Decl);
      break;
    }
    case NodeKind::RecordDeclaration:
    {
      auto Decl = static_cast<RecordDeclaration*>(X);
      addSymbol(Decl->Name->getCanonicalText(), Decl, SymbolKind::Type);
      break;
    }
    case NodeKind::VariantDeclaration:
    {
      auto Decl = static_cast<VariantDeclaration*>(X);
      addSymbol(Decl->Name->getCanonicalText(), Decl, SymbolKind::Type);
      for (auto Member: Decl->Members) {
        switch (Member->getKind()) {
          case NodeKind::TupleVariantDeclarationMember:
          {
            auto T = static_cast<TupleVariantDeclarationMember*>(Member);
            addSymbol(T->Name->getCanonicalText(), Decl, SymbolKind::Constructor);
            break;
          }
          case NodeKind::RecordVariantDeclarationMember:
          {
            auto R = static_cast<RecordVariantDeclarationMember*>(Member);
            addSymbol(R->Name->getCanonicalText(), Decl, SymbolKind::Constructor);
            break;
          }
          default:
            ZEN_UNREACHABLE
        }
      }
      break;
    }
    default:
      ZEN_UNREACHABLE
  }
}

void Scope::visitPattern(Pattern* X, Node* Decl) {
  switch (X->getKind()) {
    case NodeKind::BindPattern:
    {
      auto Y = static_cast<BindPattern*>(X);
      addSymbol(Y->Name->getCanonicalText(), Decl, SymbolKind::Var);
      break;
    }
    case NodeKind::RecordPattern:
    {
      auto Y = static_cast<RecordPattern*>(X);
      for (auto [Field, Comma]: Y->Fields) {
        if (Field->Pattern) {
          visitPattern(Field->Pattern, Decl);
        } else if (Field->Name) {
          addSymbol(Field->Name->Text, Decl, SymbolKind::Var);
        }
      }
      break;
    }
    case NodeKind::NamedRecordPattern:
    {
      auto Y = static_cast<NamedRecordPattern*>(X);
      for (auto [Field, Comma]: Y->Fields) {
        if (Field->Pattern) {
          visitPattern(Field->Pattern, Decl);
        } else if (Field->Name) {
          addSymbol(Field->Name->Text, Decl, SymbolKind::Var);
        }
      }
      break;
    }
    case NodeKind::NamedTuplePattern:
    {
      auto Y = static_cast<NamedTuplePattern*>(X);
      for (auto P: Y->Patterns) {
        visitPattern(P, Decl);
      }
      break;
    }
    case NodeKind::NestedPattern:
    {
      auto Y = static_cast<NestedPattern*>(X);
      visitPattern(Y->P, Decl);
      break;
    }
    case NodeKind::TuplePattern:
    {
      auto Y = static_cast<TuplePattern*>(X);
      for (auto [Element, Comma]: Y->Elements) {
        visitPattern(Element, Decl);
      }
      break;
    }
    case NodeKind::ListPattern:
    {
      auto Y = static_cast<ListPattern*>(X);
      for (auto [Element, Separator]: Y->Elements) {
        visitPattern(Element, Decl);
      }
      break;
    }
    case NodeKind::LiteralPattern:
      break;
    default:
      ZEN_UNREACHABLE
  }
}

Node* Scope::lookupDirect(SymbolPath Path, SymbolKind Kind) {
  ZEN_ASSERT(Path.Modules.empty());
  auto Match = Mapping.find(Path.Name);
  if (Match != Mapping.end() && std::get<1>(Match->second) == Kind) {
    return std::get<0>(Match->second);
  }
  return nullptr;
}

Node* Scope::lookup(SymbolPath Path, SymbolKind Kind) {
  ZEN_ASSERT(Path.Modules.empty());
  auto Curr = this;
  do {
    auto Found = Curr->lookupDirect(Path, Kind);
    if (Found) {
      return Found;
    }
    Curr = Curr->getParentScope();
  } while (Curr != nullptr);
  return nullptr;
}

Scope* Scope::getParentScope() {
  if (Source->Parent == nullptr) {
    return nullptr;
  }
  return Source->Parent->getScope();
}

}
