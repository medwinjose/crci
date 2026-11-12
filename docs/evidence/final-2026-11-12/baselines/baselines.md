| attack | intensity | mode | runs | mdr | goodput_per_s | banned_by | strikes_counted | honest_rate_drops | cpu_s_neighbour_mean | host_cpu_mean | host_cpu_max | frames_per_message | faithful runs |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| flood | 40 | full | 3 | 1.0 | 48.0 | 0.0 | 0.0 | 0.0 | 1.207 | 4.627 | 13.167 | 61.333 | 3/3 |
| flood | 40 | static-bucket | 3 | 0.883 | 42.51 | 0.0 | 0.0 | 9136.333 | 2.517 | 14.54 | 22.1 | 256.76 | 3/3 |
| flood | 40 | vanilla | 3 | 1.0 | 46.8 | 0.0 | 0.0 | 0.0 | 2.937 | 18.127 | 24.567 | 328.8 | 3/3 |
| flood | 100 | full | 3 | 1.0 | 47.2 | 0.0 | 0.0 | 0.0 | 1.643 | 5.147 | 14.867 | 30.903 | 3/3 |
| flood | 100 | static-bucket | 3 | 0.75 | 35.37 | 0.0 | 0.0 | 15969.0 | 3.58 | 17.82 | 24.867 | 126.6 | 3/3 |
| flood | 100 | vanilla | 3 | 0.983 | 47.15 | 0.0 | 0.0 | 0.0 | 7.827 | 46.173 | 61.533 | 328.413 | 0/3 |
| flood | 200 | full | 3 | 1.0 | 46.8 | 0.0 | 0.0 | 1.667 | 2.2 | 6.497 | 15.7 | 20.507 | 3/3 |
| flood | 200 | static-bucket | 3 | 0.813 | 39.053 | 0.0 | 0.0 | 12926.333 | 3.79 | 17.463 | 24.533 | 65.16 | 3/3 |
| flood | 200 | vanilla | 3 | 0.603 | 28.753 | 0.0 | 0.0 | 0.0 | 13.433 | 80.843 | 99.833 | 327.15 | 0/3 |
| none | 0 | full | 3 | 1.0 | 48.0 | 0.0 | 0.0 | 0.0 | 0.123 | 0.92 | 3.7 | 347.323 | 3/3 |
| none | 0 | static-bucket | 3 | 1.0 | 48.0 | 0.0 | 0.0 | 0.0 | 0.127 | 0.93 | 3.933 | 347.29 | 3/3 |
| none | 0 | vanilla | 3 | 1.0 | 48.0 | 0.0 | 0.0 | 0.0 | 0.123 | 0.867 | 3.6 | 347.323 | 3/3 |
| replay | 2 | full | 3 | 1.0 | 48.0 | 10.0 | 30.0 | 0.0 | 0.657 | 3.187 | 15.333 | 26.047 | 3/3 |
| replay | 2 | static-bucket | 3 | 0.887 | 42.52 | 0.0 | 8199.667 | 5629.333 | 2.45 | 13.663 | 23.867 | 238.403 | 3/3 |
| replay | 2 | vanilla | 3 | 1.0 | 47.6 | 0.0 | 16415.667 | 0.0 | 3.08 | 18.63 | 27.133 | 336.783 | 3/3 |
| replay | 8 | full | 3 | 1.0 | 48.0 | 10.0 | 30.0 | 0.667 | 0.633 | 2.973 | 13.4 | 25.54 | 3/3 |
| replay | 8 | static-bucket | 3 | 0.897 | 43.02 | 0.0 | 52708.333 | 4241.0 | 3.077 | 14.05 | 24.267 | 273.997 | 3/3 |
| replay | 8 | vanilla | 3 | 1.0 | 47.6 | 0.0 | 82045.0 | 0.0 | 3.24 | 14.79 | 25.967 | 296.323 | 3/3 |
| replay | 32 | full | 3 | 1.0 | 48.0 | 10.0 | 30.0 | 0.0 | 0.643 | 3.077 | 12.733 | 26.003 | 3/3 |
| replay | 32 | static-bucket | 3 | 1.0 | 47.977 | 0.0 | 50769.0 | 1434.667 | 2.933 | 10.26 | 23.067 | 208.533 | 3/3 |
| replay | 32 | vanilla | 3 | 1.0 | 48.0 | 0.0 | 49339.667 | 0.0 | 2.96 | 10.457 | 23.9 | 215.88 | 3/3 |

| criterion | holds |
|---|---|
| full: B banned by every neighbour in every replay run | yes |
| static-bucket and vanilla: B banned by no neighbour in any run | yes |
| full: MDR >= 0.99 at every intensity | yes |
| 0 honest false evictions in every run | yes |
| every run faithful (the testbed's fidelity criteria; a failing run is reported, not dropped) | NO |
