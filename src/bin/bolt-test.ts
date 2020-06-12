
// NOTE The code in this file is not as clean as we want it to be, but we'll be upgrading our
//      test infrastructure anyways with version 1.0.0 so it does not matter much.

import "source-map-support/register"
import "reflect-metadata"

import * as fs from "fs-extra"
import * as path from "path"
import * as crypto from "crypto"

import chalk from "chalk"
import { v4 as uuidv4 } from "uuid"
import yargs from "yargs"
import yaml, { FAILSAFE_SCHEMA } from "js-yaml"
import { sync as globSync } from "glob"
import ora, { Ora } from "ora"
import { Parser as CommonmarkParser } from "commonmark"
import { Parser } from "../parser"
import { Scanner } from "../scanner"
import { SyntaxKind, Syntax } from "../ast"
import { Json, serialize, JsonObject, MapLike, upsearchSync, deepEqual, serializeTag, deserializable, deserialize, JsonArray, verbose, diffpatcher } from "../util"
import { DiagnosticIndex, DiagnosticPrinter, E_TESTS_DO_NOT_COMPARE, E_INVALID_TEST_COMPARE, E_NO_BOLTFILE_FOUND_IN_PATH_OR_PARENT_DIRS, Diagnostic } from "../diagnostics"
import { TextFile, TextPos, TextSpan } from "../text"
import { diffLines } from "diff"
import { inspect } from "util"

const PACKAGE_ROOT = path.dirname(upsearchSync('package.json')!);
const STORAGE_DIR = path.join(PACKAGE_ROOT, '.test-storage');

const diagnostics = new DiagnosticPrinter();
let spinner: Ora;

// TODO move some logic from TestSession to TestSuite
// TODO hash the entire code base and have it serve as a unique key for TestSession

function toArray<T>(value: T | T[]): T[] {
  if (Array.isArray(value)) {
    return value;
  }
  return value === undefined || value === null ? [] : [ value ]
}

@deserializable()
class Test {

  public key: string;

  public result?: any;
  public error: Error | null = null;

  constructor(
    public readonly span: TextSpan,
    public readonly type: string,
    public readonly text: string,
    public readonly data: JsonObject,
  ) {
    this.key = hash([text, data]);
  }

  [serializeTag]() {
    return [
      this.span,
      this.type,
      this.text,
      this.data,
    ]
  }

}

interface LoadTestsOptions {
  include: string[];
  exclude: string[];
}

class TestSuite {

  constructor(private tests: Test[]) {

  }

}

class TestSession {

  private failCount = 0;

  public key: string;

  constructor(private tests: Test[] = []) {
    this.key = uuidv4();
  }

  public getAllTests() {
    return this.tests[Symbol.iterator]();;
  }

  public scanForTests(options?: LoadTestsOptions) {
    const includes = options?.include ?? ['test/**/*.md'];
    const excludes = options?.exclude ?? [];
    spinner.text = 'Scanning for tests [0 found]';
    for (const include of includes) {
      for (const filepath of globSync(include, { ignore: excludes })) {
        spinner.info(`Found file ${filepath}`)
        for (const test of loadTests(filepath)) {
          this.tests.push(test);
          spinner.text = `Scanning for tests [${this.tests.length} found]`;
        }
      }
    }
  }

  public run() {
    let i = 1;
    //let failed = [];
    for (const test of this.tests) {
      spinner.text = `Running tests [${i}/${this.tests.length}]`
      const runner = TEST_RUNNERS[test.type]
      if (runner === undefined) {
        spinner.warn(`Test runner '${test.type}' not found.`)
        continue;
      }
      let result;
      try {
        test.result = runner(test);
      } catch (e) {
        test.error = e;
        this.failCount++;
        //failed.push(test);
        spinner.warn(`The following test from ${path.relative(process.cwd(), test.span.file.fullPath)} failed with "${e.message}":\n\n${test.text}\n`)
      }
      i++;
    }
    if (this.failCount > 0) {
      spinner.fail(`${this.failCount} tests failed.`)
    }
  }

