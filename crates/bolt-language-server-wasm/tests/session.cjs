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
server.writeFile("/project/.bolt/packages/std/lib.bolt", "pub fn id(x) = x;\npub fn not(b: bool) -> bool = !b;\n");
const text = 'import "std" (id, not);\nlet a: str = id(1);\nnot(2);\n';
const opened = send({ jsonrpc: "2.0", method: "textDocument/didOpen",
  params: { textDocument: { uri, languageId: "bolt", version: 1, text } } });
assert.equal(opened[0].method, "workspace/diagnostic/refresh");
assert.deepEqual(diagnostics(), ["1:13 expected `str`, found an integer", "2:4 expected `bool`, found an integer"]);

send({ jsonrpc: "2.0", method: "textDocument/didChange",
  params: { textDocument: { uri, version: 2 }, contentChanges: [{ text: 'import "std" (id);\nlet a: str = id("x");\n' }] } });
assert.deepEqual(diagnostics(), []);

const untitled = send({ jsonrpc: "2.0", method: "textDocument/didOpen",
  params: { textDocument: { uri: "untitled:Untitled-1", languageId: "bolt", version: 1, text: "" } } });
assert.equal(untitled[0].params.message, "only files are supported, not untitled:Untitled-1");
assert.throws(() => server.writeFile("relative.bolt", ""), /expected an absolute path/);

assert.deepEqual(send({ jsonrpc: "2.0", id: 10, method: "shutdown" }), [{ jsonrpc: "2.0", id: 10, result: null }]);
console.log("ok");
