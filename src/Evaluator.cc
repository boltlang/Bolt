
#include "bolt/CST.hpp"
#include "bolt/Evaluator.hpp"

namespace bolt {

Value Evaluator::evaluateExpression(Expression* X, Env& Env) {
  switch (X->getKind()) {
    case NodeKind::ReferenceExpression:
    {
      auto RE = static_cast<ReferenceExpression*>(X);
      return Env.lookup(RE->Name.getCanonicalText());
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
      E.add(BP->Name->getCanonicalText(), V);
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
      auto Params = Fn->getParams();
      auto ParamIter = Params.begin();
      auto ParamsEnd = Params.end();
      auto ArgIter = Args.begin();
      auto ArgsEnd = Args.end();
      for (;;) {
        if (ParamIter == ParamsEnd && ArgIter == ArgsEnd) {
          break;
        }
        if (ParamIter == ParamsEnd) {
          // TODO Make this a soft failure
          ZEN_PANIC("Too much arguments supplied to function call.");
        }
        if (ArgIter == ArgsEnd) {
          // TODO Make this a soft failure
          ZEN_PANIC("Too much few arguments supplied to function call.");
        }
        assignPattern((*ParamIter)->Pattern, *ArgIter, NewEnv);
      }
      switch (Fn->getBody()->getKind()) {
        case NodeKind::LetExprBody:
          return evaluateExpression(static_cast<LetExprBody*>(Fn->getBody())->Expression, NewEnv);
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
  if (isa<Expression>(N)) {
    evaluateExpression(cast<Expression>(N), E);
    return;
  }
  switch (N->getKind()) {
    case NodeKind::SourceFile:
    {
      auto SF = static_cast<SourceFile*>(N);
      for (auto Element: SF->Elements) {
        evaluate(Element, E);
      }
      break;
    }
    case NodeKind::PrefixFunctionDeclaration:
    case NodeKind::InfixFunctionDeclaration:
    case NodeKind::SuffixFunctionDeclaration:
    case NodeKind::NamedFunctionDeclaration:
    {
      auto Decl = static_cast<FunctionDeclaration*>(N);
      E.add(Decl->getNameAsString(), Decl);
      break;
    }
    case NodeKind::VariableDeclaration:
    {
      auto Decl = static_cast<VariableDeclaration*>(N);
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
      break;
    }
    default:
      ZEN_UNREACHABLE
  }
}

}
