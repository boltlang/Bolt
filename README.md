Bolt
====

Bolt is a new programming language for rapidly building complex applications.
Bolt has an unique mix of features that allows you to write fully-fledged
applications insanely fast.

 - **Garbage collected** using state-of-the art garbage collectors where
   possible, so you don't have to worry about memory leaks.
 - **JSX support**, so you write your views in the syntax you're already
   familiar with.
 - **Advanced language feature**, such as _algebraic effects_, making things
   like React's `useState` primitive objects in the language.
 - **Cross-platform standard library**, allowing you to write your code for the
   web and the desktop at the same time.

```
fn fac(n) {
  match n {
    0 => 1,
    _ => fac(n-1),
  }
}

print(fac(5)) // 10
```

## FAQ

### Why should I choose Bolt over JavaScript?

Bolt was made to make writing user-interfaces dead-simple, while also making
sure the resulting code is really fast. You should choose Bolt if you care
about _correctness_, _performance_ and _scalability_.

 - Correctness, because Bolt has a type system that is far superior to
   JavaScript's. No more member accesses into `undefined` or other weird error
   messages.
 - Performance, because the language has been designed to leave room for a lot
   of optimisations. The garbage collector is one of the few overheads that the
   language requires.
 - Scalability, because just like Rust, Bolt takes a _functional_ approach to
   software design using type traits, favouring composition over inheritance.

### What languages inspired Bolt?

Rust and Haskell are two of my favorite languages that you'll notice Bolt
shares a lot of its syntax and semantics with.

### What's the difference between Bolt and Rust?

I really like Rust, but if I just care about writing an application I believe
Rust's memory model with its borrow checker is overkill. Having a garbage
collector certainly results in a performance penalty, but I believe that as
long as the user does not notice it, it does not really matter.

## License

Bolt itself is licensed under the GPL-3.0, because we put a lot of work in it
and we want the open-source nature of Bolt to be preserved. However, code that
is compiled using the Bolt compiler may be licensed under any license you want.

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <https://www.gnu.org/licenses/>.

