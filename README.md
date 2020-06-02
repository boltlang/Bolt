Bolt
====

<p align="center">
  <img height="100" src="https://github.com/boltlang/Bolt/blob/master/logo.png?raw=true" />
</p>

Bolt is a new programming language in the making for rapidly building complex applications.
It Bolt makes writing web applications dead-simple, and will eventually also support mobile and desktop.

Bolt has an unique mix of features that allows you to write fully-fledged
applications insanely fast, while at the same time guaranteeing that your programs
do what they are supposed to do.

 - **Garbage collected**, freeing you from worrying about memory management.
 - **JSX support**, so you write your views in the syntax you're already
   familiar with.
 - **Lots of useful language features** such as match-expressions, tuples, macros, static type checking, automatic type inference, and much more to give you the best development experience.
 - **Cross-platform standard library**, allowing you to write your code for the
   web platform and the native platform at the same time.

```
fn fac(n) {
  match n {
    0 => 1,
    _ => fac(n-1),
  }
}

println!("The faculty of {} is {}", 5, fac(5));
```

## FAQ

### Why yet another programming language?

Granted, there are _a lot_ of programming languages, but oddly enough I found myself
in the situation where I was still not completely satisfied. For example, Rust is a fantastic programming language,
but writing web applications in it feels a little counter-intuitive due to its manual memory management.
On the other hand, functional  programming languages like Haskell and Idris look like they come straight out of the future, but sometimes have unpredictable run-time performance and force you to do weird things when all you want to do is mutate a variable. Bolt aims to solve these two issues, and to be a gate into more complex compilers. Who knows what the successor will look like?

More expressiveness, less boilerplate, and most of all more safety. Bolt is my first attempt at finding it.

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
shares a lot of its syntax and semantics with. Rust is the language that inspired Bolt's syntax,
while Haskell helped guide the type checker and standard library design.
Traces of other languages can be found, too.
The macro system was inspired by Racket's macro system,
and the sucesses of garbage-collected languages like Go, JavaScript, Python and Java convinced me that
this feature is a must-have.

All in all, Bolt is a mixture of many very different programming languages, and I believe this mixture makes it unique.

### What's the difference between Bolt and Rust?

Right now, Bolt looks a lot like Rust. I think the developers of the Rust programming language did a great job at 
creating a language that is both pleasant to read and write and is open to a lot of optimisations. However,
Bolt does not come with the advanced borrow checker of the Rust language. It simply isn't needed, because Bolt uses
a garbage collector underneath, and hopefully will contain some optimisations in the future that can make your programs
as fast as Rust.

## License

Bolt itself is licensed under the GPL-3.0, because I put a lot of work in it
and I want the open-source nature of Bolt to be preserved. However, code that
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

