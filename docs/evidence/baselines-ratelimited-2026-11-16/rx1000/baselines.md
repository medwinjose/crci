| attack | intensity | mode | runs | mdr | goodput_per_s | banned_by | strikes_counted | honest_rate_drops | cpu_s_neighbour_mean | host_cpu_mean | host_cpu_max | frames_per_message | faithful runs |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| flood | 40 | full | 3 | 1.0 | 47.6 | 0.0 | 0.0 | 0.0 | 0.937 | 5.23 | 10.6 | 61.387 | 3/3 |
| flood | 40 | static-bucket | 3 | 0.96 | 45.51 | 0.0 | 0.0 | 3033.333 | 0.853 | 6.357 | 11.067 | 125.067 | 3/3 |
| flood | 40 | vanilla | 3 | 0.703 | 33.26 | 0.0 | 0.0 | 0.0 | 0.853 | 6.043 | 10.2 | 132.52 | 3/3 |
| flood | 100 | full | 3 | 1.0 | 47.2 | 0.0 | 0.0 | 57.0 | 0.803 | 4.373 | 9.333 | 27.837 | 3/3 |
| flood | 100 | static-bucket | 3 | 0.983 | 47.103 | 0.0 | 0.0 | 2345.333 | 0.83 | 6.15 | 10.033 | 49.003 | 3/3 |
| flood | 100 | vanilla | 3 | 0.65 | 29.937 | 0.0 | 0.0 | 0.0 | 0.87 | 6.13 | 10.5 | 53.447 | 3/3 |
| flood | 200 | full | 3 | 0.99 | 46.77 | 0.0 | 0.0 | 38.333 | 0.73 | 4.057 | 9.533 | 13.223 | 3/3 |
| flood | 200 | static-bucket | 3 | 0.99 | 47.46 | 0.0 | 0.0 | 2473.667 | 0.843 | 6.717 | 14.267 | 24.61 | 3/3 |
| flood | 200 | vanilla | 3 | 0.74 | 34.85 | 0.0 | 0.0 | 0.0 | 0.84 | 5.997 | 10.133 | 26.963 | 1/3 |
| none | 0 | full | 3 | 1.0 | 48.0 | 0.0 | 0.0 | 0.0 | 0.137 | 1.067 | 5.1 | 347.297 | 1/3 |
| none | 0 | static-bucket | 3 | 1.0 | 48.0 | 0.0 | 0.0 | 0.0 | 0.14 | 1.043 | 4.5 | 347.357 | 1/3 |
| none | 0 | vanilla | 3 | 1.0 | 48.0 | 0.0 | 0.0 | 0.0 | 0.13 | 1.033 | 4.667 | 347.343 | 1/3 |
| replay | 2 | full | 3 | 1.0 | 48.0 | 10.0 | 30.0 | 0.0 | 0.39 | 2.923 | 9.9 | 25.0 | 3/3 |
| replay | 2 | static-bucket | 3 | 0.967 | 46.427 | 0.0 | 1548.333 | 1969.667 | 0.86 | 6.333 | 11.2 | 123.4 | 3/3 |
| replay | 2 | vanilla | 3 | 0.743 | 35.383 | 0.0 | 3373.333 | 0.0 | 0.827 | 5.99 | 11.067 | 130.487 | 3/3 |
| replay | 8 | full | 3 | 1.0 | 47.2 | 10.0 | 30.0 | 0.0 | 0.383 | 2.977 | 10.3 | 24.847 | 3/3 |
| replay | 8 | static-bucket | 3 | 0.997 | 47.87 | 0.0 | 7248.0 | 873.0 | 0.8 | 5.837 | 11.0 | 110.28 | 3/3 |
| replay | 8 | vanilla | 3 | 0.983 | 46.45 | 0.0 | 9201.0 | 0.0 | 0.78 | 5.55 | 10.533 | 119.787 | 3/3 |
| replay | 32 | full | 3 | 1.0 | 47.6 | 10.0 | 30.0 | 0.0 | 0.387 | 2.977 | 10.1 | 24.083 | 3/3 |
| replay | 32 | static-bucket | 3 | 1.0 | 47.427 | 0.0 | 19008.333 | 447.333 | 0.85 | 5.223 | 14.667 | 90.977 | 3/3 |
| replay | 32 | vanilla | 3 | 0.993 | 46.93 | 0.0 | 14316.0 | 0.0 | 0.717 | 4.86 | 10.867 | 101.9 | 3/3 |

| criterion | holds |
|---|---|
| full: B banned by every neighbour in every replay run | yes |
| static-bucket and vanilla: B banned by no neighbour in any run | yes |
| full: MDR >= 0.99 in every run without attack and every replay run (flood: reported only) | yes |
| 0 honest false evictions in every run | yes |
| every run faithful (the testbed's fidelity criteria; a failing run is reported, not dropped) | NO |
| no attack: every mode MDR >= 0.99 (the link rate carries the honest load) | yes |
| replay x32: full MDR exceeds vanilla MDR by at least 0.05 (isolation protects honest traffic) | NO |
