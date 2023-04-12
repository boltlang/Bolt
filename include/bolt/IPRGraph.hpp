
#pragma once

#include <unordered_map>

namespace bolt {

  class Node;
  class ReferenceExpression;

  /**
   * An inter-procedural reference graph.
   *
   * This graph keeps track of the references made to other procedures in the
   * same program.
   */
  class IPRGraph {

    std::unordered_map<Node*, Node*> Edges;

  public:

    void populate(Node* , Node* Decl = nullptr);

    bool isRecursive(ReferenceExpression* Node);

  };

}
