# Adversary host with free identities

Mean over seeds (1, 2, 3).

| condition | runs | faithful | delivery_all_nodes | undelivered_pairs | latency_p99_ms | node_bans | adversary_identities_banned | invalid_signatures_checked | foreign_accepted_per_target_max | listener_refused | listener_evicted | attacker_connections | attacker_identities | cpu_s_targets_mean | cpu_s_others_mean | cpu_busy_pct_mean |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| ref | 3 | 3 | 1.0 | 0.0 | None | 0.0 | None | None | None | None | None | None | None | None | None | 0.887 |
| hold-on | 3 | 3 | 1.0 | 0.0 | None | 0.0 | 0.0 | 0.0 | 0.0 | 2434.333 | 8022.0 | 12798.0 | 15489.333 | 0.25 | None | 3.097 |
| hold-off | 3 | 3 | 1.0 | 0.0 | None | 0.0 | 0.0 | 0.0 | 0.0 | 16877.667 | 0.0 | 4693.0 | 15418.0 | 0.207 | None | 2.93 |
| forge-on | 3 | 3 | 1.0 | 0.0 | None | 0.0 | 2742.667 | 8228.0 | 0.0 | 602.667 | 0.0 | 2750.0 | 3359.333 | 0.447 | 0.157 | 2.17 |
| forge-off | 3 | 3 | 1.0 | 0.0 | None | 0.0 | 3309.0 | 9927.0 | 0.0 | 0.0 | 0.0 | 3315.667 | 3315.667 | 0.487 | 0.15 | 2.283 |
| flood-on | 3 | 3 | 0.997 | 18.333 | None | 0.0 | 0.0 | 0.0 | 2604.333 | 160.667 | 0.0 | 275.0 | 437.333 | 4.437 | 2.783 | 8.657 |
| flood-off | 3 | 3 | 0.963 | 137.333 | None | 0.0 | 0.0 | 0.0 | 10026.667 | 0.0 | 0.0 | 502.333 | 502.333 | 10.197 | 4.903 | 15.723 |

## Criteria (fixed before the runs)

- V1 every run faithful: **holds**
- V2 hold-off: a message misses a node in every run: **FAILS**
- V3 outage: no undelivered pair in any run: **FAILS**
- D1 hold-on: no undelivered pair in any run: **holds**
- D2 no node banned in any run: **holds**
- D3 forge-on: broken signatures checked per target <= 20 + 20 T: **holds**
- D4 forge-on: mean delivery to all nodes >= 0.99 in every run: **holds**
- D5 flood-on: foreign messages accepted at the target <= 20 + 20 T: **holds**
