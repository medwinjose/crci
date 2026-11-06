| attack | intensity | mode | runs | mdr | goodput_per_s | banned_by | strikes_counted | honest_rate_drops | cpu_s_neighbour_mean | host_cpu_mean | host_cpu_max | frames_per_message | faithful runs |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| flood | 40 | full | 3 | 1.0 | 48.0 | 0.0 | 0.0 | 0.0 | 1.153 | 4.55 | 12.9 | 61.343 | 3/3 |
| flood | 40 | static-bucket | 3 | 0.877 | 41.83 | 0.0 | 0.0 | 4504.667 | 2.207 | 12.707 | 18.933 | 221.267 | 3/3 |
| flood | 40 | vanilla | 3 | 1.0 | 48.0 | 0.0 | 0.0 | 0.0 | 2.95 | 18.28 | 25.9 | 328.793 | 3/3 |
| flood | 100 | full | 3 | 1.0 | 46.8 | 0.0 | 0.0 | 0.0 | 1.643 | 5.063 | 14.9 | 30.89 | 3/3 |
| flood | 100 | static-bucket | 3 | 0.753 | 35.837 | 0.0 | 0.0 | 18097.0 | 3.527 | 18.117 | 25.933 | 131.89 | 3/3 |
| flood | 100 | vanilla | 3 | 1.0 | 46.8 | 0.0 | 0.0 | 0.0 | 8.097 | 47.07 | 77.2 | 328.417 | 3/3 |
| flood | 200 | full | 3 | 1.0 | 46.8 | 0.0 | 0.0 | 3.0 | 2.27 | 6.867 | 16.433 | 20.5 | 3/3 |
| flood | 200 | static-bucket | 3 | 0.743 | 35.033 | 0.0 | 0.0 | 13895.667 | 3.963 | 18.847 | 27.233 | 67.28 | 3/3 |
| flood | 200 | vanilla | 3 | 1.0 | 47.2 | 0.0 | 0.0 | 0.0 | 13.56 | 81.183 | 99.967 | 327.027 | 0/3 |
| none | 0 | full | 3 | 1.0 | 48.0 | 0.0 | 0.0 | 0.0 | 0.12 | 0.877 | 3.9 | 347.323 | 3/3 |
| none | 0 | static-bucket | 3 | 1.0 | 48.0 | 0.0 | 0.0 | 0.0 | 0.123 | 0.863 | 3.767 | 347.29 | 3/3 |
| none | 0 | vanilla | 3 | 1.0 | 48.0 | 0.0 | 0.0 | 0.0 | 0.123 | 0.857 | 3.667 | 347.273 | 3/3 |
| replay | 2 | full | 3 | 1.0 | 47.2 | 10.0 | 30.0 | 0.0 | 0.63 | 3.033 | 12.867 | 25.527 | 3/3 |
| replay | 2 | static-bucket | 3 | 0.91 | 43.727 | 0.0 | 8155.667 | 5578.667 | 2.41 | 13.57 | 19.033 | 244.287 | 3/3 |
| replay | 2 | vanilla | 3 | 1.0 | 47.6 | 0.0 | 16424.0 | 0.0 | 3.1 | 18.557 | 27.7 | 336.787 | 3/3 |
| replay | 8 | full | 3 | 1.0 | 48.0 | 10.0 | 30.0 | 2.0 | 0.62 | 2.893 | 12.833 | 25.94 | 3/3 |
| replay | 8 | static-bucket | 3 | 0.893 | 41.667 | 0.0 | 52973.0 | 2751.667 | 2.92 | 13.45 | 21.0 | 259.157 | 3/3 |
| replay | 8 | vanilla | 3 | 1.0 | 47.2 | 0.0 | 82044.667 | 0.0 | 3.22 | 15.317 | 25.933 | 296.303 | 3/3 |
| replay | 32 | full | 3 | 1.0 | 48.0 | 10.0 | 30.0 | 0.0 | 0.633 | 3.027 | 12.833 | 25.513 | 3/3 |
| replay | 32 | static-bucket | 3 | 0.997 | 47.493 | 0.0 | 49493.667 | 1199.0 | 2.887 | 9.683 | 25.867 | 201.66 | 3/3 |
| replay | 32 | vanilla | 3 | 1.0 | 47.6 | 0.0 | 51172.667 | 0.0 | 3.077 | 10.917 | 27.1 | 225.437 | 3/3 |

| criterion | holds |
|---|---|
| full: B banned by every neighbour in every replay run | yes |
| static-bucket and vanilla: B banned by no neighbour in any run | yes |
| full: MDR >= 0.99 at every intensity | yes |
| 0 honest false evictions in every run | yes |
| every run faithful (the testbed's fidelity criteria; a failing run is reported, not dropped) | NO |
