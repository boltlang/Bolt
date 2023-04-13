
## Record types can be unified without causing an error

```
struct Person.
  email: String
  age: Int

let bert
  = Person {
    email = "bar@boo.com",
    age = 32
  }
let bob
  = Person {
    email = "boo",
    age = 43
  }

bert == bob
```

## Return types are polymorphic

```
let id x = x

id 1
id "foo"
id True
```

## Nested definitions work

```
let foo x.
  let bar y z = y + z - x
  bar

foo True
```

## Everything that can be type-checked will be type-checked

```
let foo n.
  let f : String = 1
  return n
```

## Recursive definitions do not cause infinite loops in the type-checker

```
let fac n = fac_2 n

let fac_2 n = fac_3 n + fac n

let fac_3 n = fac_2 (n-1)

not (fac 1)
```

## Example with mutual recursion works

```
let is_even x.
  if x == 0.
    return True
  else.
    return is_odd (x-1)

let is_odd x.
  if x == 1.
    return False
  else.
    return is_even (x-1)

not (is_even True)
```

## Polymorphic records can be partially typed

```
struct Timestamped a b.
  first: a
  second: b
  timestamp: Int

type Foo = Timestamped Int

type Bar = Foo Int

let t : Bar = Timestamped { first = "bar", second = 1, timestamp = 12345 }
```

## Extensible records work

```
struct Timestamped a.
  data: a
  timestamp: Int

let t = Timestamped { data = "foo", timestamp = 12345 }

t.data == 1
t.data == "foo"

let u = Timestamped { data = True, timestamp = 12345 }

u.data == "foo"
u.data == False
```

## A recursive function is automatically instantiated

```
let fac n.
  if n == 0.
    return 1
  else.
    return n * fac (n-"foo")
```

## Enum-declarations are correctly typed

```
enum Maybe a.
  Just a
  Nothing

let right_1 : Maybe Int = Just 1
let right_2 : Maybe String = Just "foo"
let wrong : Maybe Int = Just "foo"
```

## Kind inference works

```
enum Maybe a.
  Just a
  Nothing

let foo_1 : Maybe
let foo_2 : Maybe Int
let foo_3 : Maybe Int Int
let foo_4 : Maybe Int Int Int
```

## Can indirectly apply a polymorphic datatype to some type

```
enum Maybe a.
  Just a
  Nothing

enum App a b.
  MkApp (a b)

enum Foo.
  MkFoo (App Maybe Int)

let f : Foo = MkFoo (MkApp (Just 1))
```

## Record-declarations inside enum-declarations work

```
enum Shape.
  Circle.
    radius: Int 
  Rect.
    width: Int
    height: Int

let z = Circle { radius = 12 }
let a = Rect { width = 12, height = 12 }

a == z
```

## Tuple types are correctly inferred and unified

```
let foo_1 : (Int, Int, Int) = (1, 2, 3)
let foo_2 : (Int, Int, Int) = (1, 2, "foo")
```

## Module references work

```
mod CD.
  mod A.
    struct Foo
  mod B.
    let alpha: A.Foo
```

<<<<<<< HEAD
## Rest-expressions on extensible records work

```
struct Point.
  x: Int
  y: Int
  z: Int

let foo { x, y, .. } : Point -> Int = x + y

foo { x = 1, y = 2 }
```
=======

## A polymorphic function is properly generalized when assigned to a new variable

```
let id x = x
let id2 = id
let id3 = id

id3 1
id3 "bla"

id2 1
id2 "bla"
````
>>>>>>> 11bae0fed0d58eafebd863b4e3bf117176176cb1
