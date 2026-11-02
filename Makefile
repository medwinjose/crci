# Reproduce recorded experiment results. Linux only; the testbed needs root
# (network namespaces, netem). Output goes to OUT.
#
#   make reproduce            # every experiment wired in so far
#   make testbed OUT=/tmp/x   # the emulation testbed runs only
#   make netem OUT=/tmp/x     # the loss / jitter / reordering runs only (hours)

OUT ?= /tmp/crci-testbed
SUDO ?= sudo
BIN := target/release/crci-node

.PHONY: build testbed netem reproduce

build:
	cargo build --release -p crci-node --bin crci-node

testbed: build
	$(SUDO) scripts/testbed/run_all.sh $(OUT) $(BIN)

netem: build
	$(SUDO) scripts/testbed/run_netem.sh $(OUT)-netem $(BIN)

reproduce: testbed netem
