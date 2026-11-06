| config | runs | forged_frames_per_s_per_link | evict_median_ms | evict_max_ms | verifications_per_neighbour | frames_dropped_banned | honest_false_evictions | deliv_pre | deliv_post | cpu_mean_pct | cpu_max_pct | mem_used_mib_max | rss_mib_per_node | logs_mib | cpu_s_byzantine | cpu_s_neighbour_mean | cpu_s_neighbour_max | cpu_s_other_mean |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| fresh-1000 | 3 | 0.0 | None | None | 0.0 | 0.0 | 0.0 | 1.0 | None | 6.13 | 19.07 | 665.33 | 7.6 | 67.11 | 8.83 | 1.95 | 2.2 | 0.41 |
| reference | 3 | 0.0 | None | None | 0.0 | 0.0 | 0.0 | 1.0 | None | 0.89 | 4.13 | 622.0 | 6.99 | 4.77 | 0.15 | 0.12 | 0.15 | 0.11 |
| valid-x1 | 3 | 0.0 | None | None | 0.0 | 0.0 | 0.0 | 1.0 | None | 4.58 | 13.3 | 642.67 | 7.41 | 31.43 | 2.03 | 1.2 | 1.45 | 0.45 |
| forged-x1 | 3 | 40.0 | 120.0 | 136.67 | 3.0 | 39.67 | 0.0 | 1.0 | 1.0 | 3.06 | 12.6 | 638.0 | 7.29 | 12.88 | 3.39 | 0.66 | 0.74 | 0.21 |
| forged-x8 | 3 | 320.0 | 68.67 | 70.33 | 1.53 | 87.67 | 0.0 | 1.0 | 1.0 | 3.16 | 13.4 | 642.33 | 7.3 | 13.0 | 3.63 | 0.67 | 0.76 | 0.22 |
| forged-x64 | 3 | 2560.0 | 65.33 | 67.0 | 1.27 | 169.67 | 0.0 | 1.0 | 1.0 | 3.05 | 12.5 | 640.67 | 7.3 | 12.93 | 3.76 | 0.66 | 0.74 | 0.21 |

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
| readmission run: all 10 neighbours readmit B and accept its messages again (readmitted 10, accepted after 90) | yes |
| neighbour CPU seconds at 2 560 forged frames/s per link <= 1.5x the load-matched reference | yes |
| no frame is paced in the reference runs | yes |
| in every flood run only B's neighbours ever pace a connection | yes |
| fresh flood (1 000 messages/s): neighbour CPU seconds <= 2.5x the load-matched reference | yes |
| fresh flood: total log size <= 3x the load-matched reference | yes |
| fresh flood: honest delivery >= 0.99 | yes |
