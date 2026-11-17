# Adversary host with free identities

Mean over seeds (1, 2, 3).

| condition | runs | faithful | delivery_all_nodes | undelivered_pairs | latency_p95_ms | node_bans | adversary_identities_banned | invalid_signatures_checked | foreign_accepted_per_target_max | listener_refused | listener_evicted | attacker_connections | attacker_identities | cpu_s_targets_mean | cpu_s_others_mean | cpu_busy_pct_mean |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| ref | 3 | 3 | 1.0 | 0.0 | 309.333 | 0.0 | None | None | None | None | None | None | None | None | None | 0.903 |
| outage | 3 | 3 | 1.0 | 0.0 | 1015.333 | 0.0 | None | None | None | None | None | None | None | None | None | 0.98 |
| hold-on | 3 | 3 | 1.0 | 0.0 | 825.333 | 0.0 | 0.0 | 0.0 | 0.0 | 2140.333 | 7610.0 | 12575.667 | 15264.0 | 0.263 | None | 3.113 |
| hold-off | 3 | 3 | 1.0 | 0.0 | 813.667 | 0.0 | 0.0 | 0.0 | 0.0 | 16565.667 | 0.0 | 4867.333 | 15303.667 | 0.223 | None | 3.013 |
| forge-on | 3 | 3 | 1.0 | 0.0 | 290.333 | 0.0 | 2734.667 | 8204.0 | 0.0 | 9275.667 | 0.0 | 2745.667 | 12123.333 | 0.483 | 0.153 | 2.98 |
| forge-off | 3 | 3 | 1.0 | 0.0 | 291.333 | 0.0 | 11478.667 | 34437.667 | 0.0 | 0.0 | 0.0 | 11535.667 | 11535.667 | 1.027 | 0.153 | 5.107 |
| flood-on | 3 | 3 | 0.997 | 17.667 | 300.667 | 0.0 | 0.0 | 0.0 | 2606.667 | 161.333 | 0.0 | 275.0 | 438.0 | 4.46 | 2.79 | 8.737 |
| flood-off | 3 | 3 | 0.957 | 134.667 | 320.667 | 0.0 | 0.0 | 0.0 | 10040.0 | 0.0 | 0.0 | 503.0 | 503.0 | 9.703 | 4.6 | 14.93 |

## Criteria (fixed before the runs)

- V1 every run faithful: **holds**
- V2 hold-off: a message misses a node in every run: **FAILS**
- V3 outage: no undelivered pair in any run: **holds**
- D1 hold-on: no undelivered pair in any run: **holds**
- D2 no node banned in any run: **holds**
- D3 forge-on: broken signatures checked per target <= 20 + 20 T: **holds**
- D4 forge-on: mean delivery to all nodes >= 0.99 in every run: **holds**
- D5 flood-on: foreign messages accepted at the target <= 20 + 20 T: **holds**
