
// NOTE The code in this file is not as clean as we want it to be, but we'll be upgrading our
//      test infrastructure anyways with version 1.0.0 so it does not matter much.

import chalk from "chalk"
import { Parser as CommonmarkParser } from "commonmark"
import * as crypto from "crypto"
import { diffLines } from "diff"
import * as fs from "fs-extra"
import { sync as globSync } from "glob"
import yaml from "js-yaml"
import ora, { Ora } from "ora"
import * as path from "path"
import "reflect-metadata"
import "source-map-support/register"
import { inspect } from "util"
import yargs, { Argv } from "yargs"
import { Syntax, SyntaxKind } from "../ast"
import { Diagnostic, DiagnosticIndex, DiagnosticPrinter, E_TESTS_DO_NOT_COMPARE } from "../diagnostics"
import { Parser } from "../parser"
import { Scanner } from "../scanner"
import { TextFile, TextPos, TextSpan } from "../text"
import { deserializable, deserialize, Json, JsonObject, MapLike, serialize, serializeTag, upsearchSync, assert } from "../util"

const PACKAGE_ROOT = path.resolve(path.dirname(upsearchSync('package.json')!));
const DEFAULT_STORAGE_DIR = 'test-storage';
const STORAGE_DIR = path.join(PACKAGE_ROOT, 'test-storage');

const diagnostics = new DiagnosticPrinter();
let spinner: Ora;

// TODO move some logic from TestSession to TestSuite
// TODO hash the entire code base and have it serve as a unique key for TestSession

class FancyError extends Error {
  constructor(public message: string) {
    super(message);
  }
}

@deserializable()
class Test {

  public key: string;

  constructor(
    public readonly span: TextSpan,
    public readonly type: string,
    public readonly text: string,
    public readonly data: JsonObject,
    public result?: any,
    public error: Error | null = null
  ) {
    this.key = hash([text, data]);
  }

  /**
   * Note that tests loose their associated test results when they are serialized.
   */
  [serializeTag]() {
    return [
      this.span,
      this.type,
      this.text,
      this.data,
      this.result,
      this.error,
    ]
  }

}

interface ScanForTestsOptions {
  include: string[];
  exclude: string[];
}

function getKeyForCurrentSources() {
  const hasher = crypto.createHash('sha512');
  for (const filepath of globSync('src/**/*.ts')) {
    const contents = fs.readFileSync(filepath, 'binary');
    hasher.update(contents)
    hasher.update('\0');
  }
  return hasher.digest('hex')
}

function scanForTestsInCurrentSources(options: ScanForTestsOptions) {
  const tests: Test[] = [];
  const includes = options?.include ?? ['test/**/*.md'];
  const excludes = options?.exclude ?? [];
  spinner.text = 'Scanning for tests [0 found]';
  for (const include of includes) {
    for (const filepath of globSync(include, { ignore: excludes })) {
      spinner.info(`Found file ${filepath}`)
      for (const test of loadTests(filepath)) {
        tests.push(test);
        spinner.text = `Scanning for tests [${tests.length} found]`;
      }
    }
  }
  return tests;
}

function getSnapshotForCurrentSources(options: ScanForTestsOptions): TestSnapshot | null {
  const key = getKeyForCurrentSources();
  const tests = scanForTestsInCurrentSources(options);
  runTests(tests);
  if (tests.some(t => t.error !== null)) {
    return null;
  }
  return new TestSnapshot(key, tests);
}

function runTests(tests: Test[]): void {
  let failCount = 0;
  let i = 1;
  for (const test of tests) {
    spinner.text = `Running tests [${i}/${tests.length}]`
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
      failCount++;
      spinner.warn(`The following test from ${path.relative(process.cwd(), test.span.file.fullPath)} failed with "${e.message}":\n\n${test.text}\n`)
    }
    i++;
  }
  if (failCount > 0) {
    spinner.fail(`${failCount} tests failed.`)
  }
}

//function saveTest(test: Test) {
//  fs.mkdirpSync(path.join(STORAGE_DIR, 'tests'));
//  fs.writeFileSync(path.join(STORAGE_DIR, 'tests', test.key), JSON.stringify(serialize(test)), 'utf8');
//}

@deserializable()
class TestSnapshot {

  constructor(
    public key: string,
    public tests: Test[],
  ) {
    fs.mkdirpSync(path.join(STORAGE_DIR, 'snapshots'));
    fs.writeFileSync(path.join(STORAGE_DIR, 'snapshots', key), JSON.stringify(serialize(this)), 'utf8');
  }

  public hasFailedTests() {
    return this.tests.some(t => t.error !== null);
  }

  public saveTo(dir: string) {
    fs.mkdirpSync(dir);
    const fd = fs.openSync(path.join(dir, this.key), fs.constants.O_WRONLY);
    try {
      for (const test of this.tests) {
        assert(test.error === null);
        fs.writeSync(fd, JSON.stringify([serialize(test), serialize(test.result)]) + '\n', undefined, 'utf8');
      }
    } finally {
      fs.closeSync(fd);
    }
  }

  private [serializeTag]() {
    return [
      this.key,
      this.tests,
    ]
  }

}

function loadSnapshot(key: string): TestSnapshot {
  const resolvedKey = resolveSnapshotReference(key);
  const data = readJson(path.join(STORAGE_DIR, 'snapshots', resolvedKey));
  if (data === null) {
    throw new FancyError(`A snapshot named '${resolvedKey}' could not be loaded.`)
  }
  return deserialize(data);
}

