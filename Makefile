# Reproduce recorded experiment results. Linux only; the testbed needs root
# (network namespaces, netem). Output goes to OUT.
#
#   make reproduce            # every experiment wired in so far, then the tables
#   make smoke OUT=/tmp/x     # one small run of each testbed script (minutes)
#   make testbed OUT=/tmp/x   # the emulation testbed runs only
#   make netem OUT=/tmp/x     # the loss / jitter / reordering runs only (hours)
#   make attacks OUT=/tmp/x   # attacker and eventual-delivery runs
#   make tables OUT=/tmp/x    # regenerate the tables from runs already in OUT*
#   make sim OUT=/tmp/x       # the in-process simulation, 20 to 5 000 nodes (no root; about 1 h)

OUT ?= /tmp/crci-testbed
SUDO ?= sudo
BIN := target/release/crci-node
TB := scripts/testbed
SIM := target/release/examples/mesh_sim

.PHONY: build testbed netem attacks tables smoke sim reproduce

build:
	cargo build --release -p crci-node --bin crci-node

# Each runner also writes its own results.csv (report.py); netem adds netem/*.csv.
testbed: build
	$(SUDO) $(TB)/run_all.sh $(OUT) $(BIN)

netem: build
	$(SUDO) $(TB)/run_netem.sh $(OUT)-netem $(BIN)

attacks: build
	$(SUDO) $(TB)/run_attacks.sh $(OUT)-attacks $(BIN)

# Markdown for docs/benchmarks.md: the main table (testbed) and per-condition tables (netem).
tables:
	python3 $(TB)/main_table.py $(OUT) | tee $(OUT)/main-table.md
	python3 $(TB)/netem_table.py $(OUT)-netem

# Smallest end-to-end check of the netem runner and its tables: seed 1, N = 20 only (about 20 minutes).
smoke: build
	$(SUDO) $(TB)/run_netem.sh $(OUT)-smoke $(BIN) "1" "20"

# Writes results.csv, cross-check.csv (against the recorded testbed runs), tables.md and figures/.
sim:
	cargo build --release -p crci-node --example mesh_sim
	python3 scripts/sim/run_sim.py $(OUT)-sim $(SIM)
	python3 scripts/sim/run_sim.py $(OUT)-sim-timing $(SIM) --groups scale --seeds 1 --max-workers 1
	cp $(OUT)-sim-timing/results.csv $(OUT)-sim/timing.csv
	python3 scripts/sim/sim_report.py $(OUT)-sim

reproduce: testbed netem attacks tables sim
