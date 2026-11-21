.PHONY: fmt fmt-check lint test race-or-sanitize build build-nogui check clean

fmt:
	gofmt -w .

fmt-check:
	test -z "$$(gofmt -l .)"

lint:
	go vet ./...

test:
	go test -tags nogui ./...
	go test -race ./...

race-or-sanitize:
	go test -race ./...

build:
	go build -o bin/netauto ./cmd/netauto

build-nogui:
	go build -tags nogui -o bin/netauto-nogui ./cmd/netauto

check: fmt-check lint test

clean:
	rm -rf bin/
