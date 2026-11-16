| attack | intensity | mode | runs | mdr | goodput_per_s | banned_by | strikes_counted | honest_rate_drops | cpu_s_neighbour_mean | host_cpu_mean | host_cpu_max | frames_per_message | faithful runs |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| flood | 40 | full | 3 | 0.983 | 46.027 | 0.0 | 0.0 | 101.0 | 0.333 | 2.027 | 3.6 | 38.08 | 3/3 |
| flood | 40 | static-bucket | 3 | 0.797 | 37.41 | 0.0 | 0.0 | 233.0 | 0.327 | 1.99 | 3.7 | 38.367 | 3/3 |
| flood | 40 | vanilla | 3 | 0.77 | 36.563 | 0.0 | 0.0 | 0.0 | 0.363 | 2.117 | 3.767 | 40.94 | 3/3 |
| flood | 100 | full | 3 | 0.983 | 46.103 | 0.0 | 0.0 | 141.667 | 0.327 | 2.017 | 3.233 | 15.613 | 3/3 |
| flood | 100 | static-bucket | 3 | 0.84 | 39.383 | 0.0 | 0.0 | 283.667 | 0.33 | 2.143 | 3.567 | 15.777 | 3/3 |
| flood | 100 | vanilla | 3 | 0.853 | 39.587 | 0.0 | 0.0 | 0.0 | 0.357 | 2.19 | 3.933 | 16.07 | 3/3 |
| flood | 200 | full | 3 | 0.973 | 46.01 | 0.0 | 0.0 | 111.333 | 0.33 | 2.137 | 4.133 | 7.843 | 3/3 |
| flood | 200 | static-bucket | 3 | 0.847 | 40.293 | 0.0 | 0.0 | 247.667 | 0.333 | 2.133 | 3.9 | 8.267 | 3/3 |
| flood | 200 | vanilla | 3 | 0.803 | 37.987 | 0.0 | 0.0 | 0.0 | 0.34 | 2.253 | 3.6 | 7.943 | 3/3 |
| none | 0 | full | 3 | 1.0 | 48.0 | 0.0 | 0.0 | 0.0 | 0.147 | 1.14 | 3.433 | 347.783 | 0/3 |
| none | 0 | static-bucket | 3 | 1.0 | 48.0 | 0.0 | 0.0 | 0.0 | 0.147 | 1.033 | 3.267 | 347.757 | 0/3 |
| none | 0 | vanilla | 3 | 1.0 | 48.0 | 0.0 | 0.0 | 0.0 | 0.147 | 1.057 | 3.233 | 347.733 | 0/3 |
| replay | 2 | full | 3 | 0.997 | 47.817 | 5.333 | 16.0 | 118.667 | 0.337 | 2.007 | 3.633 | 37.16 | 3/3 |
| replay | 2 | static-bucket | 3 | 0.757 | 35.047 | 0.0 | 169.667 | 289.0 | 0.323 | 2.017 | 3.267 | 38.95 | 3/3 |
| replay | 2 | vanilla | 3 | 0.71 | 34.103 | 0.0 | 233.667 | 0.0 | 0.34 | 1.987 | 3.233 | 39.937 | 3/3 |
| replay | 8 | full | 3 | 0.987 | 46.303 | 6.0 | 18.0 | 104.333 | 0.323 | 1.99 | 3.667 | 36.417 | 3/3 |
| replay | 8 | static-bucket | 3 | 0.843 | 40.52 | 0.0 | 799.333 | 256.0 | 0.337 | 2.017 | 3.333 | 41.197 | 3/3 |
| replay | 8 | vanilla | 3 | 0.763 | 36.643 | 0.0 | 847.333 | 0.0 | 0.343 | 2.043 | 4.3 | 40.303 | 3/3 |
| replay | 32 | full | 3 | 0.993 | 47.203 | 6.0 | 18.0 | 117.667 | 0.33 | 2.037 | 3.233 | 36.59 | 3/3 |
| replay | 32 | static-bucket | 3 | 0.897 | 42.77 | 0.0 | 745.667 | 201.333 | 0.323 | 1.967 | 3.333 | 39.48 | 3/3 |
| replay | 32 | vanilla | 3 | 0.88 | 41.82 | 0.0 | 363.0 | 0.0 | 0.373 | 2.057 | 3.3 | 40.393 | 3/3 |

| criterion | holds |
|---|---|
| full: B banned by every neighbour in every replay run | NO |
| static-bucket and vanilla: B banned by no neighbour in any run | yes |
| full: MDR >= 0.99 in every run without attack and every replay run (flood: reported only) | NO |
| 0 honest false evictions in every run | yes |
| every run faithful (the testbed's fidelity criteria; a failing run is reported, not dropped) | NO |
| no attack: every mode MDR >= 0.99 (the link rate carries the honest load) | yes |
| replay x32: full MDR exceeds vanilla MDR by at least 0.05 (isolation protects honest traffic) | yes |
