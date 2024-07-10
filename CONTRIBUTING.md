# Contributing to Bolt

So you want to contribute something to the Bolt language or its compiler? That's
great! We're so glad to have you!

## Preliminary Knowledge

It is recommended you know a thing or two about compilers.

## Building

### 1. Check out the sources

```sh
git clone https://github.com/boltlang/bolt
git clone https://github.com/samvv/zen bolt/deps/zen
```

You need the most recent version of the Zen libries to be able to compile. If
you still get build errors, the Zen libraries in the repositories might be out
of date. Do not hesitate to file an issue!

2. Configure the build using CMake

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

3. Build the compiler

Issue the following command while being in the root folder of the project:

```sh
ninja -C build bolt
```

4. Run the compiler

The compiler is not available in `build/bolt`. You can run it like so:

```sh
./build/bolt test.bolt
```

That's it! You've succesfully compiled some sources with Bolt!