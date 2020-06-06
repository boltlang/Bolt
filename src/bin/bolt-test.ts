
import "source-map-support/register"

import * as fs from "fs-extra"
import * as path from "path"
import * as crypto from "crypto"

import yargs from "yargs"
import yaml from "js-yaml"
import { sync as globSync } from "glob"
import ora from "ora"

import { Parser } from "../parser"
import { Scanner } from "../scanner"
import { SyntaxKind, Syntax } from "../ast"
import { Json, serialize, JsonObject, MapLike, upsearchSync, deepEqual } from "../util"
import { DiagnosticIndex } from "../diagnostics"
import { TextFile, TextPos, TextSpan } from "../text"

const PACKAGE_ROOT = path.dirname(upsearchSync('package.json')!);
const STORAGE_DIR = path.join(PACKAGE_ROOT, '.test-storage');

const spinner = ora(`Initializing test session ...`).start();

function toArray<T>(value: T | T[]): T[] {
  if (Array.isArray(value)) {
    return value;
  }
  return value === undefined || value === null ? [] : [ value ]
}

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

}

interface LoadTestsOptions {
  include: string[];
  exclude: string[];
}

class TestSession {

  private failCount = 0;

  public key: string;

  constructor(private tests: Test[] = []) {
    this.key = `${Date.now().toString()}-${Math.random()}`;
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
    fs.mkdirpSync(path.join(STORAGE_DIR, 'snapshots', this.key))
    for (const test of this.tests) {
      fs.writeFileSync(path.join(STORAGE_DIR, 'snapshots', this.key, test.key), JSON.stringify(test.result), 'utf8');
    }
  }

  public hasFailedTests() {
    return this.failCount > 0;
  }

}

function compare(actual: string, expected: string) {
  for (const testKey of fs.readdirSync(path.join(STORAGE_DIR, 'snapshots', actual))) {
    const actualTestResult = readJson(path.join(STORAGE_DIR, 'snapshots', actual, testKey))!;
    const expectedTestResult = readJson(path.join(STORAGE_DIR, 'snapshots', expected, testKey));
    if (!deepEqual(actualTestResult, expectedTestResult)) {
      spinner.warn(`Test ${testKey} does not compare to its expected value.`)
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

function readJson(filename: string): Json | null {
  try {
    return JSON.parse(fs.readFileSync(filename, 'utf8'));
  } catch (e) {
    if (e.code === 'ENOENT') {
      return null
    }
    throw e;
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
    , args => {

      const session = new TestSession();
      session.scanForTests(args as LoadTestsOptions);
      session.run();
      session.save();

      if (session.hasFailedTests()) {
        return;
      }

      const expectedKey = fs.readFileSync(path.join(STORAGE_DIR, 'aliases', 'lkg'), 'utf8')
      compare(session.key, expectedKey)

    }
  )
  .command(['snapshot [name]'], 'Create a new snapshot from the output of the current compiler',
    yargs => yargs
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

      // Set the tests that we have run as being the new 'lkg'
      fs.mkdirpSync(path.join(STORAGE_DIR, 'aliases'));
      fs.writeFileSync(path.join(STORAGE_DIR, 'aliases', 'lkg'), session.key, 'utf8')
    }
  )
  .command('inspect <pattern..>', 'Inspect the ouput of a given test',
    yargs => yargs
    , args => {
    }
  )
  .version()
  .help()
  .argv;
