// A session with the language server compiled to wasm, in Node. Run it
// with `scripts/smoke-test.sh`, which builds the module first.
const assert = require("node:assert/strict");
const { LanguageServer } = require(process.argv[2]);

const server = new LanguageServer();
const send = (message) => server.handle(JSON.stringify(message)).map((s) => JSON.parse(s));
const uri = "file:///project/main.bolt";
const diagnostics = () =>
  send({ jsonrpc: "2.0", id: 9, method: "textDocument/diagnostic", params: { textDocument: { uri } } })[0]
    .result.items.map((d) => `${d.range.start.line}:${d.range.start.character} ${d.message}`);

const capabilities = { workspace: { diagnostics: { refreshSupport: true } } };
const [init] = send({ jsonrpc: "2.0", id: 1, method: "initialize", params: { capabilities } });
assert.equal(init.result.serverInfo.name, "bolt-language-server");
send({ jsonrpc: "2.0", method: "initialized", params: {} });

// A package that the document imports, given by the page.
server.writeFile("/project/.bolt/packages/std/lib.bolt", "pub fn id(x) = x;\npub fn not(b: Bool) -> Bool = !b;\n");
const text = 'import "std" (id, not);\nlet a: String = id(1);\nnot(2);\n';
const opened = send({ jsonrpc: "2.0", method: "textDocument/didOpen",
  params: { textDocument: { uri, languageId: "bolt", version: 1, text } } });
assert.equal(opened[0].method, "workspace/diagnostic/refresh");
assert.deepEqual(diagnostics(), ["1:16 expected `String`, found an integer", "2:4 expected `Bool`, found an integer"]);
// The labels, like that of the annotation that says which type was expected.
const [first] = send({ jsonrpc: "2.0", id: 8, method: "textDocument/diagnostic", params: { textDocument: { uri } } })[0]
  .result.items;
assert.deepEqual(first.relatedInformation.map((r) => [r.location.uri, r.location.range.start, r.message]), [
  [uri, { line: 1, character: 7 }, "expected because of this annotation"],
]);

send({ jsonrpc: "2.0", method: "textDocument/didChange",
  params: { textDocument: { uri, version: 2 }, contentChanges: [{ text: 'import "std" (id);\nlet a: String = id("x");\n' }] } });
assert.deepEqual(diagnostics(), []);

// The prelude of `std`, which the code gets without importing it.
server.writeFile("/project/.bolt/packages/std/__prelude__/lib.bolt", "pub fn twice(x: i32) -> i32 = x * 2;\n");
send({ jsonrpc: "2.0", method: "textDocument/didChange",
  params: { textDocument: { uri, version: 3 }, contentChanges: [{ text: 'let a: String = twice(1);\n' }] } });
assert.deepEqual(diagnostics(), ["0:16 expected `String`, found `i32`"]);

const untitled = send({ jsonrpc: "2.0", method: "textDocument/didOpen",
  params: { textDocument: { uri: "untitled:Untitled-1", languageId: "bolt", version: 1, text: "" } } });
assert.equal(untitled[0].params.message, "only files are supported, not untitled:Untitled-1");
assert.throws(() => server.writeFile("relative.bolt", ""), /expected an absolute path/);

assert.deepEqual(send({ jsonrpc: "2.0", id: 10, method: "shutdown" }), [{ jsonrpc: "2.0", id: 10, result: null }]);
console.log("ok");
