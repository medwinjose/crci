| config | runs | forged_frames_per_s_per_link | evict_median_ms | evict_max_ms | verifications_per_neighbour | frames_dropped_banned | honest_false_evictions | deliv_pre | deliv_post | cpu_mean_pct | cpu_max_pct | mem_used_mib_max | rss_mib_per_node | logs_mib | cpu_s_byzantine | cpu_s_neighbour_mean | cpu_s_neighbour_max | cpu_s_other_mean |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| fresh-1000 | 3 | 0.0 | None | None | 0.0 | 0.0 | 0.0 | 1.0 | None | 6.4 | 18.77 | 713.67 | 7.28 | 38.92 | 9.12 | 1.94 | 2.2 | 0.43 |
| reference | 3 | 0.0 | None | None | 0.0 | 0.0 | 0.0 | 1.0 | None | 0.89 | 3.53 | 676.0 | 6.83 | 4.09 | 0.16 | 0.12 | 0.15 | 0.11 |
| valid-x1 | 3 | 0.0 | None | None | 0.0 | 0.0 | 0.0 | 1.0 | None | 4.96 | 13.07 | 687.67 | 7.11 | 25.15 | 2.13 | 1.25 | 1.52 | 0.47 |
| forged-x1 | 3 | 40.0 | 127.67 | 136.33 | 3.0 | 39.0 | 0.0 | 1.0 | 1.0 | 3.16 | 12.83 | 688.33 | 7.05 | 10.74 | 3.71 | 0.68 | 0.76 | 0.21 |
| forged-x8 | 3 | 320.0 | 67.33 | 69.0 | 1.57 | 96.67 | 0.0 | 1.0 | 1.0 | 3.05 | 13.63 | 692.0 | 7.1 | 10.79 | 3.78 | 0.67 | 0.76 | 0.21 |
| forged-x64 | 3 | 2560.0 | 65.33 | 67.33 | 1.33 | 149.67 | 0.0 | 1.0 | 1.0 | 3.07 | 12.33 | 690.67 | 7.1 | 10.79 | 3.68 | 0.66 | 0.76 | 0.21 |

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
