| config | runs | forged_frames_per_s_per_link | evict_median_ms | evict_max_ms | verifications_per_neighbour | frames_dropped_banned | honest_false_evictions | deliv_pre | deliv_post | cpu_mean_pct | cpu_max_pct | mem_used_mib_max | rss_mib_per_node | logs_mib | cpu_s_byzantine | cpu_s_neighbour_mean | cpu_s_neighbour_max | cpu_s_other_mean |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| reference | 3 | 0.0 | None | None | 0.0 | 0.0 | 0.0 | 1.0 | None | 0.88 | 3.9 | 610.67 | 6.93 | 4.77 | None | None | None | None |
| forged-x1 | 3 | 40.0 | 128.33 | 136.33 | 3.0 | 16418.67 | 0.0 | 1.0 | 1.0 | 2.47 | 13.47 | 625.33 | 7.12 | 17.13 | None | None | None | None |
| forged-x8 | 3 | 320.0 | 60.0 | 94.67 | 1.67 | 131632.0 | 0.0 | 1.0 | 1.0 | 3.94 | 14.47 | 625.67 | 7.13 | 48.55 | None | None | None | None |
| forged-x64 | 3 | 2560.0 | 64.67 | 69.67 | 1.3 | 1053883.0 | 0.0 | 1.0 | 1.0 | 15.27 | 32.13 | 633.0 | 7.24 | 299.5 | None | None | None | None |

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
