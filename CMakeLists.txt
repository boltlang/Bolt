
cmake_minimum_required(VERSION 3.20)

project(Bolt C CXX)

set(CMAKE_CXX_STANDARD 20)

add_subdirectory(deps/zen EXCLUDE_FROM_ALL)
add_subdirectory(deps/llvm-project/llvm EXCLUDE_FROM_ALL)

set(ICU_DIR "${CMAKE_CURRENT_SOURCE_DIR}/build/icu/install")
set(ICU_CFLAGS "-DUNISTR_FROM_CHAR_EXPLICIT=explicit -DUNISTR_FROM_STRING_EXPLICIT=explicit -DU_NO_DEFAULT_INCLUDE_UTF_HEADERS=1 -DU_HIDE_OBSOLETE_UTF_OLD_H=1")
set(ICU_INCLUDE_DIRS "${ICU_DIR}/include")
set(ICU_LIBRARY_DIRS "${ICU_DIR}/lib")
set(ICU_LIBRARIES icuuc)

if (CMAKE_BUILD_TYPE STREQUAL "RelWithDebInfo" OR CMAKE_BUILD_TYPE STREQUAL "Debug")
  set(BOLT_DEBUG ON)
endif()

#find_package(LLVM 19.0 REQUIRED)

add_library(
  BoltCore
  #src/Text.cc
  src/CST.cc
  src/Diagnostics.cc
  src/ConsolePrinter.cc
  src/Scanner.cc
  src/Parser.cc
  src/Type.cc
  src/Checker.cc
  src/Evaluator.cc
  src/Scope.cc
  src/Program.cc
)
target_link_directories(
  BoltCore
  PUBLIC
  ${ICU_LIBRARY_DIRS}
)
target_compile_options(
  BoltCore
  PUBLIC
  -Werror
  -fno-exceptions
  ${ICU_CFLAGS}
)

if (CMAKE_CXX_COMPILER_ID STREQUAL "Clang" AND BOLT_DEBUG)
  target_compile_options(
    BoltCore
    PUBLIC
    -fstandalone-debug
  )
endif()
target_include_directories(
  BoltCore
  PUBLIC
  include
  ${ICU_LIBRARY_DIRS}
)
target_link_libraries(
  BoltCore
  PUBLIC
  zen
  icuuc
)

add_library(
  BoltLLVM
  src/LLVMCodeGen.cc
)
target_link_libraries(
  BoltLLVM
  PUBLIC
  BoltCore
  LLVMCore
  LLVMTarget
)
target_include_directories(
  BoltLLVM
  PUBLIC
  deps/llvm-project/llvm/include # FIXME this is a hack
  ${CMAKE_BINARY_DIR}/deps/llvm-project/llvm/include # FIXME this is a hack
)

add_executable(
  bolt
  src/main.cc
)
target_link_libraries(
  bolt
  PUBLIC
  BoltCore
  BoltLLVM
)

if (BOLT_ENABLE_TESTS)
  add_subdirectory(deps/googletest EXCLUDE_FROM_ALL)
  add_executable(
    alltests
    test/TestText.cc
    test/TestChecker.cc
  )
  target_link_libraries(
    alltests
    PUBLIC
    BoltCore
    gtest
    gtest_main
  )
endif()

# add_custom_command(
#   OUTPUT "${CMAKE_CURRENT_SOURCE_DIR}/include/bolt/CST.hpp" "${CMAKE_CURRENT_SOURCE_DIR}/src/CST.cc"
#   COMMAND scripts/gennodes.py --name=CST ./bolt-cst-spec.txt -Iinclude/ --include-root=bolt --source-root=src/ --namespace=bolt
#   DEPENDS scripts/gennodes.py
#   MAIN_DEPENDENCY "${CMAKE_CURRENT_SOURCE_DIR}/bolt-cst-spec.txt"
#   WORKING_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}"
# )

