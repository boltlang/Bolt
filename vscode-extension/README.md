Bolt
====

This is an plugin providing experimental support for the Bolt programming
language in Visual Studio Code.

## Planned Features

 - Syntax highlighting with contextual information from the compiler
 - Rich type hints that appear whenever you hover an expression
 - Integration with the build tools
 - A debugger that works independently from the target platform

## Extension Settings

This extension contributes the following settings:

 - `bolt.pathToLanguageServer`: you can set this to point to a custom binary
   that will be spawned instead of the built-in language server
 - `bolt.watchLanguageServer`: set to `true` to automatically restart the
   lanuage server whenever the binary was changed on the file system

## License

This software is generously licensed under the MIT license. See `LICENSE.txt`
for more information.

