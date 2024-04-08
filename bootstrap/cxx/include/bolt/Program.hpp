
#pragma once

#include <cstdlib>
#include <filesystem>
#include <unordered_map>

#include "bolt/Common.hpp"
#include "bolt/Checker.hpp"
#include "bolt/DiagnosticEngine.hpp"

class SourceFile;

namespace bolt {

class Program {

  DiagnosticEngine& DE;
  LanguageConfig Config;

  std::unordered_map<std::filesystem::path, SourceFile*> SourceFiles;
  std::unordered_map<SourceFile*, Checker> TCs;

public:

  Program(DiagnosticEngine& DE, LanguageConfig Config = {}):
    DE(DE), Config(Config) {}

  auto getSourceFiles() {
    return zen::make_iterator_range(SourceFiles.begin(), SourceFiles.end()).map_second();
  }

  void addSourceFile(std::filesystem::path Path, SourceFile* SF) {
    SourceFiles.emplace(Path, SF);
  }

  DiagnosticEngine& getDiagnostics() {
    return DE;
  }

  void setDiagnostics(DiagnosticEngine& New) {
    DE = New;
  }

  Checker& getTypeChecker(SourceFile* SF) {
    auto Match = TCs.find(SF);
    if (Match != TCs.end()) {
      return Match->second;
    }
    return TCs.emplace(SF, Checker { Config, DE }).first->second;
  }

  void check() {
    for (auto SF: getSourceFiles()) {
      getTypeChecker(SF).check(SF);
    }
  }

  // ~Program() {
  //   for (auto [SF, TC]: TCs) {
  //     delete TC;
  //   }
  // }

};

}