  public save() {
    fs.mkdirpSync(path.join(STORAGE_DIR, 'tests'));
    for (const test of this.tests) {
      fs.writeFileSync(path.join(STORAGE_DIR, 'tests', test.key), JSON.stringify(serialize(test)), 'utf8');
    }
    fs.mkdirpSync(path.join(STORAGE_DIR, 'snapshots', this.key))
    for (const test of this.tests) {
      fs.writeFileSync(path.join(STORAGE_DIR, 'snapshots', this.key, test.key), JSON.stringify(serialize(test.result)), 'utf8');
    }
  }

  public hasFailedTests() {
    return this.failCount > 0;
  }

}

function toString(value: any): string {
  return inspect(value, {
    colors: false,
    depth: Infinity,
  })
}

function compare(actualKey: string, expectedKey: string) {

  for (const testKey of fs.readdirSync(path.join(STORAGE_DIR, 'snapshots', actualKey))) { 

    const test = deserialize(readJson(path.join(STORAGE_DIR, 'tests', testKey)))

    const actualData = readJson(path.join(STORAGE_DIR, 'snapshots', actualKey, testKey))!;
    const expectedData = readJson(path.join(STORAGE_DIR, 'snapshots', expectedKey, testKey))!;
    const actual = deserialize(actualData);
    const expected = deserialize(expectedData);
    const diffs = diffLines(toString(actual), toString(expected));
    if (diffs.some(diff => diff.added || diff.removed)) {
      diagnostics.add({
        message: E_TESTS_DO_NOT_COMPARE,
        severity: 'error',
        node: test,
      });
      for (const diff of diffs) {
        let out = diff.value;
        if (diff.removed) {
          out = chalk.red(out);
        } else if (diff.added) {
          out = chalk.green(out);
        }
        process.stderr.write(out);
      }
      //lconsole.error(jsondiffpatch.formatters.console.format(delta, expected) + '\n');
    }

  }

}

function isWhiteSpace(ch: string): boolean {
  return /[\n\r\t ]/.test(ch);
}

interface TestFileMetadata {
  type: string;
  expect: string;
}

interface SimpleToken {
  text: string;
  startPos: TextPos;
  endPos: TextPos;
}

const PREAMBLE_START = '---\n';
const PREAMBLE_END = '---\n';

function getPreamble(text: string): string {
  if (!text.startsWith(PREAMBLE_START)) {
    return '';
  }
  let out = '';
  for (let i = PREAMBLE_START.length; i < text.length; i++) {
    if (text.startsWith(PREAMBLE_END, i)) {
      break;
    }
    out += text[i];
  }
  return out;
}

function* loadTests(filepath: string): IterableIterator<Test> {
  const file = new TextFile(filepath);
  const contents = file.getText('utf8');
  const preamble = getPreamble(contents);
  const metadata = yaml.safeLoad(preamble);
  const parser = new CommonmarkParser();
  const rootNode = parser.parse(contents);
  if (rootNode.firstChild === null) {
    return;
  }
  for (let node = rootNode.firstChild; node.next !== null; node = node.next) {
    if (node.type === 'code_block') {
      if (metadata['split-lines']) {
        let startPos = new TextPos(0, node.sourcepos[0][0], node.sourcepos[0][1]);
        startPos.advance('```')
        startPos.advance(node.info! + '\n')
        let endPos = startPos.clone();
        for (const line of node.literal!.split('\n')) {
          if (line.length > 0) {
            yield new Test(new TextSpan(file, startPos.clone(), endPos.clone()), metadata.type, line, metadata);
            startPos = endPos;
          }
          endPos.advance(line + '\n');
        }
      } else {
        const startPos = new TextPos(0, node.sourcepos[0][0], node.sourcepos[0][1]);
        const endPos = new TextPos(0, node.sourcepos[1][0], node.sourcepos[1][1]);
        yield new Test(new TextSpan(file, startPos, endPos), metadata.type, node.literal!, metadata);
      }
    }
  }
}

