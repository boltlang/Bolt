
## Record types can be unified without causing an error

```
struct Person.
  email: String
  age: Int

let bert = Person { email = "bar@boo.com", age = 32 }
let bob = Person { email = "boo", age = 43 }

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


### Polymorphic records can be partially typed

```
struct Timestamped a b.
  first: a
  second: b
  timestamp: Int

type Foo = Timestamped Int

type Bar = Foo Int

let t : Bar = Timestamped { first = "bar", second = 1, timestamp = 12345 }
```