function valueToString(value: any): string {
  return inspect(value, {
    colors: false,
    depth: Infinity,
  })
}

function compareTestSnapshots(actualSnapshot: TestSnapshot, expectedSnapshot: TestSnapshot) {

  for (const actualTest of actualSnapshot.tests) {

    const expectedTest = expectedSnapshot.tests.find(t => t.key === actualTest.key);
    if (expectedTest === undefined) {
      spinner.warn(`Test result '${actualTest.key}' has no correspoding result to compare against.`)
      continue;
    }

    const diffs = diffLines(valueToString(actualTest.result), valueToString(expectedTest.result));
    if (diffs.some(diff => diff.added || diff.removed)) {
      diagnostics.add({
        message: E_TESTS_DO_NOT_COMPARE,
        severity: 'error',
        node: actualTest,
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

function resolveSnapshotReference(reference: string): string {

  // If `name` directly refers to a snapshot, we don't have any more work to do.
  if (fs.existsSync(path.join(STORAGE_DIR, 'snapshots', reference))) {
    return reference;
  }

  // Try to read an alias, returning early if it was indeed found.
  const snapshotKey = tryReadFileSync(path.join(STORAGE_DIR, 'aliases', reference));
  if (snapshotKey !== null) {
    return snapshotKey;
  }

  // We don't support any more refs at the moment, so we indicate failure
  throw new FancyError(`A test snapshot named '${reference}' was not found.`)
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

interface CommonArgv {

}

function createTestEngineFromArgs(args: CommonArgv) {
  
}

function wrapper<T extends object>(fn: (args: T) => number | undefined) {
  return function (args: T) {
    spinner = ora(`Initializing test session ...`).start();
    let exitCode;
    try {
      exitCode = fn(args);
    } catch (e) {
      if (e instanceof FancyError) {
        spinner.fail(e.message);
        process.exit(1);
      } else {
        throw e;
      }
    }
    process.exit(exitCode ?? 0);
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
      .string('storage-dir')
      .alias('S', 'storage-dir')
      .describe('storage-dir', 'The directory where test results will be stored')
      .default('storage-dir', DEFAULT_STORAGE_DIR)
    , wrapper(args => {

      const testEngine = createTestEngineFromArgs(args);

      // Load and run all tests, saving the results to disk
      const snapshot = getSnapshotForCurrentSources(args)
      if (snapshot === null || snapshot.hasFailedTests()) {
        return 1;
      }

      for (const alias of args.alias) {
        fs.mkdirpSync(path.join(STORAGE_DIR, 'aliases'))
        fs.writeFileSync(path.join(STORAGE_DIR, 'aliases', alias), snapshot.key, 'utf8')       
      }

      const expectedSnapshot = loadSnapshot('lkg');
      compareTestSnapshots(snapshot, expectedSnapshot)

    }
  ))

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
    , wrapper(args => {

      // Load and run all tests, saving the results to disk
      const snapshot = getSnapshotForCurrentSources(args);
      if (snapshot === null || snapshot.hasFailedTests()) {
        return 1;
      }

      // Add any aliases that might have been requested for this snapshot
      fs.mkdirpSync(path.join(STORAGE_DIR, 'aliases'));
      for (const alias of args.alias) {
        fs.writeFileSync(path.join(STORAGE_DIR, 'aliases', alias), snapshot.key, 'utf8')
      }

      // Output the unqiue identifier for this snapshot
      spinner.succeed(`${snapshot.key} created.`)

    }
  ))

  .command('compare [expected] [actual]', 'Compare the output of two given tests',

    yargs => yargs
      .string('actual')
      .describe('actual', 'A reference to a snapshot that will be checked')
      .string('expected')
      .describe('expected', 'A reference to a test snapshot that will serve as the ground truth')

    , wrapper(args => {

      let expectedKey = args.expected ?? 'lkg';
      let expectedSnapshot = null;
      let actualKey = args.actual ?? null;
      let actualSnapshot = null;

      if (args.actual === undefined) {
        // Load and run all tests, saving the results to disk
        const snapshot = getSnapshotForCurrentSources(args);
        if (snapshot === null) {
          return 1;
        }
        actualSnapshot = snapshot;
        actualKey = snapshot.key;
      }

      if (expectedSnapshot === null) {
        expectedSnapshot = loadSnapshot(expectedKey);
      }
      if (actualSnapshot === null) {
        actualSnapshot = loadSnapshot(actualKey!);
      }

      spinner.info(`Comparing test snapshot ${actualKey} to ${expectedKey}`)

      compareTestSnapshots(actualSnapshot, expectedSnapshot);

    }
  ))

  .command( 'clean', 'Clean up test snapshots that are unused', 
    yargs => yargs
      .string('keep')
      .array('keep')
      .default('keep', ['lkg'])
      .describe('keep', 'Keep the given aliases and anything they refer to')
    , wrapper(args => {
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
         if (!snapshotsToKeep.has(snapshotKey)) {
           fs.removeSync(path.join(STORAGE_DIR, 'snapshots', snapshotKey));
         }
      }
      spinner.succeed('Cleanup complete.')
      return 0;
    }
  ))

  .version()
  .help()
  .argv;
