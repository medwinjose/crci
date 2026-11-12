## Strike threshold k against honest duplicates (simulation)

| q | k | honest links ever banned | honest nodes banned | attacker coverage | attacker frames admitted per link | eviction median (simulated s) | delivery from the fault |
|---|---|---|---|---|---|---|---|
| 0 | 1 | 0 | 0 | 1 | 171 | 0.0383 | 0.997 |
| 0 | 3 | 0 | 0 | 1 | 177 | 0.288 | 0.997 |
| 0 | 5 | 0 | 0 | 1 | 181 | 0.542 | 0.997 |
| 1e-05 | 1 | 0.0553 | 0.248 | 1 | 171 | 0.0383 | 0.996 |
| 1e-05 | 3 | 0 | 0 | 1 | 177 | 0.288 | 0.997 |
| 1e-05 | 5 | 0 | 0 | 1 | 181 | 0.542 | 0.997 |
| 3e-05 | 1 | 0.145 | 0.543 | 1 | 171 | 0.0383 | 0.992 |
| 3e-05 | 3 | 0 | 0 | 1 | 177 | 0.288 | 0.997 |
| 3e-05 | 5 | 0 | 0 | 1 | 181 | 0.542 | 0.997 |
| 0.0001 | 1 | 0.405 | 0.907 | 1 | 171 | 0.0383 | 0.978 |
| 0.0001 | 3 | 0.00505 | 0.0296 | 1 | 177 | 0.288 | 0.997 |
| 0.0001 | 5 | 0.000254 | 0.00185 | 1 | 181 | 0.542 | 0.997 |
| 0.0003 | 1 | 0.692 | 0.983 | 1 | 171 | 0.0383 | 0.782 |
| 0.0003 | 3 | 0.109 | 0.489 | 1 | 177 | 0.288 | 0.997 |
| 0.0003 | 5 | 0.0105 | 0.0611 | 1 | 181 | 0.542 | 0.997 |
| 0.001 | 1 | 0.901 | 0.996 | 1 | 171 | 0.04 | 0.456 |
| 0.001 | 3 | 0.676 | 0.985 | 1 | 177 | 0.288 | 0.854 |
| 0.001 | 5 | 0.412 | 0.904 | 1 | 181 | 0.542 | 0.971 |

| criterion | holds |
|---|---|
| at every q, honest links ever banned is non-increasing in k | yes |
| at every q, attacker frames admitted per link is non-decreasing in k | yes |
| attacker coverage is 100 % in every run, for every k and q | yes |
| at q = 0 no honest link is banned, for any k | yes |

## Defence modes against replay and flood (simulation)

| attack | intensity | mode | delivery from the fault | goodput (deliveries/s) | frames per honest delivery | verifications per honest delivery | attacker coverage | attacker frames admitted per link | rate-limit drops | origin-budget drops |
|---|---|---|---|---|---|---|---|---|---|---|
| flood | 2 | full | 0.992 | 706 | 20.9 | 4.16 | 0 | 1.53e+03 | 3.36e+03 | 0 |
| flood | 2 | static-bucket | 0.992 | 706 | 20.9 | 4.16 | 0 | 1.53e+03 | 3.36e+03 | 0 |
| flood | 2 | vanilla | 1 | 712 | 20.8 | 4.16 | 0 | 1.53e+03 | 0 | 0 |
| flood | 8 | full | 0.975 | 694 | 45.5 | 9 | 0 | 3.44e+03 | 8.72e+04 | 1.64e+04 |
| flood | 8 | static-bucket | 0.849 | 604 | 64.5 | 11.8 | 0 | 4.34e+03 | 2.83e+05 | 0 |
| flood | 8 | vanilla | 1 | 712 | 62.8 | 12.5 | 0 | 4.64e+03 | 0 | 0 |
| flood | 32 | full | 0.955 | 680 | 47.7 | 9.62 | 0 | 4.61e+03 | 1.6e+05 | 3.97e+04 |
| flood | 32 | static-bucket | 0.32 | 228 | 175 | 31.1 | 0 | 6.7e+03 | 6.88e+05 | 0 |
| flood | 32 | vanilla | 1 | 712 | 231 | 45.9 | 0 | 1.7e+04 | 0 | 0 |
| none | 0 | full | 1 | 792 | 6.09 | 1.25 | — | — | 0 | 0 |
| none | 0 | static-bucket | 1 | 792 | 6.09 | 1.25 | — | — | 0 | 0 |
| none | 0 | vanilla | 1 | 792 | 6.09 | 1.25 | — | — | 0 | 0 |
| replay | 2 | full | 0.997 | 710 | 7.26 | 1.38 | 1 | 174 | 0 | 0 |
| replay | 2 | static-bucket | 1 | 712 | 7.25 | 1.4 | 0 | 833 | 0 | 0 |
| replay | 2 | vanilla | 1 | 712 | 7.25 | 1.4 | 0 | 833 | 0 | 0 |
| replay | 8 | full | 0.997 | 710 | 10.1 | 1.38 | 1 | 172 | 0 | 0 |
| replay | 8 | static-bucket | 1 | 712 | 10 | 1.4 | 0 | 2.83e+03 | 0 | 0 |
| replay | 8 | vanilla | 1 | 712 | 10 | 1.4 | 0 | 2.83e+03 | 0 | 0 |
| replay | 32 | full | 0.997 | 710 | 21.3 | 1.38 | 1 | 172 | 0 | 0 |
| replay | 32 | static-bucket | 1 | 712 | 21.2 | 1.4 | 0 | 1.08e+04 | 0 | 0 |
| replay | 32 | vanilla | 1 | 712 | 21.2 | 1.4 | 0 | 1.08e+04 | 0 | 0 |

| criterion | holds |
|---|---|
| full: every replaying attacker banned at every honest neighbour (coverage 100 %) | yes |
| static-bucket and vanilla: no attacker banned in any run | yes |
| full: delivery >= 0.99 against replay at every intensity | NO |
| 0 honest false evictions in every modes run | yes |
