
#include "zen/range.hpp"

#include "bolt/CST.hpp"
#include "bolt/Evaluator.hpp"

namespace bolt {

  Value Evaluator::evaluateExpression(Expression* X, Env& Env) {
    switch (X->getKind()) {
      case NodeKind::ReferenceExpression:
      {
        auto RE = static_cast<ReferenceExpression*>(X);
        return Env.lookup(getCanonicalText(RE->Name));
        // auto Decl = RE->getScope()->lookup(RE->getSymbolPath());
        // ZEN_ASSERT(Decl && Decl->getKind() == NodeKind::FunctionDeclaration);
        // return static_cast<FunctionDeclaration*>(Decl);
      }
      case NodeKind::LiteralExpression:
      {
        auto CE = static_cast<LiteralExpression*>(X);
        switch (CE->Token->getKind()) {
          case NodeKind::IntegerLiteral:
            return static_cast<IntegerLiteral*>(CE->Token)->V;
          case NodeKind::StringLiteral:
            return static_cast<StringLiteral*>(CE->Token)->Text;
          default:
            ZEN_UNREACHABLE
        }
      }
      case NodeKind::CallExpression:
      {
        auto CE = static_cast<CallExpression*>(X);
        auto Op = evaluateExpression(CE->Function, Env);
        std::vector<Value> Args;
        for (auto Arg: CE->Args) {
          Args.push_back(evaluateExpression(Arg, Env));
        }
        return apply(Op, Args);
      }
      default:
        ZEN_UNREACHABLE
    }
  }

  void Evaluator::assignPattern(Pattern* P, Value& V, Env& E) {
    switch (P->getKind()) {
      case NodeKind::BindPattern:
      {
        auto BP = static_cast<BindPattern*>(P);
        E.add(getCanonicalText(BP->Name), V);
        break;
      }
      default:
        ZEN_UNREACHABLE
    }
  }

  Value Evaluator::apply(Value Op, std::vector<Value> Args) {
    switch (Op.getKind()) {
      case ValueKind::SourceFunction:
      {
        auto Fn = Op.getDeclaration();
        Env NewEnv;
        for (auto [Param, Arg]: zen::zip(Fn->Params, Args)) {
          assignPattern(Param->Pattern, Arg, NewEnv);
        }
        switch (Fn->Body->getKind()) {
          case NodeKind::LetExprBody:
            return evaluateExpression(static_cast<LetExprBody*>(Fn->Body)->Expression, NewEnv);
          default:
            ZEN_UNREACHABLE
        }
      }
      case ValueKind::NativeFunction:
      {
        auto Fn = Op.getBinding();
        return Fn(Args);
      }
      default:
        ZEN_UNREACHABLE
    }
  }

  void Evaluator::evaluate(Node* N, Env& E) {
    switch (N->getKind()) {
      case NodeKind::SourceFile:
      {
        auto SF = static_cast<SourceFile*>(N);
        for (auto Element: SF->Elements) {
          evaluate(Element, E);
        }
        break;
      }
      case NodeKind::ExpressionStatement:
      {
        auto ES = static_cast<ExpressionStatement*>(N);
        evaluateExpression(ES->Expression, E);
        break;
      }
      case NodeKind::LetDeclaration:
      {
        auto Decl = static_cast<LetDeclaration*>(N);
        if (Decl->isFunction()) {
          E.add(Decl->getNameAsString(), Decl);
        } else {
          Value V;
          if (Decl->Body) {
            switch (Decl->Body->getKind()) {
              case NodeKind::LetExprBody:
              {
                auto Body = static_cast<LetExprBody*>(Decl->Body);
                V = evaluateExpression(Body->Expression, E);
              }
              default:
                ZEN_UNREACHABLE
            }
          }
        }
        break;
      }
      default:
        ZEN_UNREACHABLE
    }
  }

}
