| config | runs | forged_frames_per_s_per_link | evict_median_ms | evict_max_ms | verifications_per_neighbour | frames_dropped_banned | honest_false_evictions | deliv_pre | deliv_post | cpu_mean_pct | cpu_max_pct | mem_used_mib_max | rss_mib_per_node | logs_mib | cpu_s_byzantine | cpu_s_neighbour_mean | cpu_s_neighbour_max | cpu_s_other_mean |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| fresh-1000 | 3 | 0.0 | None | None | 0.0 | 0.0 | 0.0 | 1.0 | None | 6.42 | 20.57 | 655.67 | 7.62 | 67.92 | 8.5 | 1.98 | 2.19 | 0.45 |
| reference | 3 | 0.0 | None | None | 0.0 | 0.0 | 0.0 | 1.0 | None | 0.81 | 3.87 | 608.67 | 6.99 | 4.77 | 0.15 | 0.11 | 0.15 | 0.1 |
| valid-x1 | 3 | 0.0 | None | None | 0.0 | 0.0 | 0.0 | 1.0 | None | 4.11 | 12.3 | 633.33 | 7.42 | 31.52 | 1.59 | 1.0 | 1.18 | 0.44 |
| forged-x1 | 3 | 40.0 | 128.0 | 136.33 | 3.0 | 39.0 | 0.0 | 1.0 | 1.0 | 2.68 | 13.43 | 628.67 | 7.27 | 12.71 | 2.6 | 0.55 | 0.62 | 0.2 |
| forged-x8 | 3 | 320.0 | 59.67 | 60.67 | 1.57 | 90.67 | 0.0 | 1.0 | 1.0 | 2.7 | 12.7 | 632.67 | 7.3 | 12.75 | 2.7 | 0.55 | 0.63 | 0.21 |
| forged-x64 | 3 | 2560.0 | 64.33 | 66.67 | 1.33 | 166.33 | 0.0 | 1.0 | 1.0 | 2.62 | 12.93 | 633.67 | 7.29 | 12.94 | 2.69 | 0.56 | 0.63 | 0.2 |

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
| readmission run: all 10 neighbours readmit B and accept its messages again (readmitted 10, accepted after 104) | yes |
| neighbour CPU seconds at 2 560 forged frames/s per link <= 1.5x the load-matched reference | yes |
| no frame is paced in the reference runs | yes |
| in every flood run only B's neighbours ever pace a connection | yes |
| fresh flood (1 000 messages/s): neighbour CPU seconds <= 2.5x the load-matched reference | yes |
| fresh flood: total log size <= 3x the load-matched reference | yes |
| fresh flood: honest delivery >= 0.99 | yes |
