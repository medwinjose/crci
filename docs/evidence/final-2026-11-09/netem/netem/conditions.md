| Condition | Runs | Runs with a false eviction | Honest false evictions (total, max in one run) | B banned by all its neighbours | Delivery before the fault, mean | Delivery after B is banned, mean (min of a run) | Eviction latency, median of run medians (max) | Frames dropped by the rate limit | Relay frames lost (failed delivery) | Deliveries > 5 s after origination |
|---|---|---|---|---|---|---|---|---|---|---|
| l0 | 18 | 0 | 0, 0 | 18/18 | 100.0% | 100.0% (100%) | 2.0 s (2.1 s) | 0 | 0 | 0 |
| l5 | 18 | 0 | 0, 0 | 18/18 | 100.0% | 100.0% (100%) | 2.1 s (2.6 s) | 0 | 9 | 0 |
| l10 | 18 | 0 | 0, 0 | 18/18 | 100.0% | 100.0% (100%) | 2.1 s (3.4 s) | 0 | 106 | 11 |
| l15 | 18 | 0 | 0, 0 | 18/18 | 100.0% | 100.0% (100%) | 2.2 s (6.4 s) | 0 | 333 | 104 |
| l20 | 18 | 0 | 0, 0 | 18/18 | 100.0% | 99.6% (94%) | 2.2 s (12.1 s) | 0 | 797 | 902 |
| r5 | 18 | 0 | 0, 0 | 18/18 | 100.0% | 100.0% (100%) | 2.0 s (2.1 s) | 0 | 11 | 0 |
| r10 | 18 | 0 | 0, 0 | 18/18 | 100.0% | 100.0% (100%) | 2.1 s (2.1 s) | 0 | 58 | 0 |
| r15 | 18 | 0 | 0, 0 | 18/18 | 100.0% | 100.0% (100%) | 2.0 s (2.2 s) | 0 | 141 | 0 |
| l10r10 | 18 | 0 | 0, 0 | 18/18 | 100.0% | 100.0% (100%) | 2.1 s (6.3 s) | 0 | 135 | 0 |
