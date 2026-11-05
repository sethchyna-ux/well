# ==============================================================================
# Makefile for Well ("Phrear") Systems & Terminal Architecture
# ==============================================================================

.PHONY: all check test test-crate debug debug-trace debug-lldb dev dev-watch zig package bench rerun clean doctor help core-metrics wasm graphics mobile cloud pipelines

PIPELINE = ./scripts/well-pipeline.sh
TARGET ?= all
CRATE ?=

all: check test package

help:
	@$(PIPELINE) help

doctor:
	@$(PIPELINE) doctor

zig:
	@$(PIPELINE) zig

check:
	@$(PIPELINE) check $(CRATE)

test:
	@$(PIPELINE) test

test-crate:
	@if [ -z "$(CRATE)" ]; then \
		echo "[ERROR] Please specify CRATE=<name>, e.g. make test-crate CRATE=well-config"; \
		exit 1; \
	fi
	@$(PIPELINE) test --crate $(CRATE)

dev:
	@$(PIPELINE) dev $(if $(CRATE),--crate $(CRATE),)

dev-watch:
	@$(PIPELINE) dev --watch $(if $(CRATE),--crate $(CRATE),)

debug:
	@$(PIPELINE) debug

debug-trace:
	@$(PIPELINE) debug --trace

debug-lldb:
	@$(PIPELINE) debug --trace --lldb

package:
	@$(PIPELINE) package

bench:
	@$(PIPELINE) bench

rerun:
	@$(PIPELINE) rerun $(TARGET)

core-metrics:
	@$(PIPELINE) pipeline core-metrics

wasm:
	@$(PIPELINE) pipeline wasm

graphics:
	@$(PIPELINE) pipeline graphics

mobile:
	@$(PIPELINE) pipeline mobile

cloud:
	@$(PIPELINE) pipeline cloud

pipelines:
	@$(PIPELINE) pipeline all-stacks

clean:
	@$(PIPELINE) clean all
