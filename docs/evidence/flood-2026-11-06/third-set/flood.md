| config | runs | forged_frames_per_s_per_link | evict_median_ms | evict_max_ms | verifications_per_neighbour | frames_dropped_banned | honest_false_evictions | deliv_pre | deliv_post | cpu_mean_pct | cpu_max_pct | mem_used_mib_max | rss_mib_per_node | logs_mib | cpu_s_byzantine | cpu_s_neighbour_mean | cpu_s_neighbour_max | cpu_s_other_mean |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| reference | 3 | 0.0 | None | None | 0.0 | 0.0 | 0.0 | 1.0 | None | 0.83 | 4.3 | 608.0 | 6.92 | 4.77 | 0.14 | 0.12 | 0.15 | 0.1 |
| valid-x1 | 3 | 0.0 | None | None | 0.0 | 0.0 | 0.0 | 1.0 | None | 4.18 | 12.43 | 628.67 | 7.35 | 31.52 | 1.57 | 1.01 | 1.23 | 0.43 |
| forged-x1 | 3 | 40.0 | 111.67 | 136.67 | 3.0 | 16421.33 | 0.0 | 1.0 | 1.0 | 2.34 | 13.63 | 619.0 | 7.11 | 12.89 | 1.42 | 0.6 | 0.66 | 0.21 |
| forged-x8 | 3 | 320.0 | 59.33 | 69.33 | 1.57 | 131637.67 | 0.0 | 1.0 | 1.0 | 3.64 | 14.1 | 621.33 | 7.11 | 12.8 | 3.87 | 1.3 | 1.46 | 0.2 |
| forged-x64 | 3 | 2560.0 | 63.67 | 68.0 | 1.17 | 1053956.67 | 0.0 | 1.0 | 1.0 | 13.7 | 29.53 | 620.0 | 7.15 | 12.94 | 23.67 | 6.55 | 6.98 | 0.21 |

| criterion | holds |
|---|---|
| every run faithful | yes |
| B banned by all its neighbours in every flood run | yes |
| eviction median <= 1 s and max <= 5 s in every flood run | yes |
| <= 3 signature verifications per neighbour in every flood run | yes |
| 0 honest false evictions in every run | yes |
| mean honest delivery after the ban (whole run for the references) >= 0.99 | yes |
| host memory within 10 % of the reference run of the same seed | yes |
| total log size within 2x of the load-matched reference of the same seed | yes |
| neighbour CPU seconds at 2 560 forged frames/s per link <= 1.5x the load-matched reference | NO |
| no frame is paced in the reference runs | yes |
| in every flood run only B's neighbours ever pace a connection | yes |
