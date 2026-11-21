.PHONY: verify sync-readme gen-workflows check-all

verify:
	./scripts/verify-repo.sh

sync-readme:
	./scripts/sync-readme.sh

gen-workflows:
	./scripts/gen-workflows.sh

check-all: verify
	./scripts/check-all.sh
