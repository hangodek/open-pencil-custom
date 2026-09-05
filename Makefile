.PHONY: all run web open desktop cli build test check fmt clean help

all: run

help:
	@./run help

run:
	@./run web

web:
	@./run web

open:
	@./run web -o

desktop:
	@./run desktop

cli:
	@./run cli

build:
	@./run build

test:
	@cargo test -p op-host-services -p op-host-web --features canvaskit

check:
	@cargo check --workspace

fmt:
	@cargo fmt --all

clean:
	@cargo clean
