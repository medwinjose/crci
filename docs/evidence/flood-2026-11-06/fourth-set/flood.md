| config | runs | forged_frames_per_s_per_link | evict_median_ms | evict_max_ms | verifications_per_neighbour | frames_dropped_banned | honest_false_evictions | deliv_pre | deliv_post | cpu_mean_pct | cpu_max_pct | mem_used_mib_max | rss_mib_per_node | logs_mib | cpu_s_byzantine | cpu_s_neighbour_mean | cpu_s_neighbour_max | cpu_s_other_mean |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| fresh-1000 | 3 | 0.0 | None | None | 0.0 | 0.0 | 0.0 | 1.0 | None | 14.84 | 22.0 | 647.33 | 7.51 | 174.6 | 21.72 | 6.33 | 6.76 | 0.42 |
| reference | 3 | 0.0 | None | None | 0.0 | 0.0 | 0.0 | 1.0 | None | 0.82 | 3.6 | 611.33 | 6.99 | 4.77 | 0.15 | 0.12 | 0.15 | 0.1 |
| valid-x1 | 3 | 0.0 | None | None | 0.0 | 0.0 | 0.0 | 1.0 | None | 4.11 | 13.4 | 632.33 | 7.41 | 31.49 | 1.58 | 1.0 | 1.2 | 0.44 |
| forged-x1 | 3 | 40.0 | 128.33 | 137.0 | 3.0 | 40.0 | 0.0 | 1.0 | 1.0 | 2.64 | 13.27 | 629.0 | 7.24 | 12.6 | 2.52 | 0.54 | 0.62 | 0.2 |
| forged-x8 | 3 | 320.0 | 60.67 | 76.0 | 1.6 | 94.0 | 0.0 | 1.0 | 1.0 | 2.67 | 12.57 | 633.67 | 7.28 | 12.8 | 2.66 | 0.55 | 0.62 | 0.21 |
| forged-x64 | 3 | 2560.0 | 63.67 | 65.67 | 1.27 | 157.67 | 0.0 | 1.0 | 1.0 | 2.72 | 12.87 | 632.67 | 7.28 | 12.94 | 2.85 | 0.56 | 0.63 | 0.2 |

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
| fresh flood (1 000 messages/s): neighbour CPU seconds <= 2.5x the load-matched reference | NO |
| fresh flood: total log size <= 3x the load-matched reference | NO |
| fresh flood: honest delivery >= 0.99 | yes |
