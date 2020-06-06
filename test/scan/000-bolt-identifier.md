---
type: scan
expect: BoltIdentifier
split-lines: true
---

The most simple identifiers are those made out of ASCII letters:

    Foo
    Bar
    Baz

However, they may also contain digits as long as they do not begin with a
digit:

    Var1
    Var2
    Var10029384

Identifiers may be as long as you want:

    ThisIsALongAndValidIdentifier
    ThisIsAnEvenLongButStilCompletelyValidIdentifier

Moreover, they may have arbitrary underscores (`_`) in their names.

    a_valid_identifier
    another__0000__valid_identfier
    _1
    __2
    ___3

They may even be nothing more than underscores:

    _
    __
    ___

All identifiers starting with a `ID_Start` character are valid identifiers,
including `Other_ID_Start`:

    ℘rototype
    ℮llipsis

Likewise, the following code points using `Other_ID_Continue` are also valid:

    α·β
    ano·teleia

And, of course, the combination of `ID_Start` and `ID_Continue`:

    alfa·beta

