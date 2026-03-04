.PHONY: build install uninstall clean test

PREFIX ?= /usr/local

build:
	cargo build --release

install: build
	@if [ -w "$(PREFIX)/bin" ]; then \
		install -m 755 target/release/ariadne $(PREFIX)/bin/ariadne; \
	else \
		echo "Installing to $(PREFIX)/bin (requires sudo)..."; \
		sudo install -m 755 target/release/ariadne $(PREFIX)/bin/ariadne; \
	fi
	@echo ""
	@echo "✓ ariadne installed to $(PREFIX)/bin/ariadne"
	@echo "  Run 'ariadne --help' to get started"

uninstall:
	@if [ -w "$(PREFIX)/bin" ]; then \
		rm -f $(PREFIX)/bin/ariadne; \
	else \
		sudo rm -f $(PREFIX)/bin/ariadne; \
	fi
	@echo "✓ ariadne removed"

test:
	cargo test

clean:
	cargo clean
