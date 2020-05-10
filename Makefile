
TREEGEN_FILES = spec/ast.txt lib/bin/bolt-treegen.js lib/treegen/parser.js lib/treegen/index.js lib/treegen/util.js src/treegen/ast-template.js

all: lib/ast.js
	bolt bundle test.bolt

lib/ast.js: $(TREEGEN_FILES)
	@echo "Generating AST definitions ..."
	@mkdir -p lib/
	@chmod +x lib/bin/*.js
	@bolt-treegen --js-file=lib/ast.js --dts-file src/ast.d.ts spec/ast.txt

lib/treegen/parser.js: src/treegen/parser.pegjs
	@echo "Generating parser ..."
	@mkdir -p lib/treegen/
	@if ! npx pegjs --output lib/treegen/parser.js src/treegen/parser.pegjs; then \
			rm -rf lib/treegen/parser.js; \
			exit 1; \
		fi

.PHONY: clean

clean:
	rm -rf lib/treegen/parser.js
	rm -rf lib/ast.js
	rm -rf src/ast.d.ts

.PHONY: distclean

distclean:
	rm -rf lib/

