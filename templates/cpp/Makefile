SHELL := bash
APP := templateapp
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
	vuln shell docs docs-check assets assets-check notices notices-check screenshots window-check package demo run check clean

help:
	@printf '%s\n' \
	  'make setup             check the prerequisites (scripts/dev-setup.sh --install installs them)' \
	  'make check             full local quality suite (scripts/check.sh)' \
	  'make fmt               format C++ sources and shell scripts' \
	  'make fmt-check         fail if a C++ file is not formatted' \
	  'make lint              cppcheck on everything and clang-tidy on the no-GUI build, warnings as errors' \
	  'make lint-gui          clang-tidy on the dashboard files, warnings as errors' \
	  'make test              tests of the whole project, dashboard included' \
	  'make test-nogui        tests of the no-GUI build' \
	  'make race-or-sanitize  tests under AddressSanitizer, UndefinedBehaviorSanitizer and ThreadSanitizer' \
	  'make fuzz              run every fuzz target for FUZZTIME (default 5s)' \
	  'make build             build the application with the dashboard into dist/bin' \
	  'make build-nogui       build the application without the dashboard into dist/bin' \
	  'make vuln              licenses, pinned versions and known vulnerabilities of the components' \
	  'make shell             shellcheck, shfmt and bats on scripts/' \
	  'make docs              regenerate the tables in docs/COMMANDS.md' \
	  'make assets            regenerate the icon files in assets/' \
	  'make notices           regenerate THIRD_PARTY_NOTICES.md' \
	  'make screenshots       render the dashboard screens headlessly into dist/screens' \
	  'make window-check      open the dashboard window on a virtual display (Linux)' \
	  'make package           build the release file for this operating system' \
	  'make demo              load demo data into a scratch database and open the application' \
	  'make run               build without the dashboard and open the terminal UI' \
	  'make clean             remove build/ and dist/'

setup:
	scripts/dev-setup.sh

fmt:
	scripts/format.sh --write
	shfmt -w $(SHELL_SOURCES)

fmt-check:
	scripts/format.sh

lint:
	scripts/lint.sh

lint-gui:
	scripts/lint.sh --gui

test:
	scripts/build.sh dev --test

test-nogui:
	scripts/build.sh nogui --test

race-or-sanitize:
	scripts/sanitize.sh

fuzz:
	scripts/fuzz.sh --time "$(FUZZTIME)"

build:
	scripts/build.sh release --target $(APP) --copy $(APP) "$(BIN)/$(APP)$(EXE)"

build-nogui:
	scripts/build.sh release-nogui --target $(APP) --copy $(APP) "$(BIN)/$(APP)-nogui$(EXE)"

vuln:
	scripts/vuln.sh

shell:
	shellcheck $(wildcard scripts/*.sh) $(wildcard scripts/tests/*.bash)
	shellcheck --shell=bats $(wildcard scripts/tests/*.bats)
	shfmt -d $(SHELL_SOURCES)
	bats scripts/tests

docs:
	scripts/build.sh nogui --target gendocs --run gendocs

docs-check:
	scripts/build.sh nogui --target gendocs --run gendocs -check

assets:
	scripts/build.sh nogui --target genassets --run genassets -out assets

assets-check:
	scripts/build.sh nogui --target genassets --run genassets -out assets -check

notices:
	scripts/build.sh nogui --target gennotices --run gennotices

notices-check:
	scripts/build.sh nogui --target gennotices --run gennotices -check

screenshots:
	scripts/build.sh dev --target screenshots --run screenshots -out "$(SCREENS)"

window-check:
	scripts/window-check.sh

package:
	scripts/package.sh

demo:
	scripts/demo.sh

run: build-nogui
	"$(BIN)/$(APP)-nogui$(EXE)" --ui tui

check:
	scripts/check.sh

clean:
	rm -rf build dist
