| config | runs | forged_frames_per_s_per_link | evict_median_ms | evict_max_ms | verifications_per_neighbour | frames_dropped_banned | honest_false_evictions | deliv_pre | deliv_post | cpu_mean_pct | cpu_max_pct | mem_used_mib_max | rss_mib_per_node | logs_mib | cpu_s_byzantine | cpu_s_neighbour_mean | cpu_s_neighbour_max | cpu_s_other_mean |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| fresh-1000 | 3 | 0.0 | None | None | 0.0 | 0.0 | 0.0 | 1.0 | None | 6.65 | 21.47 | 663.67 | 7.36 | 42.3 | 9.24 | 2.04 | 2.27 | 0.45 |
| reference | 3 | 0.0 | None | None | 0.0 | 0.0 | 0.0 | 1.0 | None | 0.85 | 4.1 | 625.67 | 6.95 | 4.77 | 0.17 | 0.13 | 0.16 | 0.11 |
| valid-x1 | 3 | 0.0 | None | None | 0.0 | 0.0 | 0.0 | 1.0 | None | 4.26 | 13.43 | 638.67 | 7.2 | 28.47 | 1.63 | 1.01 | 1.2 | 0.45 |
| forged-x1 | 3 | 40.0 | 127.67 | 127.67 | 3.0 | 40.0 | 0.0 | 1.0 | 1.0 | 2.55 | 12.93 | 638.67 | 7.15 | 11.96 | 2.48 | 0.54 | 0.61 | 0.21 |
| forged-x8 | 3 | 320.0 | 58.67 | 93.67 | 1.67 | 108.67 | 0.0 | 1.0 | 1.0 | 2.66 | 12.97 | 640.0 | 7.19 | 12.28 | 2.56 | 0.54 | 0.62 | 0.21 |
| forged-x64 | 3 | 2560.0 | 65.0 | 67.33 | 1.27 | 166.0 | 0.0 | 1.0 | 1.0 | 2.81 | 13.83 | 641.67 | 7.19 | 12.34 | 2.98 | 0.54 | 0.63 | 0.21 |

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
| readmission run: all 10 neighbours readmit B and accept its messages again (readmitted 10, accepted after 103) | yes |
| neighbour CPU seconds at 2 560 forged frames/s per link <= 1.5x the load-matched reference | yes |
| no frame is paced in the reference runs | yes |
| in every flood run only B's neighbours ever pace a connection | yes |
| fresh flood (1 000 messages/s): neighbour CPU seconds <= 2.5x the load-matched reference | yes |
| fresh flood: total log size <= 3x the load-matched reference | yes |
| fresh flood: honest delivery >= 0.99 | yes |