function findSnapshot(ref: string): string | null {

  // If `name` directly refers to a snapshot, we don't have any more work to do.
  if (fs.existsSync(path.join(STORAGE_DIR, 'snapshots', ref))) {
    return ref;
  }

  // Try to read an alias, returning early if it was indeed found
  const snapshotKey = tryReadFileSync(path.join(STORAGE_DIR, 'aliases', ref));
  if (snapshotKey !== null) {
    return snapshotKey;
  }

  // We don't support any more refs at the moment, so we indicate failure
  return null;
}

function readJson(filename: string): Json | null {
  const contents = tryReadFileSync(filename);
  if (contents === null) {
    return null;
  }
  return JSON.parse(contents);
}

function tryReadFileSync(filename: string, encoding: BufferEncoding = 'utf8'): string | null {
  try {
    return fs.readFileSync(filename, encoding);
  } catch (e) {
    if (e.code === 'ENOENT') {
      return null
    }
    throw e;
  }
}

function tryUnlinkSync(filepath: string): void {
  try {
    fs.unlinkSync(filepath);
  } catch (e) {
    if (e.code !== 'ENOENT') {
      throw e;
    }
  }
}

function hash(value: Json) {
  const hasher = crypto.createHash('sha256');
  hasher.update(JSON.stringify(value));
  return hasher.digest('hex');
}

type TestRunner = (test: Test) => Json;

const TEST_RUNNERS: MapLike<TestRunner> = {

  scan(test: Test): any {
      const diagnostics = new DiagnosticIndex;
      const scanner = new Scanner(test.span.file, test.text, test.span.start.clone());
      const tokens = []
      while (true) {
          const token = scanner.scan();
          if (token.kind === SyntaxKind.EndOfFile) {
              break;
          }
          tokens.push(token);
      }
      return {
          diagnostics: [...diagnostics.getAllDiagnostics()],
          tokens,
      };
  },

  parse(test: Test): any {
      const kind = test.data.expect ?? 'SourceFile';
      const diagnostics = new DiagnosticIndex;
      const parser = new Parser();
      const tokens = new Scanner(test.span.file, test.text);
      let results: Syntax[];
      switch (kind) {
          case 'SourceFile':
              results = [ parser.parseSourceFile(tokens) ];
              break;
          case 'SourceElements':
              results = parser.parseSourceElements(tokens);
              break;
          default:
              throw new Error(`I did not know how to parse ${kind}`)
      }
      return {
          diagnostics: [...diagnostics.getAllDiagnostics()],
          results,
      };
  },

}

const TEST_REPORTERS = {

  scan(test: Test) {
    const printer = new DiagnosticPrinter();
    for (const diagnostic of test.result!.diagnostics) {
      printer.add(diagnostic as Diagnostic);
    }
  }

}

