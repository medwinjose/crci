# Formal Verification

CRCI employs TLA+ specifications and the TLC model checker to formally verify critical safety and liveness properties.

## Invariants Checked
- **NoForgedOrigin:** Cryptographic guarantees against forgery.
- **ReplayNeverDeliveredTwice:** Strict nonce sequence enforcement.
- **ByzantineContainment:** Honest nodes maintain reputation >= MinTrustedRep.
- **RescueNeverDropped:** Critical RESCUE and RESCUERESOLUTION messages propagate globally despite battery or queue limitations.
