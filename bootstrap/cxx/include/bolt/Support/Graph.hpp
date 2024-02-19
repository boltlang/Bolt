
#pragma once

#include <unordered_set>
#include <unordered_map>
#include <vector>
#include <stack>
#include <optional>

#include "zen/range.hpp"

namespace bolt {

template<typename V>
class Graph {

  std::unordered_set<V> Vertices;
  std::unordered_multimap<V, V> Edges;

public:

  void addVertex(V Vert) {
    Vertices.emplace(Vert);
  }

  void addEdge(V A, V B) {
    Vertices.emplace(A);
    Vertices.emplace(B);
    Edges.emplace(A, B);
  }

  std::size_t countVertices() const {
    return Vertices.size();
  }

  bool hasVertex(const V& Vert) const {
    return Vertices.count(Vert);
  }

  bool hasEdge(const V& From) const {
    return Edges.count(From);
  }

  bool hasEdge(const V& From, const V& To) const {
    for (auto X: Edges.equal_range(From)) {
      if (X == To) {
        return true;
      }
    }
  }

  auto getTargetVertices(const V& From) const {
    return zen::make_iterator_range(Edges.equal_range(From)).map_second();
  }

  auto getVertices() const {
    return zen::make_iterator_range(Vertices);
  }

private:

  struct TarjanVertexData {
    std::optional<std::size_t> Index;
    std::size_t LowLink;
    bool OnStack = false;
  };

  class TarjanSolver {
  public:

    std::vector<std::vector<V>> SCCs;

  private:

    const Graph& G;
    std::unordered_map<V, TarjanVertexData> Map;
    std::size_t Index = 0;
    std::stack<V> Stack;

    TarjanVertexData& getData(V From) {
      return Map.emplace(From, TarjanVertexData {}).first->second;
    }

    void visitCycle(const V& From) {

      auto& DataFrom = getData(From);
      DataFrom.Index = Index;
      DataFrom.LowLink = Index;
      Index++;
      Stack.push(From);
      DataFrom.OnStack = true;

      for (const auto& To: G.getTargetVertices(From)) {
        auto& DataTo = getData(To);
        if (!DataTo.Index) {
          visitCycle(To);
          DataFrom.LowLink = std::min(DataFrom.LowLink, DataTo.LowLink);
        } else if (DataTo.OnStack) {
          DataFrom.LowLink = std::min(DataFrom.LowLink, *DataTo.Index);
        }
      }

      if (DataFrom.LowLink == DataFrom.Index) {
        std::vector<V> SCC;
        for (;;) {
          auto& X = Stack.top();
          Stack.pop();
          auto& DataX = getData(X);
          DataX.OnStack = false;
          SCC.push_back(X);
          if (X == From) {
            break;
          }
        }
        SCCs.push_back(SCC);
      }

    }

   public:

    TarjanSolver(const Graph& G):
      G(G) {}

    void solve() {
      for (auto From: G.Vertices) {
        if (!Map.count(From)) {
          visitCycle(From);
        }
      }
    }

  };

public:

  std::vector<std::vector<V>> strongconnect() const {
    TarjanSolver S { *this };
    S.solve();
    return S.SCCs;
  }

};

}

