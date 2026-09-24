Bolt
====

> [!WARNING]
>
> This compiler is experimental and might have breaking changes with each
> update. It is not even guaranteed to compile something at this moment!

💬 Got some questions or feedback? Just open an issue and we'll be glad to respond!

```rust
fn fac(n: Int) -> Int {
    match n {
      0 | 1 => 1
      k => k * fac(k-1)
    }
}
```

Bolt is a new strictly-evaluated programming language in the making that aims
to make writing complex applications dead-simple. It ships with some
nice goodies, including:

 - **Static type checking** will catch hundreds of bugs before a single line of
   code is actually run.
 - **Garbage collected**, freeing you from worrying about memory management.
 - **Tuples, match-expressions and hygienic macros** make your code more
   readable and concise.
 - **Cross-platform standard library**, allowing you to write your code for the
   web and the desktop at the same time.

## Examples

_Note that these examples are stil in the design phase and not able to compile._

All objects are garbage-collected and passed by reference, meaning you can do
things like the following without a problem:

```rust
struct Node {
    value: Int,
    children: Vec<Node>,
    parent: Option<Node>,
}

impl Node {

    pub fn new(value: Int) -> Node {
        Node {
            value,
            children: Vec::new(),
            parent: None,
        }
    }

}

// Build the tree

let mut root = Node::new(1);

let mut left = Node::new(2);
root.children.push(left);
left.parent = root;

let mut right = Node::new(3);
root.children.push(right);
right.parent = root;

// Now set the leftmost node's value to 42
root.children.get(0).unwrap().value = 42;

// Because everthing is managed by references, the variable `left` will be upated!
assert!(left.value == 42);
```

Here are some records we define in Bolt:

```rust
struct Dog {
    name: String,
    age: i32,
}

let spike = Dog {
    name = "Spike",
    age = 5
}

// Shorthand
let sadie = Dog("Sadie", 12)

print(f"Hey look! {spike.name} and {sadie.name} are playing together!");
```

Records are _extensible_, meaning that you can do things like this:

```rust
fn greet({ name, .. }) {
    print(f"Hello, {name}!");
}

greet(sadie);
greet(spike);
greet({ name = "Sam", company = "Accelera" });
```

Bolt will also support traits/type classes, like in Rust and Haskell:

```rust
trait Shout a {
  fn shout(self: a) -> String;
}

impl Shout Dog {
    fn shout(self) {
        print("Bark, bark!");
    }
}

// Imagine somewhere in another library Cat is defined ...

impl Shout Cat {
    fn shout(self) {
        print("Miau! Miau!");
    }
}
```

Here's an example of a [React](https://react.dev/)-like framework in Bolt:

_Note that this example is very experimental._

```
import "html" ( Html )

fn app() -> Html {

    rx user = None;

    match user {
        None => {
            let { data, isLoading, isError, isOk } = fetch("/api/login");
            if isLoading {
                return <h1>Processing ...</h1>;
            }
            if isError {
                return <h1>Something went wrong.</h1>;
            }
            if isOk {
                user = Some({ username: data.username });
                return <h1>Logging in ...</h1>;
            }
            <h1>Please log in.</h1>
        }
        Some({ name, .. }) => <h1>Welcome Back, {username}</h1>
    }
}
```

## Core Principles

Bolt has a few fundamental design principles that we hope in time will make it
stand out from other programming languages.

 - **Do it right.** From Hindley-Milner to algebraic effects, decades
   of research has made it possible to write programming languages that can be
   way more secure and performant than would have been possible in 70s.
   Wherever possible, we try to make use of these awesome discoveries.
 - **Do it once.** Bolt aims to save development time. Type-based abstractions,
   a macro system, and a platform that works on both desktop and the web has to
   ensure that you don't waste time writing the same logic in a slightly
   different form.
 - **Do not constrain the user.** Given that the perfect programming language
   theory has yet to be discovered, real-world programs may require the
   programmer to e.g. mutate a variable. Don't make the developer jump through
   hoops unless the benefits clearly outweigh the costs.


## FAQ

### When will the compiler be ready to use?

It is really difficult to say when the compiler will be ready. This project is
made exclusively in people their spare time. Depending on the availability of the
programmers making this project possible, this might take a lot of months or
even years.

Currently, we are working hard to get the type-checker fully operational. After
that work will continue on the code generator and garbage collector. A small
part of the code generator is already done: the first bindings to LLVM are
operational and will be expanded in the future. Stay tuned!

### Why yet another programming language?

Granted, there are a lot of programming languages, but oddly enough I found
myself in the situation where I was still not completely satisfied. For
example, Rust is a fantastic programming language, but writing web applications
in it feels a little cumbersome due to its manual memory management. On the
other hand, functional programming languages like Haskell and Idris look like
they come straight out of the future, but sometimes have unpredictable run-time
performance and force you to do weird things when all you want to do is mutate
a variable. Bolt aims to solve these two issues, while not giving in on
performance or correctness.

### Why should I choose Bolt over JavaScript?

First of all, let me tell you that if you've never heard of a functional
programming language, learning Bolt will take some time. However, I assure you
that it is worth it.

 - Correctness, because Bolt has a type system that is far superior to
   JavaScript's. No more member accesses into `undefined` or other weird error
   messages.
 - Performance, because the language has been designed to leave room for a lot
   of optimisations. The garbage collector is one of the few overheads that the
   language requires.
 - Scalability, because just like Rust, Bolt takes a functional approach to
   software design using type traits, favouring composition over inheritance.

### What happened to the old compiler?

The old repository has been archived and has been replaced with this one. I
doubt there will be much interest in this older software artifact. If you want
to check it out nonetheless, you can still do it [by following this link][1].

[1]: https://github.com/boltlang/BoltJS

### What's the difference between the old Bolt programming language and this language?

I redesigned the language from the ground up to be more functional, terser, and
with very straightforward extensions for writing complex HTML. It is by no
means done, but I do hope to have struck a good balance between readability and
ease of use.

## License

Bolt was initially licensed under the GNU GPL, version 3. I decided to release this new
version under the more permissive MIT license. In the end, I hope that this
piece of software may inspire other projects and may improve the quality of new
and existing software.

