| Condition | Runs | Runs with a false eviction | Honest false evictions (total, max in one run) | B banned by all its neighbours | Delivery before the fault, mean | Delivery after B is banned, mean (min of a run) | Eviction latency, median of run medians (max) | Frames dropped by the rate limit | Relay frames lost (failed delivery) | Deliveries > 5 s after origination |
|---|---|---|---|---|---|---|---|---|---|---|
| l0 | 18 | 0 | 0, 0 | 18/18 | 100.0% | 100.0% (100%) | 2.1 s (2.1 s) | 0 | 0 | 0 |
| l5 | 18 | 0 | 0, 0 | 18/18 | 100.0% | 100.0% (100%) | 2.1 s (2.2 s) | 0 | 17 | 2 |
| l10 | 18 | 0 | 0, 0 | 18/18 | 100.0% | 100.0% (100%) | 2.1 s (10.0 s) | 0 | 98 | 0 |
| l15 | 18 | 0 | 0, 0 | 18/18 | 100.0% | 100.0% (100%) | 2.2 s (6.1 s) | 0 | 315 | 78 |
| l20 | 18 | 0 | 0, 0 | 18/18 | 100.0% | 99.6% (96%) | 2.2 s (8.0 s) | 0 | 805 | 779 |
| r5 | 18 | 0 | 0, 0 | 18/18 | 100.0% | 100.0% (100%) | 2.1 s (2.1 s) | 0 | 8 | 0 |
| r10 | 18 | 0 | 0, 0 | 18/18 | 100.0% | 100.0% (100%) | 2.0 s (2.2 s) | 0 | 46 | 0 |
| r15 | 18 | 0 | 0, 0 | 18/18 | 100.0% | 100.0% (100%) | 2.0 s (2.1 s) | 0 | 132 | 0 |
| l10r10 | 18 | 0 | 0, 0 | 18/18 | 100.0% | 100.0% (100%) | 2.1 s (2.7 s) | 0 | 125 | 8 |
