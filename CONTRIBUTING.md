# Contributing to Bolt

So you want to contribute something to the Bolt language or its compiler? That's
great! We're so glad to have you!

## Preliminary Knowledge

### Type Checking

Here are some resouces to get started with type checking, ordered from high-level overview to deep dive:

- [Hindley-Milner Type System](https://en.wikipedia.org/wiki/Hindley%E2%80%93Milner_type_system)
- [Write You A Haskell](https://smunix.github.io/dev.stephendiehl.com/fun/index.html), most notably [this implemenation](https://github.com/sdiehl/write-you-a-haskell/tree/master/chapter7/poly_constraints)
- [Type Inference blog posts by Thunderseethe](https://thunderseethe.dev/posts/type-inference/)
- [Typing Haskell in Haskell](https://gist.github.com/chrisdone/0075a16b32bfd4f62b7b)
- [The Essence of ML Type Inference](http://gallium.inria.fr/~fpottier/publis/emlti-final.pdf)

## Building

### 1. Check out the sources

```sh
git clone https://github.com/boltlang/bolt
git clone https://github.com/samvv/zen bolt/deps/zen
```

You need the most recent version of the Zen libries to be able to compile. If
you still get build errors, the Zen libraries in the repositories might be out
of date. Do not hesitate to file an issue!

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
