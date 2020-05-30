
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
import {hasOwnProperty, upsearchSync, expandPath, FastStringMap, assert} from "../util"
import {isString} from "util"
import {DiagnosticPrinter, E_FIELD_NOT_PRESENT, E_FIELD_MUST_BE_BOOLEAN, E_FIELD_MUST_BE_STRING, E_FIELD_HAS_INVALID_VERSION_NUMBER} from "../diagnostics"

//global.print = function (value: any) {
//  console.error(require('util').inspect(value, { depth: Infinity, colors: true }))
//}

const BOLT_HOME = expandPath(process.env['BOLT_HOME'] ?? '~/.bolt-compiler')

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
  let autoImport = false;

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
      if (hasOwnProperty(data, 'auto-import')) {
        if (typeof(data['auto-import']) !== 'boolean') {
          diagnostics.add({
            message: E_FIELD_MUST_BE_BOOLEAN,
            args: { name: 'auto-import' },
            severity: 'error', 
          })
        } else {
          autoImport = data['auto-import'];
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
    version,
    autoImport,
  };

}

function loadPackageFromPath(rootDir: string, isDependency: boolean): Package {
  rootDir = path.resolve(rootDir);
  const data = loadPackageMetadata(rootDir);
  const pkg = new Package(rootDir, data.name, data.version, [], data.autoImport, isDependency);
  for (const filepath of globSync(path.join(rootDir, '**/*.bolt'))) {
    pkg.addSourceFile(parseSourceFile(filepath, pkg));
  }
  return pkg;
}

function error(message: string) {
  console.error(`Error: ${message}`);
}

function loadPackagesAndSourceFiles(filenames: string[], pkgResolver: PackageResolver, cwd = '.', useStd: boolean): Package[] {
  cwd = path.resolve(cwd);
  const anonPkg = new Package(cwd, null, null, [], false, false);
  const pkgs = [ anonPkg ];
  for (const filename of filenames) {
    if (fs.statSync(filename).isDirectory()) {
      pkgs.push(loadPackageFromPath(filename, false));
    } else {
      anonPkg.addSourceFile(parseSourceFile(filename, anonPkg));
    }
  }
  if (useStd && pkgs.find(pkg => pkg.name === 'stdlib') === undefined) {
    const resolvedPath = pkgResolver.findPackagePath('stdlib');
    if (resolvedPath === null) {
      error(`Package 'stdlib' is required to build the current source set but it was not found. Use --no-std if you know what you are doing.`);
      process.exit(1);
    }
    const stdlibPkg = loadPackageFromPath(resolvedPath, true);
    assert(stdlibPkg !== null);
    pkgs.push(stdlibPkg);
  }
  return pkgs;
}

class PackagePathResolver {

  private packageNameToPath = new FastStringMap<string, string>();

  public findPackagePath(name: string): string | null {
    if (this.packageNameToPath.has(name)) {
      return this.packageNameToPath.get(name);
    }
    return null;
  }

  public mapPackgeNameToPath(name: string, filepath: string): void {
    this.packageNameToPath.set(name, filepath);
  }

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
    'check [files..]',
    'Check the given files/packages for mistakes.',
    yargs => yargs
      .string('work-dir')
      .describe('work-dir', 'The working directory where files will be resolved against.')
      .default('work-dir', '.')
      .boolean('no-std')
      .describe('no-std', 'Do not build using the standard library.')
      .string('pkg'),
    args => {
      const useStd = args['std'] as boolean ?? true;
      const cwd = process.cwd();
      const pkgResolver = new PackagePathResolver();
      for (const pkgMapping of toArray(args.pkg as string[] | string)) {
        const [pkgName, pkgPath] = pkgMapping.split(':');
        pkgResolver.mapPackgeNameToPath(pkgName, pkgPath) 
      }
      const files = toArray(args.files as string[] | string);
      if (files.length === 0) {
        const metadataPath = upsearchSync('Boltfile');
        if (metadataPath === null) {
          error(`No source files specified on the command-line and no Boltfile found in ${cwd} or any of its parent directories.`)
          process.exit(1);
        }
        files.push(metadataPath);
      }
      const pkgs = loadPackagesAndSourceFiles(files, pkgResolver, cwd, useStd);
      const program = new Program(pkgs);
      const frontend = new Frontend();
      frontend.check(program);
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

      const cwd = process.cwd();
      const files = toArray(args.files as string[] | string);
      if (files.length === 0) {
        const metadataPath = upsearchSync('Boltfile');
        if (metadataPath === null) {
          error(`No source files specified on the command-line and no Boltfile found in ${cwd} or any of its parent directories.`)
          process.exit(1);
        }
        files.push(metadataPath);
      }
      const pkgs = loadPackagesAndSourceFiles(files);
      const program = new Program(pkgs);
      const frontend = new Frontend();
      frontend.check(program);
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

