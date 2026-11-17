| Condition | Runs | Runs with a false eviction | Honest false evictions (total, max in one run) | B banned by all its neighbours | Delivery before the fault, mean | Delivery after B is banned, mean (min of a run) | Eviction latency, median of run medians (max) | Frames dropped by the rate limit | Relay frames lost (failed delivery) | Deliveries > 5 s after origination |
|---|---|---|---|---|---|---|---|---|---|---|
| l0 | 2 | 0 | 0, 0 | 2/2 | 100.0% | 100.0% (100%) | 2.0 s (2.1 s) | 0 | 0 | 0 |
| l5 | 2 | 0 | 0, 0 | 2/2 | 100.0% | 100.0% (100%) | 2.0 s (2.2 s) | 0 | 1 | 0 |
| l10 | 2 | 0 | 0, 0 | 2/2 | 100.0% | 100.0% (100%) | 2.3 s (6.3 s) | 0 | 5 | 0 |
| l15 | 2 | 0 | 0, 0 | 2/2 | 100.0% | 100.0% (100%) | 1.3 s (2.4 s) | 0 | 22 | 7 |
| l20 | 2 | 0 | 0, 0 | 2/2 | 100.0% | 97.5% (95%) | 2.2 s (12.7 s) | 0 | 25 | 159 |
| r5 | 2 | 0 | 0, 0 | 2/2 | 100.0% | 100.0% (100%) | 2.0 s (2.0 s) | 0 | 2 | 0 |
| r10 | 2 | 0 | 0, 0 | 2/2 | 100.0% | 100.0% (100%) | 2.1 s (2.1 s) | 0 | 5 | 0 |
| r15 | 2 | 0 | 0, 0 | 2/2 | 100.0% | 100.0% (100%) | 2.1 s (2.2 s) | 0 | 21 | 0 |
| l10r10 | 2 | 0 | 0, 0 | 2/2 | 100.0% | 100.0% (100%) | 2.2 s (2.3 s) | 0 | 7 | 3 |
