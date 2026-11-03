| Condition | Runs | Runs with a false eviction | Honest false evictions (total, max in one run) | B banned by all its neighbours | Delivery before the fault, mean | Delivery after B is banned, mean (min of a run) | Eviction latency, median of run medians (max) | Frames dropped by the rate limit | Relay frames lost (failed delivery) | Deliveries > 5 s after origination |
|---|---|---|---|---|---|---|---|---|---|---|
| l0 | 18 | 0 | 0, 0 | 18/18 | 100.0% | 100.0% (100%) | 2.0 s (2.1 s) | 0 | 0 | 0 |
| l5 | 18 | 0 | 0, 0 | 18/18 | 100.0% | 100.0% (100%) | 2.1 s (2.3 s) | 0 | 5 | 0 |
| l10 | 18 | 0 | 0, 0 | 18/18 | 100.0% | 100.0% (100%) | 2.1 s (3.6 s) | 0 | 82 | 37 |
| l15 | 18 | 0 | 0, 0 | 18/18 | 100.0% | 99.9% (98%) | 2.2 s (9.6 s) | 0 | 309 | 293 |
| l20 | 18 | 0 | 0, 0 | 18/18 | 100.0% | 99.8% (99%) | 2.2 s (15.2 s) | 0 | 949 | 833 |
| r5 | 18 | 0 | 0, 0 | 18/18 | 100.0% | 100.0% (100%) | 2.0 s (2.1 s) | 0 | 0 | 0 |
| r10 | 18 | 0 | 0, 0 | 18/18 | 100.0% | 100.0% (100%) | 2.1 s (2.1 s) | 0 | 0 | 0 |
| r15 | 18 | 0 | 0, 0 | 18/18 | 100.0% | 100.0% (100%) | 2.1 s (2.2 s) | 0 | 0 | 0 |
| l10r10 | 18 | 0 | 0, 0 | 18/18 | 100.0% | 100.0% (100%) | 2.1 s (8.0 s) | 0 | 91 | 12 |
