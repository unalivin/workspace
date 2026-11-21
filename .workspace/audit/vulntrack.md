# VulnTrack Requirement Audit

| # | Requirement | Status | Proof | Command |
|---|---|---|---|---|
| 1 | Threat model and architecture | Implemented | `docs/THREAT_MODEL.md`, `docs/ARCHITECTURE.md` | `verify-repo.sh` |
| 2 | SQLite schema migrations and SQL tests | Implemented | `migrations/0001_initial.sql`, `internal/db/db_test.go` | `go test ./...` (pending CI) |
| 3 | Import, deduplication, risk scoring, SLA, exceptions | Implemented | `internal/service/service.go`, `service_test.go` | `go test -race ./...` (pending CI) |
| 4 | CLI and launcher | Implemented | `cmd/vulntrack/main.go`, `main_test.go` | `go test -race ./...` (pending CI) |
| 5 | Terminal UI and Desktop GUI | Implemented | `internal/tui/tui.go`, `internal/gui/gui.go` | `go build ./...` (pending CI) |
| 6 | Packaging and documentation | Implemented | `Makefile`, `scripts/`, `docs/`, `README.md` | `verify-repo.sh` |
