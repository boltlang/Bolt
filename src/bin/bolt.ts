#!/usr/bin/env node

import * as fs from "fs"

import yargs from "yargs"

import { Scanner } from "../scanner"
import { Token, TextFile } from "../ast"

function toArray<T>(value: T): T extends Array<any> ? T : T[] {
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

      for (const filepath of toArray(args.files as string[] | string)) {  

        const file = new TextFile(filepath);
        const content = fs.readFileSync(filepath, 'utf8')
        const scanner = new Scanner(file, content)
        const tokens: Token[] = [];

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

        while (true) {
          const token = scanner.scanToken()
          if (token === null) {
            break;
          }
          tokens.push(token);
        }

        for (const hook of hooks) {
          if (hook.name === 'scan' && hook.timing == 'after') {
            for (const effect of hook.effects) {
              switch (effect) {
                case 'dump':
                  console.log(JSON.stringify(tokens.map(t => t.toJSON()), undefined, 2));
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

  .help()
  .version()
  .argv

