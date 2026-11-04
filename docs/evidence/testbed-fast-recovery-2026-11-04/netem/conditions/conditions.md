| Condition | Runs | Runs with a false eviction | Honest false evictions (total, max in one run) | B banned by all its neighbours | Delivery before the fault, mean | Delivery after B is banned, mean (min of a run) | Eviction latency, median of run medians (max) | Frames dropped by the rate limit | Relay frames lost (failed delivery) | Deliveries > 5 s after origination |
|---|---|---|---|---|---|---|---|---|---|---|
| l0 | 18 | 0 | 0, 0 | 18/18 | 100.0% | 100.0% (100%) | 2.1 s (2.1 s) | 0 | 0 | 0 |
| l5 | 18 | 0 | 0, 0 | 18/18 | 100.0% | 100.0% (100%) | 2.1 s (2.5 s) | 0 | 16 | 2 |
| l10 | 18 | 0 | 0, 0 | 18/18 | 100.0% | 100.0% (100%) | 2.1 s (6.5 s) | 0 | 86 | 142 |
| l15 | 18 | 0 | 0, 0 | 18/18 | 100.0% | 99.9% (98%) | 2.2 s (7.0 s) | 0 | 247 | 156 |
| l20 | 18 | 0 | 0, 0 | 18/18 | 100.0% | 99.9% (99%) | 2.2 s (13.1 s) | 0 | 713 | 882 |
| r5 | 18 | 0 | 0, 0 | 18/18 | 100.0% | 100.0% (100%) | 2.0 s (2.1 s) | 0 | 0 | 0 |
| r10 | 18 | 0 | 0, 0 | 18/18 | 100.0% | 100.0% (100%) | 2.0 s (2.1 s) | 0 | 0 | 0 |
| r15 | 18 | 0 | 0, 0 | 18/18 | 100.0% | 100.0% (100%) | 2.1 s (2.2 s) | 0 | 0 | 0 |
| l10r10 | 18 | 0 | 0, 0 | 18/18 | 100.0% | 100.0% (100%) | 2.1 s (6.2 s) | 0 | 78 | 2 |