yargs

  .command(['$0 [pattern..]', 'run [pattern..]'], 'Run all tests on the current version of the compiler',
    yargs => yargs
      .array('pattern')
      .describe('pattern', 'Only run the tests matching the given pattern')
      .array('include')
      .describe('include', 'Files to scan for tests')
      .default('include', ['test/**/*.md'])
      .array('exclude')
      .describe('exclude', 'Files to never scan for tests')
      .default('exclude', [])
      .array('alias')
      .describe('alias', 'Save the test results under the given alias')
      .default('alias', [])
    , args => {

      spinner = ora(`Initializing test session ...`).start();

      const session = new TestSession();
      session.scanForTests(args as LoadTestsOptions);
      session.run();
      session.save();

      if (session.hasFailedTests()) {
        return;
      }

      for (const alias of args.alias) {
        fs.mkdirpSync(path.join(STORAGE_DIR, 'aliases'))
        fs.writeFileSync(path.join(STORAGE_DIR, 'aliases', alias), session.key, 'utf8')       
      }

      const expectedKey = tryReadFileSync(path.join(STORAGE_DIR, 'aliases', 'lkg'), 'utf8');
      if (expectedKey === null) {
        spinner.fail(`An alias for 'lkg' was not found.`);
        process.exit(1);
      }
      compare(session.key, expectedKey)

    }
  )

  .command(['create-snapshot [alias..]'], 'Create a new snapshot from the output of the current compiler',
    yargs => yargs
      .array('alias')
      .describe('alias', 'A user-friendly name to refer to the snapshot.')
      .default('alias', [])
      .array('include')
      .describe('include', 'Files to scan for tests')
      .default('include', ['test/**/*.md'])
      .array('exclude')
      .describe('exclude', 'Files to never scan for tests')
      .default('exclude', [])
    , args => {

      spinner = ora(`Initializing test session ...`).start();

      // Load and run all tests, saving the results to disk
      const session = new TestSession();
      session.scanForTests(args as LoadTestsOptions);
      session.run();
      session.save();

      // Add any aliases that might have been requested for this snapshot
      fs.mkdirpSync(path.join(STORAGE_DIR, 'aliases'));
      for (const alias of args.alias) {
        fs.writeFileSync(path.join(STORAGE_DIR, 'aliases', alias), session.key, 'utf8')
      }
    }
  )

  .command('compare [expected] [actual]', 'Compare the output of two given tests',
    yargs => yargs
    , args => {

      spinner = ora(`Initializing test session ...`).start();

      let expectedSessionKey;
      let actualSessionKey;

      if (args.expected !== undefined) {
        expectedSessionKey = args.expected;
      } else {
        expectedSessionKey = 'lkg';
      }

      if (args.actual !== undefined) {
        actualSessionKey = args.actual;
      } else {
        // Load and run all tests, saving the results to disk
        const session = new TestSession();
        session.scanForTests(args as LoadTestsOptions);
        session.run();
        session.save();
        actualSessionKey = session.key;
      }

      spinner.info(`Comparing ${actualSessionKey} to ${expectedSessionKey}`)

      const keyA = findSnapshot(expectedSessionKey as string);
      if (keyA === null) {
        spinner.fail(`A test snapshot named '${expectedSessionKey}' was not found.`)
        return 1;
      }
      const keyB = findSnapshot(actualSessionKey as string);
      if (keyB === null) {
        spinner.fail(`A test snapshot named '${actualSessionKey}' was not found.`)
        return 1;
      }
      compare(keyA, keyB);
    }
  )

  .command( 'clean', 'Clean up test snapshots that are unused', 
    yargs => yargs
      .array('keep')
      .default('keep', ['lkg'])
      .describe('keep', 'Keep the given aliases and anything they refer to')
    , args => {
      spinner = ora(`Initializing test session ...`).start();
      const snapshotsToKeep = new Set();
      for (const alias of fs.readdirSync(path.join(STORAGE_DIR, 'aliases'))) {
        if (args.keep.indexOf(alias) !== -1) {
          const snapshotKey = tryReadFileSync(path.join(STORAGE_DIR, 'aliases', alias));
          if (snapshotKey !== null && !fs.existsSync(path.join(STORAGE_DIR, 'snapshots', snapshotKey))) {
            spinner.info(`Removing dangling alias ${alias} because the test snapshot it refers to is missing.`)
            tryUnlinkSync(path.join(STORAGE_DIR, 'aliases', alias));
          } else {
            snapshotsToKeep.add(snapshotKey);
          }
        }
      }
      for (const snapshotKey of fs.readdirSync(path.join(STORAGE_DIR, 'snapshots'))) {
         if (!(snapshotKey in snapshotsToKeep)) {
           fs.removeSync(path.join(STORAGE_DIR, 'snapshots', snapshotKey));
         }
      }
      spinner.succeed('Cleanup complete.')
    }
  )

  .version()
  .help()
  .argv;
