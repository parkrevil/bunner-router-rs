## Variables
PACKAGE := bunner_router_rs
TARGET_DIR := target
DOC_DIR := $(TARGET_DIR)/doc

## Default / Meta targets
.PHONY: all

all: format lint test

## Code Quality
.PHONY: lint format format-check

lint:
	cargo clippy --workspace --all-features --lib --bins -- -D warnings -D clippy::dbg_macro -D clippy::todo -D clippy::unimplemented -D clippy::panic -D clippy::print_stdout -D clippy::print_stderr
	cargo clippy --workspace --all-features --tests --examples --benches -- -D warnings -A dead_code -A clippy::panic -A clippy::print_stdout -A clippy::print_stderr

format:
	cargo fmt --all

format-check:
	cargo fmt --all -- --check

## Testing
.PHONY: test

test:
	INSTA_UPDATE=always RUSTFLAGS="-A dead_code" cargo nextest run --package $(PACKAGE) --no-tests=pass; \

## Benchmarking
.PHONY: bench

bench:
	cargo bench --bench bunner_router_rs

## Docs
.PHONY: doc doc-open

doc:
	cargo doc --all-features --no-deps --document-private-items

doc-open: doc
	@xdg-open $(DOC_DIR)/$(PACKAGE)/index.html 2>/dev/null || true

## Security / Quality (optional tools: cargo-audit, cargo-deny)
.PHONY: audit
audit:
	@if command -v cargo-audit >/dev/null 2>&1; then \
		cargo audit; \
	else \
		echo "cargo-audit not installed. Install with: cargo install cargo-audit" >&2; \
	fi

## Coverage
.PHONY: coverage coverage-lcov
coverage:
	@if command -v cargo-llvm-cov >/dev/null 2>&1; then \
		mkdir -p $(TARGET_DIR)/llvm-cov-target/html; \
		RUSTFLAGS="-A dead_code" cargo llvm-cov --ignore-filename-regex '($(CURDIR)/tests/.*|$(CURDIR)/src/.*_test\.rs$$)'; \
	else \
		echo "cargo-llvm-cov not installed. Install with: cargo install cargo-llvm-cov" >&2; \
	fi

coverage-lcov:
	@if command -v cargo-llvm-cov >/dev/null 2>&1; then \
		mkdir -p $(TARGET_DIR)/llvm-cov-target; \
		RUSTFLAGS="-A dead_code" cargo llvm-cov --ignore-filename-regex '($(CURDIR)/tests/.*|$(CURDIR)/src/.*_test\.rs$$)' --lcov --output-path $(TARGET_DIR)/llvm-cov-target/lcov.info; \
	else \
		echo "cargo-llvm-cov not installed. Install with: cargo install cargo-llvm-cov" >&2; \
	fi

## Release / Publish
.PHONY: publish-dry-run publish
publish-dry-run:
	cargo publish --dry-run

publish:
	cargo publish

## Cleanup
.PHONY: clean distclean
clean:
	cargo clean

distclean: clean
	rm -rf $(TARGET_DIR)

