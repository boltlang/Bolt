#!/usr/bin/env node

import "reflect-metadata"
import "source-map-support/register"

import * as path from "path"
import * as fs from "fs-extra"

import yargs from "yargs"

import { Program } from "../program"
import { TextFile } from "../ast"

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
      .default('work-dir', '.'),

    args => {

      const files = toArray(args.path as string[] | string).map(filepath => new TextFile(filepath, args['work-dir']));
      const program = new Program(files)
      program.compile("JS");

    })

  .command(

    'exec [files..]',
    'Run the specified Bolt packages/scripts',

    yargs => yargs
      .string('work-dir')
      .describe('work-dir', 'The working directory where files will be resolved against.')
      .default('work-dir', '.'),

    args => {

      const files = toArray(args.files as string | string[]).map(p => new TextFile(p));

      if (files.length > 0) {

        const program = new Program(files);

        for (const file of files) {
          program.eval(file)
        }

      } else {

        throw new Error(`Executing packages is not yet supported.`)

      }

    }

  )

  .help()
  .version()
  .argv

