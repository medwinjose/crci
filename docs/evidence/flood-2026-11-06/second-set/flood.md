| config | runs | forged_frames_per_s_per_link | evict_median_ms | evict_max_ms | verifications_per_neighbour | frames_dropped_banned | honest_false_evictions | deliv_pre | deliv_post | cpu_mean_pct | cpu_max_pct | mem_used_mib_max | rss_mib_per_node | logs_mib | cpu_s_byzantine | cpu_s_neighbour_mean | cpu_s_neighbour_max | cpu_s_other_mean |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| reference | 3 | 0.0 | None | None | 0.0 | 0.0 | 0.0 | 1.0 | None | 0.89 | 4.03 | 607.0 | 6.97 | 4.77 | None | None | None | None |
| forged-x1 | 3 | 40.0 | 120.0 | 136.67 | 3.0 | 16420.67 | 0.0 | 1.0 | 1.0 | 2.45 | 13.8 | 622.0 | 7.17 | 12.62 | None | None | None | None |
| forged-x8 | 3 | 320.0 | 60.33 | 86.67 | 1.67 | 131635.0 | 0.0 | 1.0 | 1.0 | 3.94 | 13.9 | 620.0 | 7.17 | 12.75 | None | None | None | None |
| forged-x64 | 3 | 2560.0 | 63.33 | 67.33 | 1.27 | 1053930.67 | 0.0 | 1.0 | 1.0 | 13.27 | 28.63 | 622.67 | 7.22 | 13.09 | None | None | None | None |

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
