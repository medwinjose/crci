| config | runs | forged_frames_per_s_per_link | evict_median_ms | evict_max_ms | verifications_per_neighbour | frames_dropped_banned | honest_false_evictions | deliv_pre | deliv_post | cpu_mean_pct | cpu_max_pct | mem_used_mib_max | rss_mib_per_node | logs_mib | cpu_s_byzantine | cpu_s_neighbour_mean | cpu_s_neighbour_max | cpu_s_other_mean |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| fresh-1000 | 3 | 0.0 | None | None | 0.0 | 0.0 | 0.0 | 1.0 | None | 6.42 | 19.63 | 735.67 | 7.29 | 38.4 | 8.92 | 1.92 | 2.15 | 0.43 |
| reference | 3 | 0.0 | None | None | 0.0 | 0.0 | 0.0 | 1.0 | None | 0.88 | 3.63 | 700.67 | 6.88 | 4.09 | 0.16 | 0.13 | 0.16 | 0.11 |
| valid-x1 | 3 | 0.0 | None | None | 0.0 | 0.0 | 0.0 | 1.0 | None | 4.68 | 13.97 | 712.67 | 7.13 | 25.2 | 1.93 | 1.18 | 1.38 | 0.46 |
| forged-x1 | 3 | 40.0 | 119.33 | 137.0 | 3.0 | 40.0 | 0.0 | 1.0 | 1.0 | 2.97 | 13.4 | 712.33 | 7.08 | 10.49 | 3.43 | 0.61 | 0.7 | 0.21 |
| forged-x8 | 3 | 320.0 | 60.33 | 70.0 | 1.47 | 91.33 | 0.0 | 1.0 | 1.0 | 3.02 | 13.63 | 713.67 | 7.11 | 10.76 | 3.49 | 0.64 | 0.72 | 0.21 |
| forged-x64 | 3 | 2560.0 | 66.0 | 68.33 | 1.27 | 156.33 | 0.0 | 1.0 | 1.0 | 3.08 | 12.8 | 713.67 | 7.13 | 10.8 | 3.66 | 0.65 | 0.73 | 0.21 |

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
| readmission run: all 10 neighbours readmit B and accept its messages again (readmitted 10, accepted after 167) | yes |
| neighbour CPU seconds at 2 560 forged frames/s per link <= 1.5x the load-matched reference | yes |
| no frame is paced in the reference runs | yes |
| in every flood run only B's neighbours ever pace a connection | yes |
| fresh flood (1 000 messages/s): neighbour CPU seconds <= 2.5x the load-matched reference | yes |
| fresh flood: total log size <= 3x the load-matched reference | yes |
| fresh flood: honest delivery >= 0.99 | yes |
