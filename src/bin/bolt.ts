#!/usr/bin/env node

import "reflect-metadata"
import "source-map-support/register"

import * as path from "path"
import * as fs from "fs-extra"
import { spawnSync } from "child_process"

import yargs from "yargs"

import { Scanner } from "../scanner"
import { Parser } from "../parser"
import { Expander } from "../expander"
import { TypeChecker } from "../checker"
import { Compiler } from "../compiler"
import { Emitter } from "../emitter"
import { TextFile, SourceFile, setParents } from "../ast"

function toArray<T>(value: T | T[]): T[] {
  if (Array.isArray(value)) {
    return value as T[]
  }
  return value === null || value === undefined ? [] : [value]
}

function pushAll<T>(array: T[], elements: T[]) {
  for (const element of elements) {
    array.push(element);
  }
}

function stripExtension(filepath: string) {
  const i = filepath.lastIndexOf('.');
  return i !== -1 ? filepath.substring(0, i) : filepath
}

function flatMap<T>(array: T[], proc: (element: T) => T[]) {
  let out: T[] = []
  for (const element of array) {
    pushAll(out, proc(element));
  }
  return out
}

interface Hook {
  timing: 'before' | 'after'
  name: string
  effects: string[]
}

function parseHook(str: string): Hook {
  let timing: 'before' | 'after' = 'before';
  if (str[0] === '+') {
    str = str.substring(1)
    timing = 'after';
  }
  const [name, rawEffects] = str.split('=');
  return {
    timing,
    name,
    effects: rawEffects.split(','),
  }
}

yargs

  .command(

    'compile [files..]',
    'Compile a set of source files', 

    yargs => yargs
      .string('hook')
      .describe('hook', 'Add a hook to a specific compile phase. See the manual for details.'),

    args => {

      const hooks: Hook[] = toArray(args.hook as string[] | string).map(parseHook);
      const sourceFiles: SourceFile[] = [];

      for (const filepath of toArray(args.files as string[] | string)) {  

        const file = new TextFile(filepath);
        const content = fs.readFileSync(filepath, 'utf8')
        const scanner = new Scanner(file, content)

        for (const hook of hooks) {
          if (hook.name === 'scan' && hook.timing === 'before') {
            for (const effect of hook.effects) {
              switch (effect) {
                case 'abort':
                  process.exit(0);
                  break;
                default:
                  throw new Error(`Could not execute hook effect '${effect}.`);
              }
            }
          }
        }

        const sourceFile = scanner.scan();
        // while (true) {
        //   const token = scanner.scanToken()
        //   if (token === null) {
        //     break;
        //   }
        //   tokens.push(token);
        // }

        for (const hook of hooks) {
          if (hook.name === 'scan' && hook.timing == 'after') {
            for (const effect of hook.effects) {
              switch (effect) {
                case 'dump':
                  console.log(JSON.stringify(sourceFile.toJSON(), undefined, 2));
                  break;
                case 'abort':
                  process.exit(0);
                  break;
                default:
                  throw new Error(`Could not execute hook effect '${effect}'.`)
              }
            }
          }
        }

        sourceFiles.push(sourceFile);

      }

      for (const sourceFile of sourceFiles) {
        const parser = new Parser()
        const expander = new Expander(parser)
        const expandedSourceFile = expander.getFullyExpanded(sourceFile)

        for (const hook of hooks) {
          if (hook.name === 'expand' && hook.timing == 'after') {
            for (const effect of hook.effects) {
              switch (effect) {
                case 'dump':
                  console.log(JSON.stringify(expandedSourceFile.toJSON(), undefined, 2));
                  break;
                case 'abort':
                  process.exit(0);
                  break;
                default:
                  throw new Error(`Could not execute hook effect '${effect}'.`)
              }
            }
          }
        }

      }

    })

  .command(

    'exec [files..]',
    'Run the specified Bolt scripts',

    yargs =>
      yargs,

    args => {

      const parser = new Parser()

      const sourceFiles = toArray(args.files as string[]).map(filepath => {
        const file = new TextFile(filepath)
        const contents = fs.readFileSync(filepath, 'utf8')
        const scanner = new Scanner(file, contents)
        const sourceFile = scanner.scan();
        const expander = new Expander(parser)
        const expanded = expander.getFullyExpanded(sourceFile)
        // console.log(require('util').inspect(expanded.toJSON(), { colors: true, depth: Infinity }))
        setParents(expanded)
        return expanded;
      })

      const checker = new TypeChecker()
      const compiler = new Compiler(checker, { target: "JS" })
      const bundle = compiler.compile(sourceFiles)
      const emitter = new Emitter()
      for (const file of bundle) {
        const text = emitter.emit(file);
        fs.mkdirpSync('.bolt-work')
        const filepath = path.join('.bolt-work', path.relative(process.cwd(), stripExtension(path.resolve(file.loc.source)) + '.js'))
        fs.writeFileSync(filepath, text, 'utf8')
        spawnSync('node', [filepath], { stdio: 'inherit' })
      }

    }

  )

  .help()
  .version()
  .argv

