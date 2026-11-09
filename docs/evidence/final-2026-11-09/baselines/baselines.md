| attack | intensity | mode | runs | mdr | goodput_per_s | banned_by | strikes_counted | honest_rate_drops | cpu_s_neighbour_mean | host_cpu_mean | host_cpu_max | frames_per_message | faithful runs |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| flood | 40 | full | 3 | 1.0 | 47.6 | 0.0 | 0.0 | 0.0 | 1.3 | 4.917 | 13.867 | 61.34 | 3/3 |
| flood | 40 | static-bucket | 3 | 0.857 | 40.403 | 0.0 | 0.0 | 6352.0 | 2.357 | 13.887 | 20.067 | 243.903 | 3/3 |
| flood | 40 | vanilla | 3 | 1.0 | 48.0 | 0.0 | 0.0 | 0.0 | 2.9 | 17.94 | 24.567 | 328.79 | 3/3 |
| flood | 100 | full | 3 | 1.0 | 47.6 | 0.0 | 0.0 | 0.0 | 1.75 | 5.677 | 15.1 | 30.893 | 3/3 |
| flood | 100 | static-bucket | 3 | 0.733 | 35.303 | 0.0 | 0.0 | 17009.0 | 3.66 | 18.22 | 24.767 | 133.29 | 3/3 |
| flood | 100 | vanilla | 3 | 0.983 | 46.753 | 0.0 | 0.0 | 0.0 | 7.693 | 45.483 | 60.267 | 328.45 | 0/3 |
| flood | 200 | full | 3 | 1.0 | 48.0 | 0.0 | 0.0 | 3.333 | 2.273 | 6.44 | 16.2 | 20.51 | 3/3 |
| flood | 200 | static-bucket | 3 | 0.757 | 36.51 | 0.0 | 0.0 | 11210.667 | 3.583 | 17.19 | 24.2 | 62.933 | 3/3 |
| flood | 200 | vanilla | 3 | 0.603 | 28.98 | 0.0 | 0.0 | 0.0 | 13.413 | 80.187 | 99.567 | 327.273 | 0/3 |
| none | 0 | full | 3 | 1.0 | 48.0 | 0.0 | 0.0 | 0.0 | 0.133 | 0.903 | 3.833 | 347.29 | 3/3 |
| none | 0 | static-bucket | 3 | 1.0 | 48.0 | 0.0 | 0.0 | 0.0 | 0.127 | 0.923 | 3.733 | 347.323 | 3/3 |
| none | 0 | vanilla | 3 | 1.0 | 48.0 | 0.0 | 0.0 | 0.0 | 0.127 | 0.89 | 3.833 | 347.273 | 3/3 |
| replay | 2 | full | 3 | 1.0 | 47.6 | 10.0 | 30.0 | 0.667 | 0.693 | 3.227 | 12.8 | 26.06 | 3/3 |
| replay | 2 | static-bucket | 3 | 0.88 | 42.127 | 0.0 | 8164.333 | 7686.0 | 2.513 | 14.113 | 19.7 | 252.017 | 3/3 |
| replay | 2 | vanilla | 3 | 1.0 | 47.2 | 0.0 | 16415.333 | 0.0 | 3.047 | 18.147 | 24.133 | 336.787 | 3/3 |
| replay | 8 | full | 3 | 1.0 | 48.0 | 10.0 | 30.0 | 0.0 | 0.66 | 3.123 | 12.967 | 25.64 | 3/3 |
| replay | 8 | static-bucket | 3 | 0.86 | 40.943 | 0.0 | 54878.667 | 4417.667 | 3.067 | 14.543 | 22.3 | 277.603 | 3/3 |
| replay | 8 | vanilla | 3 | 1.0 | 47.2 | 0.0 | 82038.667 | 0.0 | 3.223 | 14.943 | 25.0 | 296.337 | 3/3 |
| replay | 32 | full | 3 | 1.0 | 47.2 | 10.0 | 30.0 | 0.0 | 0.687 | 3.217 | 13.167 | 25.5 | 3/3 |
| replay | 32 | static-bucket | 3 | 1.0 | 48.0 | 0.0 | 47840.0 | 878.667 | 2.8 | 9.467 | 21.167 | 193.143 | 3/3 |
| replay | 32 | vanilla | 3 | 1.0 | 46.8 | 0.0 | 50898.0 | 0.0 | 2.953 | 10.53 | 27.167 | 216.713 | 3/3 |

| criterion | holds |
|---|---|
| full: B banned by every neighbour in every replay run | yes |
| static-bucket and vanilla: B banned by no neighbour in any run | yes |
| full: MDR >= 0.99 at every intensity | yes |
| 0 honest false evictions in every run | yes |
| every run faithful (the testbed's fidelity criteria; a failing run is reported, not dropped) | NO |
