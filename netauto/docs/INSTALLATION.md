# Installation

Requires Go `1.27.4`:

```bash
go build -o bin/netauto ./cmd/netauto
```

For headless environments:

```bash
go build -tags nogui -o bin/netauto-nogui ./cmd/netauto
```
