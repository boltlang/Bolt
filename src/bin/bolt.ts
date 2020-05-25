#!/usr/bin/env node

import "reflect-metadata"
import "source-map-support/register"

import { sync as globSync } from "glob"
import * as path from "path"
import * as fs from "fs"
import yargs from "yargs"
import yaml from "js-yaml"
import semver from "semver"

import { Program } from "../program"
import { parseSourceFile } from "../parser"
import { Frontend } from "../frontend"
import { Package } from "../common"
import {hasOwnProperty} from "../util"
import {isString} from "util"
import {DiagnosticPrinter, E_FIELD_NOT_PRESENT, E_FIELD_MUST_BE_STRING, E_FIELD_HAS_INVALID_VERSION_NUMBER} from "../diagnostics"
import {BoltSourceFileModifiers} from "../ast"

//global.print = function (value: any) {
//  console.error(require('util').inspect(value, { depth: Infinity, colors: true }))
//}

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

const diagnostics = new DiagnosticPrinter();

function loadPackageMetadata(rootDir: string) {

  let name = null
  let version = null;

  let hasVersionErrors = false;
  let hasNameErrors = false;

  const filepath = path.join(rootDir, 'Boltfile');
  if (fs.existsSync(filepath)) {
    const data = yaml.safeLoad(fs.readFileSync(filepath, 'utf8'));
    if (data !== undefined) {
      if (hasOwnProperty(data, 'name')) {
        if (!isString(data.name)) {
          diagnostics.add({
            message: E_FIELD_MUST_BE_STRING,
            severity: 'error',
            args: { name: 'name' },
          });
          hasNameErrors = true;
        } else {
          name = data.name;
        }
      }
      if (hasOwnProperty(data, 'version')) {
        if (!isString(data.version)) {
          diagnostics.add({
            message: E_FIELD_MUST_BE_STRING,
            args: { name: 'version' },
            severity: 'error',
          });
          hasVersionErrors = true;
        } else {
          if (!semver.valid(data.version)) {
            diagnostics.add({
              message: E_FIELD_HAS_INVALID_VERSION_NUMBER,
              args: { name: 'version' },
              severity: 'error',
            });
            hasVersionErrors = true;
          } else {
            version = data.version;
          }
        }
      }
    }
  }

  if (name === null && !hasNameErrors) {
    diagnostics.add({
      message: E_FIELD_NOT_PRESENT,
      severity: 'warning',
      args: { name: 'name' },
    });
  }

  if (version === null && !hasVersionErrors) {
    diagnostics.add({
      message: E_FIELD_NOT_PRESENT,
      severity: 'warning',
      args: { name: 'version' },
    });
  }

  return {
    name,
    version
  };

}

function loadPackage(rootDir: string): Package {
  const data = loadPackageMetadata(rootDir);
  const pkg = new Package(rootDir, data.name, data.version, []);
  for (const filepath of globSync(path.join(rootDir, '**/*.bolt'))) {
    pkg.addSourceFile(parseSourceFile(filepath, pkg));
  }
  return pkg;
}

function loadPackagesAndSourceFiles(filenames: string[], cwd = '.'): Package[] {
  const anonPkg = new Package(cwd, null, null, []);
  const pkgs = [ anonPkg ];
  for (const filename of filenames) {
    if (fs.statSync(filename).isDirectory()) {
      pkgs.push(loadPackage(filename));
    } else {
      anonPkg.addSourceFile(parseSourceFile(filename, anonPkg, 0));
    }
  }
  return pkgs;
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

      const pkgs = loadPackagesAndSourceFiles(toArray(args.files as string[] | string));
      const program = new Program(pkgs);
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

