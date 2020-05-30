
all: lib/parser.js

lib/parser.js: src/parser.pegjs
	@echo "Generating parser ..."
	@mkdir -p lib/
	@if ! pegjs --output lib/parser.js src/parser.pegjs; then \
			rm -rf lib/parser.js; \
			exit 1; \
		fi

