# LLVM-based Bolt Compiler

This directory contains a partial implementation of the Bolt programming
language using C++ and LLVM. It currently is one of the main contenders of
becoming the offical Bolt compiler.

## Developing

You need a recent version of the Zen libries to be able to compile. If youu
still get build errors, the Zen libraries might be out of date. Do not hesitate
to file an issue!

This is how a potential invocation to CMake could look like:

```sh
cmake \
    -DCMAKE_CXX_COMPILER=clang++ \
    -DCMAKE_BUILD_TYPE=Debug \
    -DCMAKE_EXPORT_COMPILE_COMMANDS=ON \
    -G Ninja \
    -B build \
    -DBOLT_ENABLE_TESTS=1 \
    -DZEN_ENABLE_TESTS=0 \
    -DLLVM_TARGETS_TO_BUILD=host \
    -DLLVM_OPTIMIZED_TABLEGEN=ON
```
