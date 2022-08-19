
cmake_minimum_required(VERSION 3.10)

project(Bolt CXX)

set(CMAKE_CXX_STANDARD 17)

add_subdirectory(deps/zen EXCLUDE_FROM_ALL)

add_executable(
  bolt
  src/CST.cc
  src/Diagnostics.cc
  src/Scanner.cc
  src/Parser.cc
  src/main.cc
)
target_compile_options(
  bolt
  PUBLIC
  -fstandalone-debug
)
target_include_directories(
  bolt
  PUBLIC
  include
)
target_link_libraries(
  bolt
  PUBLIC
  zen
)

add_custom_command(
  OUTPUT "${CMAKE_CURRENT_SOURCE_DIR}/include/bolt/CST.hpp" "${CMAKE_CURRENT_SOURCE_DIR}/src/CST.cc"
  COMMAND scripts/gennodes.py --name=CST ./bolt-cst-spec.txt -Iinclude/ --include-root=bolt --source-root=src/ --namespace=bolt
  DEPENDS scripts/gennodes.py
  MAIN_DEPENDENCY "${CMAKE_CURRENT_SOURCE_DIR}/bolt-cst-spec.txt"
  WORKING_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}"
)

