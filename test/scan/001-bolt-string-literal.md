---
type: scan
expect: BoltStringLiteral
split-lines: true
---

A string may hold arbirary ASCII characters, including spaces:

```
"Foo!"
"Once upon a time ..."
```

Special ASCII characters have no effect, other than that they are appended to
the contents of the string:

```
"S+me w3!rd @SCII ch@r$"
```

Some special escape sequences:

```
"\n\r"
"\n"
```
