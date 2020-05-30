
all: src/ast.js
	bolt check test.bolt --no-std

src/ast.js: src/ast-spec.txt
	@echo "Generating AST definitions ..."
	@mkdir -p lib/
	@treegen --js-file=src/ast.js --dts-file src/ast.d.ts src/ast-spec.txt

.PHONY: clean

clean:
	rm -rf src/ast.js
	rm -rf src/ast.d.ts

.PHONY: distclean

distclean:
	rm -rf lib/

