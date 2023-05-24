
cmake_minimum_required(VERSION 3.10)

project(Bolt CXX)

set(CMAKE_CXX_STANDARD 17)

add_subdirectory(deps/zen EXCLUDE_FROM_ALL)

set(ICU_DIR "${CMAKE_CURRENT_SOURCE_DIR}/build/icu/install")
set(ICU_CFLAGS "-DUNISTR_FROM_CHAR_EXPLICIT=explicit -DUNISTR_FROM_STRING_EXPLICIT=explicit -DU_NO_DEFAULT_INCLUDE_UTF_HEADERS=1 -DU_HIDE_OBSOLETE_UTF_OLD_H=1")
set(ICU_INCLUDE_DIRS "${ICU_DIR}/include")
set(ICU_LIBRARY_DIRS "${ICU_DIR}/lib")
set(ICU_LIBRARIES icuuc)

add_library(
  BoltCore
  #src/Text.cc
  src/CST.cc
  src/Diagnostics.cc
  src/Scanner.cc
  src/Parser.cc
  src/Types.cc
  src/Checker.cc
  src/IPRGraph.cc
)
target_link_directories(
  BoltCore
  PUBLIC
  ${ICU_LIBRARY_DIRS}
)
target_compile_options(
  BoltCore
  PUBLIC
  -fstandalone-debug
  -Werror
  ${ICU_CFLAGS}
)
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

add_executable(
  bolt
  src/main.cc
)
target_link_libraries(
  bolt
  PUBLIC
  BoltCore
)

if (BOLT_ENABLE_TESTS)
  add_subdirectory(deps/googletest EXCLUDE_FROM_ALL)
  add_executable(
    alltests
    src/TestText.cc
    src/TestChecker.cc
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

