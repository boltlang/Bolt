
import "source-map-support/register"
import "reflect-metadata"

import * as fs from "fs-extra"
import * as path from "path"
import * as crypto from "crypto"

import { v4 as uuidv4 } from "uuid"
import yargs from "yargs"
import yaml, { FAILSAFE_SCHEMA } from "js-yaml"
import { sync as globSync } from "glob"
import ora from "ora"

import { Parser } from "../parser"
import { Scanner } from "../scanner"
import { SyntaxKind, Syntax } from "../ast"
import { Json, serialize, JsonObject, MapLike, upsearchSync, deepEqual, serializeTag, deserializable, deserialize } from "../util"
import { DiagnosticIndex, DiagnosticPrinter, E_TESTS_DO_NOT_COMPARE, E_INVALID_TEST_COMPARE } from "../diagnostics"
import { TextFile, TextPos, TextSpan } from "../text"

const PACKAGE_ROOT = path.dirname(upsearchSync('package.json')!);
const STORAGE_DIR = path.join(PACKAGE_ROOT, '.test-storage');

const diagnostics = new DiagnosticPrinter();
const spinner = ora(`Initializing test session ...`).start();

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

  public result?: Json;
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

function compare(actualKey: string, expectedKey: string) {

  for (const testKey of fs.readdirSync(path.join(STORAGE_DIR, 'snapshots', actualKey))) { 

    const test = deserialize(readJson(path.join(STORAGE_DIR, 'tests', testKey)))

    const actualTestData = deserialize(readJson(path.join(STORAGE_DIR, 'snapshots', actualKey, testKey))!);
    const expectedTestData = deserialize(readJson(path.join(STORAGE_DIR, 'snapshots', expectedKey, testKey)));
    if (!deepEqual(actualTestData.result, expectedTestData.result)) {
      diagnostics.add({
        message: E_TESTS_DO_NOT_COMPARE,
        severity: 'error',
        node: test,
      })
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

function* loadTests(filepath: string): IterableIterator<Test> {

  const file = new TextFile(filepath);
  const contents = file.getText('utf8');

  let i = 0;
  let column = 1
  let line = 1;
  let atNewLine = true;

  assertText('---');
  let yamlStr = '';
  i += 3;
  while (!lookaheadEquals('---')) {
    yamlStr += contents[i++];
  }
  i += 3;
  const metadata = yaml.safeLoad(yamlStr);

  while (i < contents.length) {
    skipWhiteSpace();
    if (atNewLine && column >= 5) {
      const startPos = new TextPos(i, line, column);
      const text = scanCodeBlock();
      const endPos = new TextPos(i, line, column);
      if (metadata['split-lines']) {
        for (const line of text.split('\n')) {
          if (line.trim() !== '') {
            yield new Test(new TextSpan(file, startPos.clone(), endPos), metadata.type, line, metadata);
            startPos.advance(line);
          }
        }
      } else {
        yield new Test(new TextSpan(file, startPos, endPos), metadata.type, text, metadata);
      }
    } else {
      getChar();
    }
  }

  function getChar() {
    const ch = contents[i++];
    if (ch === '\n') {
      column = 1;
      line++;
      atNewLine = true;
    } else {
      if (!isEmpty(ch)) {
        atNewLine = false;
      }
      column++;
    }
    return ch;
  }

  function assertText(str: string) {
    for (let k = 0; k < str.length; k++) {
      if (contents[i+k] !== str[k]) {
        throw new Error(`Expected '${str}' but got ${contents.substr(i, i+str.length)}`)
      }
    }
  }

  function isEmpty(ch: string): boolean {
    return /[\t ]/.test(ch);
  }

  function lookaheadEquals(str: string): boolean {
    for (let k = 0; k < str.length; k++) {
      if (contents[i+k] !== str[k]) {
        return false;
      }
    }
    return true;
  }

  function scanCodeBlock() {
    let out = ''
    while (i < contents.length) {
      const ch = getChar();
      if (ch === '\n') {
        out += ch;
        skipWhiteSpace();
        continue;
      }
      if (column < 5) {
        break;
      }
      out += ch;
    }
    return out;
  }

  function skipWhiteSpace() {
    takeWhile(isWhiteSpace)
  }

  function takeWhile(pred: (ch: string) => boolean) {
    let out = '';
    while (true) {
      const c0 = contents[i];
      if (!pred(c0)) {
        break
      }
      out += getChar();
    }
    return out;
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

  scan(test: Test): Json {
      const diagnostics = new DiagnosticIndex;
      const scanner = new Scanner(test.span.file, test.text, test.span.start);
      const tokens = []
      while (true) {
          const token = scanner.scan();
          if (token.kind === SyntaxKind.EndOfFile) {
              break;
          }
          tokens.push(token);
      }
      return serialize({
          diagnostics: [...diagnostics.getAllDiagnostics()],
          tokens,
      });
  },

  parse(test: Test): Json {
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
      return serialize({
          diagnostics: [...diagnostics.getAllDiagnostics()],
          results,
      })
  },

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
  .command('compare [snapshot-a] [snapshot-b]', 'Compare the output of two given tests',
    yargs => yargs
    , args => {
      const keyA = findSnapshot(args['snapshot-a'] as string);
      if (keyA === null) {
        spinner.fail(`A test snapshot named '${keyA}' was not found.`)
        return 1;
      }
      const keyB = findSnapshot(args['snapshot-b'] as string);
      if (keyB === null) {
        spinner.fail(`A test snapshot named '${keyB}' was not found.`)
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
