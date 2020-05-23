#!/usr/bin/env node

import "reflect-metadata"
import "source-map-support/register"

import yargs from "yargs"

import { Program } from "../program"
import { parseSourceFile } from "../parser"
import { BoltSourceFile} from "../ast"
import { Frontend } from "../frontend"

global.debug = function (value: any) {
  console.error(require('util').inspect(value, { depth: Infinity, colors: true }))
}

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

yargs

  .command(

    'link [name]',
    'Link projects with each other',

    yargs => yargs,

    args => {

      console.log(args.name)

    }

  )

  .command(

    'bundle [files..]',
    'Compile and optimise a set of Bolt packages/scripts', 

    yargs => yargs
      .string('work-dir')
      .describe('work-dir', 'The working directory where files will be resolved against.')
      .default('work-dir', '.')
      .string('target')
      .describe('target', 'The target language to compile to.')
      .default('target', 'JS')
      .boolean('force')
      .describe('force', 'Ignore as much errors as possible.')
      .default('force', false)

    , args => {

      const sourceFiles = toArray(args.files as string[] | string).map(parseSourceFile);
      const program = new Program(sourceFiles);
      const frontend = new Frontend();
      frontend.typeCheck(program);
      if (frontend.diagnostics.hasErrors && !args.force) {
        process.exit(1);
      }
      frontend.compile(program, args.target);

    })

  .command(

    'exec [files..]',
    'Run the specified Bolt packages/scripts',

    yargs => yargs
      .string('work-dir')
      .describe('work-dir', 'The working directory where files will be resolved against.')
      .default('work-dir', '.'),

    args => {

      const sourceFiles = toArray(args.files as string | string[]).map(parseSourceFile);

      if (sourceFiles.length === 0) {
        throw new Error(`Executing packages is not yet supported.`)
      }

      const program = new Program(sourceFiles);
      const frontend = new Frontend();
      frontend.eval(program);

    }

  )

  .help()
  .version()
  .argv

