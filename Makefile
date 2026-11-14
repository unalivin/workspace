SHELL := bash
SCRIPTS := $(wildcard scripts/*.sh)
BATS_FILES := $(wildcard scripts/tests/*.bats)
SHELL_SOURCES := $(SCRIPTS) $(BATS_FILES) scripts/tests/helpers.bash

.PHONY: help check lint fmt fmt-check test verify list workflows readme check-all tools

help:
	@printf '%s\n' \
	  'make check       shellcheck, shfmt, bats and repository checks' \
	  'make lint        shellcheck on the workspace scripts' \
	  'make fmt         format the workspace scripts with shfmt' \
	  'make fmt-check   fail if shfmt would change a script' \
	  'make test        bats tests of the workspace scripts' \
	  'make verify      registry, required files, emails, comments, workflows, README table' \
	  'make list        print the project table' \
	  'make workflows   regenerate .github/workflows' \
	  'make readme      refresh the README project table' \
	  'make check-all   run "make check" in every template and project' \
	  'make tools       install the pinned shell tooling'

check: lint fmt-check test verify

lint:
	shellcheck $(SCRIPTS) scripts/tests/helpers.bash
	shellcheck --shell=bats $(BATS_FILES)

fmt:
	shfmt -w $(SHELL_SOURCES)

fmt-check:
	shfmt -d $(SHELL_SOURCES)

test:
	bats scripts/tests

verify:
	scripts/verify-repo.sh

list:
	@scripts/list-projects.sh

workflows:
	scripts/gen-workflows.sh

readme:
	scripts/sync-readme.sh

check-all:
	scripts/check-all.sh

tools:
	scripts/install-shell-tools.sh
