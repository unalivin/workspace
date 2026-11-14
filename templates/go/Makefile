SHELL := bash
APP := templateapp
BASE_TAGS := sqlite_fts5
GUI_TAGS := $(BASE_TAGS) no_animations
NOGUI_TAGS := $(BASE_TAGS) nogui
EXE := $(shell go env GOEXE)
BIN := dist/bin
SCREENS := $(CURDIR)/dist/screens
FUZZTIME ?= 5s
GO_FILES := $(shell git ls-files -co --exclude-standard '*.go' 2>/dev/null)
SHELL_SOURCES := $(wildcard scripts/*.sh) $(wildcard scripts/tests/*.bats) $(wildcard scripts/tests/*.bash)

.PHONY: help setup fmt fmt-check lint lint-gui test test-nogui race-or-sanitize fuzz build build-nogui \
	vuln shell docs docs-check assets assets-check notices notices-check screenshots package demo run check clean

help:
	@printf '%s\n' \
	  'make setup             check the prerequisites (scripts/dev-setup.sh --install installs them)' \
	  'make check             full local quality suite (scripts/check.sh)' \
	  'make fmt               format Go sources and shell scripts' \
	  'make fmt-check         fail if a file is not formatted' \
	  'make lint              go vet and staticcheck on the no-GUI build' \
	  'make lint-gui          go vet and staticcheck on the dashboard build' \
	  'make test              tests of the dashboard build' \
	  'make test-nogui        tests of the no-GUI build' \
	  'make race-or-sanitize  tests under the race detector' \
	  'make fuzz              run every fuzz target for FUZZTIME (default 5s)' \
	  'make build             build the application with the dashboard into dist/bin' \
	  'make build-nogui       build the application without the dashboard into dist/bin' \
	  'make vuln              govulncheck' \
	  'make shell             shellcheck, shfmt and bats on scripts/' \
	  'make docs              regenerate the tables in docs/COMMANDS.md' \
	  'make assets            regenerate the icon files in assets/' \
	  'make notices           regenerate THIRD_PARTY_NOTICES.md and review licenses' \
	  'make screenshots       render the dashboard screens headlessly into dist/screens' \
	  'make package           build the release archive for this operating system' \
	  'make demo              load demo data into a scratch database and open the application' \
	  'make run               build without the dashboard and open the terminal UI' \
	  'make clean             remove dist/'

setup:
	scripts/dev-setup.sh

fmt:
	gofmt -w $(GO_FILES)
	shfmt -w $(SHELL_SOURCES)

fmt-check:
	@unformatted="$$(gofmt -l $(GO_FILES))"; \
	if [ -n "$$unformatted" ]; then \
	  printf 'not formatted (run make fmt):\n%s\n' "$$unformatted" >&2; \
	  exit 1; \
	fi

lint:
	go vet -tags "$(NOGUI_TAGS)" ./...
	staticcheck -tags "$(NOGUI_TAGS)" ./...

lint-gui:
	go vet -tags "$(GUI_TAGS)" ./...
	staticcheck -tags "$(GUI_TAGS)" ./...

test:
	go test -tags "$(GUI_TAGS)" ./...

test-nogui:
	go test -tags "$(NOGUI_TAGS)" ./...

race-or-sanitize:
	go test -race -tags "$(NOGUI_TAGS)" ./...

fuzz:
	scripts/fuzz.sh --time "$(FUZZTIME)"

build:
	go build -trimpath -tags "$(GUI_TAGS)" -o "$(BIN)/$(APP)$(EXE)" ./cmd/$(APP)

build-nogui:
	go build -trimpath -tags "$(NOGUI_TAGS)" -o "$(BIN)/$(APP)-nogui$(EXE)" ./cmd/$(APP)

vuln:
	govulncheck -tags "$(GUI_TAGS)" ./...

shell:
	shellcheck $(wildcard scripts/*.sh) $(wildcard scripts/tests/*.bash)
	shellcheck --shell=bats $(wildcard scripts/tests/*.bats)
	shfmt -d $(SHELL_SOURCES)
	bats scripts/tests

docs:
	go test -tags "$(NOGUI_TAGS)" ./tests/ -run TestGeneratedCommandTablesAreCurrent -update-docs

docs-check:
	go test -tags "$(NOGUI_TAGS)" ./tests/ -run TestGeneratedCommandTablesAreCurrent

assets:
	go run ./internal/tools/genassets -out assets

assets-check:
	go run ./internal/tools/genassets -out assets -check

notices:
	go run ./internal/tools/gennotices -tags "$(GUI_TAGS)"

notices-check:
	go run ./internal/tools/gennotices -tags "$(GUI_TAGS)" -check

screenshots:
	go test -tags "$(NOGUI_TAGS)" ./internal/gui/ -count=1 -screenshots "$(SCREENS)"

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
