
all: src/ast.ts
	npm test

src/ast.ts: src/ast-spec.ts
	@echo "Generating AST definitions ..."
	@mkdir -p lib/
	@tsastgen src/ast-spec.ts:src/ast.ts

.PHONY: clean

clean:
	rm -rf src/ast.ts

.PHONY: distclean

distclean:
	rm -rf lib/

