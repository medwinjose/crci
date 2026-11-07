# Fuzzing

Coverage-guided fuzz targets (cargo-fuzz / libFuzzer) for the parts of `crci-core` that handle input
from other nodes. They need a nightly toolchain and Linux (WSL2 works), so this crate is not part of
the workspace and is not built by `cargo test`.

| Target | What it drives | What it checks |
|---|---|---|
| `wire_decode` | arbitrary bytes into the handshake and frame decoders, then into a node's receive path | no panic; the signed bytes of a message do not change on re-encode |
| `replay_filter` | `ReplayFilter` with arbitrary windows (0–299), origins, sequence numbers (including near `u64::MAX`) and senders (including more than the per-origin record limit) | every verdict, the number of origins and the number of peer records equal an independently written reference model |
| `runtime_state` | a whole `NodeRuntime` fed signed, tampered, repeated and junk frames from six neighbours while its clock moves, with a small origin limit so that forgetting runs | every table stays inside its limit; everything delivered carries a valid origin signature; a neighbour that only sent distinct, untampered messages is never banned |
| `sybil_guard` | `SybilGuard` with thousands of identities, strikes and clock steps | no panic; the record table never exceeds its limit |
| `log_gates` | `LogGate` and `FairLogGate` | held-back accounting and the documented bound on printed lines per window |

```bash
cargo install cargo-fuzz --locked
rustup toolchain install nightly
make fuzz                 # every target, 60 s each
make fuzz FUZZ_TIME=600   # longer
cargo +nightly fuzz run replay_filter    # one target, until stopped
```

A crash is saved under `fuzz/artifacts/<target>/`; `cargo +nightly fuzz run <target> <file>` replays it.
The corpus lives in `fuzz/corpus/` and is not committed. Each run starts from whatever corpus is there,
so a longer run continues the last one.

The targets were checked against deliberately planted bugs (an off-by-one in the window floor, peer
records not updated when the window slides, a table limit that never triggers, a log bound that is too
loose, origin keys not forgotten); each is found within two minutes. A target that has never failed
proves nothing until it has been shown able to.
