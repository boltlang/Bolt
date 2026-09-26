# A Tour of The Bolt Programming Language

## Printing

To print a string, simply call the `print` function like in Python.

```
print("Hello, world!");
```

Output:

```
Hello, world!
```

You can also print numbers and booleans:

```rust
print(33);
print(True);
```

Output:

```
33
True
```

Any object that implements the `Display` trait is printable.

## Debug Printing

Bolt provides a convenient `debug` function to output the state of objects to
the console or terminal. It accepts any object and will print its
representation.

```rust
debug("Hello, world!");
debug(33);
debug(True);
```

```
"Hello, world!"
33
True
```

In Rust you would need to `#[derive(Debug)]` to print a struct or an enum. In
Bolt this is not necessary.

```rust
struct User {
    email: String,
    username: String,
}

let sam = User { "samvv@example.com", "samvv" };

debug(sam);
```

Outputs:

```
User {
    email: "samvv@example.com",
    username: "samvv",
}
```

## Format Strings

Just like in Python, format strings allow you to mix fragments of text with
values that need to be stringified.

```rust
let name = "Sam";
let age = 32;

print(f"Hello, {name}! You have {50 - age} years to go before becoming 50!");
```

Outputs:

```
Hello, Sam! You have 28 years to go before becoming 50!
```

## Extensible Records

Records are _extensible_, meaning that you can do things like this:

```rust
// greet accepts any record that has a 'name' field
fn greet(data: { name: String, .. }) {
    print(f"Hello, {data.name}!");
}

greet({ name = "Bob", email = "bob@example.com" }); // ok
greet({ company = "Accelera", name = "Sam", age = 32 }); // ok
greet({ fullname = "Herman Smith" }); // compile error (no field 'name')
```

## Keyword Arguments

Just like in Python, Bolt supports keyword arguments:

```rust
fn register(name: String, email: String) {
    todo!()
}

// Both calls are equivalent
register("Davis", "davis@example.com");
register(name="Davis", email="davis@example.com");
```

## Function Parameter Defaults

```rust
fn shoot(direction: Vec3, strength: Float32 = 1.0) {
    todo!()
}

shoot(Vec3::new(1.0, -1.0, 0.0)); // strength is 1.0
shoot(Vec3::new(1.0, -1.0, 0.0), 2.0); // strength is 2.0
shoot(Vec3::new(1.0, -1.0, 0.0), strength=0.5); // strength is 0.5
```

## Garbage Collection

All objects in the Bolt programming language are garbage-collected. That means
that you don't need to call `malloc()` and `free()`(like in C) or reason about
borrowing rules (like in Rust).

One of the additional advantages of a garbage collector is that cycles are
allowed. Some might consider the use of cycles bad practice, but it is
occasionally useful for prototyping:

```rust
struct Node<T> {
    value: Int32,
    next: Option<Node<T>>,
    prev: Option<Node<T>>,
}

let first = Node { 1, None, None }
let second = Node { 2, None, first }
second.prev = first; // create the cycle
```

## No `mut`

Because references are not explicit in Bolt, Rust's `mut` keyword becomes irrelevant.

```rust
let foo = 1;
foo = 2;
print(foo);
```

Output:

```
2
```

## Typeclasses

Bolt will also support traits/typeclasses, like in Rust and Haskell:

```rust
class Shout<T> {
  fn shout(self: T) -> String;
}

impl Shout<Dog> {
    fn shout(self) {
        print("Bark, bark!");
    }
}

// Imagine somewhere in another library Cat is defined ...

impl Shout<Cat> {
    fn shout(self) {
        print("Miau! Miau!");
    }
}
```

## Reactive UI Framework

Here's an example of a [React](https://react.dev/)-like framework in Bolt:

_Note that this example is very experimental._

```rust
import "html" ( Html )

fn app() -> Html {

    let reactive user = None;

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

