
// FIXME writeExcerpt does not work well with the last line in a file

#include <sstream>
#include <cmath>

#include "zen/config.hpp"

#include "bolt/CST.hpp"
#include "bolt/Type.hpp"
#include "bolt/DiagnosticEngine.hpp"
#include "bolt/Diagnostics.hpp"
#include "bolt/ConsolePrinter.hpp"

#define ANSI_RESET "\u001b[0m"
#define ANSI_BOLD "\u001b[1m"
#define ANSI_ITALIC "\u001b[3m"
#define ANSI_UNDERLINE "\u001b[4m"
#define ANSI_REVERSED "\u001b[7m"

#define ANSI_FG_BLACK "\u001b[30m"
#define ANSI_FG_RED "\u001b[31m"
#define ANSI_FG_GREEN "\u001b[32m"
#define ANSI_FG_YELLOW "\u001b[33m"
#define ANSI_FG_BLUE "\u001b[34m"
#define ANSI_FG_CYAN "\u001b[35m"
#define ANSI_FG_MAGENTA "\u001b[36m"
#define ANSI_FG_WHITE "\u001b[37m"

#define ANSI_BG_BLACK "\u001b[40m"
#define ANSI_BG_RED "\u001b[41m"
#define ANSI_BG_GREEN "\u001b[42m"
#define ANSI_BG_YELLOW "\u001b[43m"
#define ANSI_BG_BLUE "\u001b[44m"
#define ANSI_BG_CYAN "\u001b[45m"
#define ANSI_BG_MAGENTA "\u001b[46m"
#define ANSI_BG_WHITE "\u001b[47m"

namespace bolt {

Diagnostic::Diagnostic(DiagnosticKind Kind):
  Kind(Kind) {}

bool sourceLocLessThan(const Diagnostic* L, const Diagnostic* R) {
  auto N1 = L->getNode();
  auto N2 = R->getNode();
  if (N1 == nullptr && N2 == nullptr) {
    return false;
  }
  if (N1 == nullptr) {
    return true;
  }
  if (N2 == nullptr) {
    return false;
  }
  return N1->getStartLine() < N2->getStartLine() || N1->getStartColumn() < N2->getStartColumn();
};

void DiagnosticStore::sort() {
  std::sort(Diagnostics.begin(), Diagnostics.end(), sourceLocLessThan);
}

DiagnosticStore::~DiagnosticStore() {
  for (auto D: Diagnostics) {
    delete D;
  }
}

ConsoleDiagnostics::ConsoleDiagnostics(ConsolePrinter& P):
  ThePrinter(P) {}

void ConsoleDiagnostics::addDiagnostic(Diagnostic* D) {

  ThePrinter.writeDiagnostic(*D);

  // Since this DiagnosticEngine is expected to own the diagnostic, we simply
  // destroy the processed diagnostic so that there are no memory leaks.
  delete D;
}

}
