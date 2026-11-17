SHELL := bash
APP := secretsentry
NOGUI_PACKAGES := -p $(APP)-core -p $(APP)-tui -p $(APP)-app
CARGO := cargo
LOCKED := --locked
ifeq ($(OS),Windows_NT)
EXE := .exe
else
EXE :=
endif
BIN := dist/bin
SCREENS := $(CURDIR)/dist/screens
FUZZTIME ?= 5s
SHELL_SOURCES := $(wildcard scripts/*.sh) $(wildcard scripts/tests/*.bats) $(wildcard scripts/tests/*.bash)

.PHONY: help setup fmt fmt-check lint lint-gui test test-nogui race-or-sanitize fuzz build build-nogui \
	vuln shell docs docs-check assets assets-check notices notices-check screenshots package demo run check clean

help:
	@printf '%s\n' \
	  'make setup             check the prerequisites (scripts/dev-setup.sh --install installs them)' \
	  'make check             full local quality suite (scripts/check.sh)' \
	  'make fmt               format Rust sources and shell scripts' \
	  'make fmt-check         fail if a Rust file is not formatted' \
	  'make lint              cargo clippy with warnings as errors on the no-GUI build' \
	  'make lint-gui          cargo clippy with warnings as errors on the whole workspace' \
	  'make test              tests of the whole workspace, dashboard included' \
	  'make test-nogui        tests of the no-GUI build' \
	  'make race-or-sanitize  tests under AddressSanitizer (scripts/sanitize.sh)' \
	  'make fuzz              run every fuzz target for FUZZTIME (default 5s)' \
	  'make build             build the application with the dashboard into dist/bin' \
	  'make build-nogui       build the application without the dashboard into dist/bin' \
	  'make vuln              cargo audit and cargo deny' \
	  'make shell             shellcheck, shfmt and bats on scripts/' \
	  'make docs              regenerate the tables in docs/COMMANDS.md' \
	  'make assets            regenerate the icon files in assets/' \
	  'make notices           regenerate THIRD_PARTY_NOTICES.md' \
	  'make screenshots       render the dashboard screens headlessly into dist/screens' \
	  'make package           build the release file for this operating system' \
	  'make demo              load demo data into a scratch database and open the application' \
	  'make run               build without the dashboard and open the terminal UI' \
	  'make clean             remove dist/ (cargo clean removes target/)'

setup:
	scripts/dev-setup.sh

fmt:
	$(CARGO) fmt --all
	$(CARGO) fmt --all --manifest-path fuzz/Cargo.toml
	shfmt -w $(SHELL_SOURCES)

fmt-check:
	$(CARGO) fmt --all --check
	$(CARGO) fmt --all --check --manifest-path fuzz/Cargo.toml

lint:
	$(CARGO) clippy $(LOCKED) $(NOGUI_PACKAGES) --no-default-features --all-targets -- -D warnings

lint-gui:
	$(CARGO) clippy $(LOCKED) --workspace --all-targets -- -D warnings

test:
	$(CARGO) test $(LOCKED) --workspace

test-nogui:
	$(CARGO) test $(LOCKED) $(NOGUI_PACKAGES) --no-default-features

race-or-sanitize:
	scripts/sanitize.sh

fuzz:
	scripts/fuzz.sh --time "$(FUZZTIME)"

build:
	$(CARGO) build $(LOCKED) --release -p $(APP)-app
	mkdir -p "$(BIN)"
	cp "target/release/$(APP)$(EXE)" "$(BIN)/$(APP)$(EXE)"

build-nogui:
	$(CARGO) build $(LOCKED) --release -p $(APP)-app --no-default-features
	mkdir -p "$(BIN)"
	cp "target/release/$(APP)$(EXE)" "$(BIN)/$(APP)-nogui$(EXE)"

vuln:
	$(CARGO) audit
	$(CARGO) deny --locked check licenses bans sources

shell:
	shellcheck $(wildcard scripts/*.sh) $(wildcard scripts/tests/*.bash)
	shellcheck --shell=bats $(wildcard scripts/tests/*.bats)
	shfmt -d $(SHELL_SOURCES)
	bats scripts/tests

docs:
	$(CARGO) run $(LOCKED) -q -p $(APP)-app --no-default-features --example gendocs

docs-check:
	$(CARGO) run $(LOCKED) -q -p $(APP)-app --no-default-features --example gendocs -- -check

assets:
	$(CARGO) run $(LOCKED) -q -p $(APP)-core --example genassets -- -out assets

assets-check:
	$(CARGO) run $(LOCKED) -q -p $(APP)-core --example genassets -- -out assets -check

notices:
	$(CARGO) run $(LOCKED) -q -p $(APP)-app --no-default-features --example gennotices

notices-check:
	$(CARGO) run $(LOCKED) -q -p $(APP)-app --no-default-features --example gennotices -- -check

screenshots:
	$(CARGO) run $(LOCKED) -q -p $(APP)-gui --example screenshots -- -out "$(SCREENS)"

package:
	scripts/package.sh

demo:
	scripts/demo.sh

run: build-nogui
	"$(BIN)/$(APP)-nogui$(EXE)" --ui tui

check:
	scripts/check.sh

clean:
	rm -rf dist
