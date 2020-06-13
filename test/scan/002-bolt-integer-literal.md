---
type: scan
expect: BoltIntegerLiteral
split-lines: true
---

All decimal digits are valid integers.

```
1
2
3
4
5
6
7
8
9
0
```

Any combination of decimal digits are valid integers, including integers
prefixed with an abirary amount of zeroes.

```
12345
99
10
01
000
0010
```

In binary mode, integers are read in base-2.

```
0b0
0b1
0b10010
0b00100
0b00000
```

This means the following expressions are invalid in binary mode:

```
0b20001
0b12345
0b00003
```

In octal mode, integers are read in base-8.

```
0o0
0o00000
0o007
0o706
0o12345
```

This means the following expressions are invalid in octal mode:

```
0o8
0o9
0o123456789
```

