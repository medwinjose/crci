CRCI mitigation: flag zones where rescue requests contradict
the consensus (honest nodes filed rescues, median says clear).

╔══════════════════════════════════════════════╗
║  STRESS TEST 3 — Signature Forgery Attempt  ║
╚══════════════════════════════════════════════╝
Setup: attacker tries to impersonate node-001

Injecting forged message into node-002's inbox...
  ⚠ [node-002] PUBKEY MISMATCH on 'msg-forged' — impersonation dropped!

  ✅ Forged message REJECTED — node-002 recorded 0 observations
  ✅ Pubkey mismatch detected and dropped at receive

╔══════════════════════════════════════════════╗
║  STRESS TEST 4 — Network Partition           ║
╚══════════════════════════════════════════════╝
Partition active: left-001/002/003 isolated from right-001/002/003

  ✍  [left-001] originating 'msg-rescue-left' → sig: 6dc45f1f...
  [left-002][zone-a] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: trapped, need help
  [left-003][zone-a] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: trapped, need help
  ⛔ [left-002] REPLAY dropped from 'left-001' — seq 1 already seen (min 2)
Right partition received rescue during partition: false
  ✅ Partition correctly isolated — message did not cross

Healing partition: connecting left-003 ↔ right-001...
  ✍  [left-001] originating 'msg-rescue-left-2' → sig: 4ddd7f41...
  [left-002][zone-a] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: still trapped, partition healed
  [left-003][zone-a] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: still trapped, partition healed
  ⛔ [left-002] REPLAY dropped from 'left-001' — seq 2 already seen (min 3)
  [right-001][zone-a] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: still trapped, partition healed
  [right-002][zone-a] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: still trapped, partition healed
  ⛔ [left-003] REPLAY dropped from 'left-001' — seq 2 already seen (min 3)
  [right-003][zone-a] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: still trapped, partition healed
  ⛔ [right-001] REPLAY dropped from 'left-001' — seq 2 already seen (min 3)
  ⛔ [right-002] REPLAY dropped from 'left-001' — seq 2 already seen (min 3)
  ✅ After healing — rescue reached right-003

╔══════════════════════════════════════════════════════╗
║  SCENARIO 1 — Flash Flood, Tamil Nadu Coast          ║
║  T+0: Cyclone makes landfall. Storm surge incoming.   ║
╚══════════════════════════════════════════════════════╝

=== T+0: Cyclone makes landfall ===
Network: 8 nodes across coastal and inland zones

--- T+15 min: Storm surge reaches coastal zone ---
  ✍  [ravi] originating 'r1' → sig: 37a09e0e...
  ✍  [priya] originating 'r2' → sig: e01fa7ee...
  ✍  [selvam] originating 'r3' → sig: 11848f93...
  ✍  [meera] originating 'r4' → sig: 538eb8f3...
  ✍  [arjun] originating 'r5' → sig: 36161b5f...
  [kumar][coastal-south] normal | sev:2 conf:2 vis:unknown | note: indoors, can hear water, cannot see outside
  [kumar][coastal-south] normal | sev:1 conf:5 vis:direct | note: situation manageable, no evacuation needed
  [meera][coastal-south] normal | sev:1 conf:5 vis:direct | note: inland relief camp ready, have 3 trucks available
  [meera][coastal-south] normal | sev:2 conf:2 vis:unknown | note: indoors, can hear water, cannot see outside
  [arjun][inland-relief] normal | sev:1 conf:5 vis:direct | note: situation manageable, no evacuation needed
  [arjun][inland-relief] normal | sev:2 conf:2 vis:unknown | note: indoors, can hear water, cannot see outside
  [selvam][coastal-north] normal | sev:4 conf:5 vis:direct | note: water rising fast, knee deep on main street
  [ravi][coastal-north] normal | sev:2 conf:2 vis:unknown | note: indoors, can hear water, cannot see outside
  [ravi][coastal-north] normal | sev:5 conf:5 vis:direct | note: boat torn loose, water entering ground floor
  [lakshmi][inland-relief] normal | sev:1 conf:5 vis:direct | note: inland relief camp ready, have 3 trucks available
  [lakshmi][inland-relief] normal | sev:1 conf:5 vis:direct | note: situation manageable, no evacuation needed
  [lakshmi][inland-relief] normal | sev:2 conf:2 vis:unknown | note: indoors, can hear water, cannot see outside
  [priya][coastal-north] normal | sev:4 conf:5 vis:direct | note: water rising fast, knee deep on main street
  [priya][coastal-north] normal | sev:1 conf:5 vis:direct | note: situation manageable, no evacuation needed
  ⛔ [priya] REPLAY dropped from 'priya' — seq 1 already seen (min 2)
  [priya][coastal-north] normal | sev:5 conf:5 vis:direct | note: boat torn loose, water entering ground floor
  [kumar][coastal-south] normal | sev:1 conf:5 vis:direct | note: inland relief camp ready, have 3 trucks available
  ⛔ [kumar] REPLAY dropped from 'priya' — seq 1 already seen (min 2)
  [kumar][coastal-south] normal | sev:4 conf:5 vis:direct | note: water rising fast, knee deep on main street
  ⛔ [kumar] REPLAY dropped from 'meera' — seq 1 already seen (min 2)
  [kumar][coastal-south] normal | sev:5 conf:5 vis:direct | note: boat torn loose, water entering ground floor
  ⛔ [meera] REPLAY dropped from 'meera' — seq 1 already seen (min 2)
  ⛔ [meera] REPLAY dropped from 'priya' — seq 1 already seen (min 2)
  ⛔ [meera] REPLAY dropped from 'arjun' — seq 1 already seen (min 2)
  [meera][coastal-south] normal | sev:4 conf:5 vis:direct | note: water rising fast, knee deep on main street
  [meera][coastal-south] normal | sev:5 conf:5 vis:direct | note: boat torn loose, water entering ground floor
  ⛔ [arjun] REPLAY dropped from 'arjun' — seq 1 already seen (min 2)
  ⛔ [arjun] REPLAY dropped from 'meera' — seq 1 already seen (min 2)
  ⛔ [arjun] REPLAY dropped from 'priya' — seq 1 already seen (min 2)
  [arjun][inland-relief] normal | sev:4 conf:5 vis:direct | note: water rising fast, knee deep on main street
  [arjun][inland-relief] normal | sev:5 conf:5 vis:direct | note: boat torn loose, water entering ground floor
  [selvam][coastal-north] normal | sev:2 conf:2 vis:unknown | note: indoors, can hear water, cannot see outside
  [district-control][inland-relief] normal | sev:1 conf:5 vis:direct | note: inland relief camp ready, have 3 trucks available
  [district-control][inland-relief] normal | sev:1 conf:5 vis:direct | note: situation manageable, no evacuation needed
  [district-control][inland-relief] normal | sev:2 conf:2 vis:unknown | note: indoors, can hear water, cannot see outside
  ⛔ [ravi] REPLAY dropped from 'ravi' — seq 1 already seen (min 2)
  [ravi][coastal-north] normal | sev:1 conf:5 vis:direct | note: situation manageable, no evacuation needed
  ⛔ [ravi] REPLAY dropped from 'selvam' — seq 1 already seen (min 2)
  ⛔ [ravi] REPLAY dropped from 'priya' — seq 1 already seen (min 2)
  [lakshmi][inland-relief] normal | sev:4 conf:5 vis:direct | note: water rising fast, knee deep on main street
  [lakshmi][inland-relief] normal | sev:5 conf:5 vis:direct | note: boat torn loose, water entering ground floor
  ⛔ [lakshmi] REPLAY dropped from 'arjun' — seq 1 already seen (min 2)
  ⛔ [lakshmi] REPLAY dropped from 'meera' — seq 1 already seen (min 2)
  ⛔ [lakshmi] REPLAY dropped from 'priya' — seq 1 already seen (min 2)
  [priya][coastal-north] normal | sev:1 conf:5 vis:direct | note: inland relief camp ready, have 3 trucks available
  ⛔ [priya] REPLAY dropped from 'ravi' — seq 1 already seen (min 2)
  ⛔ [priya] REPLAY dropped from 'selvam' — seq 1 already seen (min 2)
  ⛔ [priya] REPLAY dropped from 'meera' — seq 1 already seen (min 2)
  ⛔ [kumar] REPLAY dropped from 'ravi' — seq 1 already seen (min 2)
  ⛔ [kumar] REPLAY dropped from 'selvam' — seq 1 already seen (min 2)
  ⛔ [kumar] REPLAY dropped from 'arjun' — seq 1 already seen (min 2)
  ⛔ [meera] REPLAY dropped from 'ravi' — seq 1 already seen (min 2)
  ⛔ [meera] REPLAY dropped from 'selvam' — seq 1 already seen (min 2)
  ⛔ [arjun] REPLAY dropped from 'ravi' — seq 1 already seen (min 2)
  ⛔ [arjun] REPLAY dropped from 'selvam' — seq 1 already seen (min 2)
  [selvam][coastal-north] normal | sev:1 conf:5 vis:direct | note: situation manageable, no evacuation needed
  [district-control][inland-relief] normal | sev:4 conf:5 vis:direct | note: water rising fast, knee deep on main street
  [district-control][inland-relief] normal | sev:5 conf:5 vis:direct | note: boat torn loose, water entering ground floor
  [ravi][coastal-north] normal | sev:1 conf:5 vis:direct | note: inland relief camp ready, have 3 trucks available
  ⛔ [ravi] REPLAY dropped from 'meera' — seq 1 already seen (min 2)
  ⛔ [lakshmi] REPLAY dropped from 'ravi' — seq 1 already seen (min 2)
  ⛔ [lakshmi] REPLAY dropped from 'selvam' — seq 1 already seen (min 2)
  ⛔ [priya] REPLAY dropped from 'arjun' — seq 1 already seen (min 2)
  [selvam][coastal-north] normal | sev:1 conf:5 vis:direct | note: inland relief camp ready, have 3 trucks available
  ⛔ [ravi] REPLAY dropped from 'arjun' — seq 1 already seen (min 2)

--- Consensus Round 1 ---
  [Zone coastal-south] 1 reporter(s)
  [Zone coastal-south] ⚠ Insufficient reporters — skipped
  [Zone coastal-north] 3 reporter(s)
  [Zone coastal-north] Median severity: 4
  [Zone coastal-north] ℹ [priya] sev 2 — unknown vis, not penalised
  [Zone coastal-north] ✓ No anomalies
  [Zone inland-relief] 1 reporter(s)
  [Zone inland-relief] ⚠ Insufficient reporters — skipped

--- T+45 min: Selvam's phone submerged by rising water ---
  ⚡ [selvam] went OFFLINE
  ✍  [ravi] originating 'rescue-ravi' → sig: e6db40be...
  ✍  [kumar] originating 'r6' → sig: e6755e3e...
  ✍  [meera] originating 'r7' → sig: e7d740cf...
  ✍  [arjun] originating 'r8' → sig: c1e2a2db...
  [kumar][coastal-south] normal | sev:1 conf:5 vis:direct | note: overreaction, drainage systems handling it
  [meera][coastal-south] normal | sev:5 conf:5 vis:direct | note: entire street underwater, 12 families need boats
  [meera][coastal-south] normal | sev:1 conf:5 vis:direct | note: dispatching trucks, need GPS coordinates
  [arjun][inland-relief] normal | sev:1 conf:5 vis:direct | note: overreaction, drainage systems handling it
  [arjun][inland-relief] normal | sev:5 conf:5 vis:direct | note: entire street underwater, 12 families need boats
  [lakshmi][inland-relief] normal | sev:1 conf:5 vis:direct | note: dispatching trucks, need GPS coordinates
  [lakshmi][inland-relief] normal | sev:1 conf:5 vis:direct | note: overreaction, drainage systems handling it
  [lakshmi][inland-relief] normal | sev:5 conf:5 vis:direct | note: entire street underwater, 12 families need boats
  [priya][coastal-north] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: trapped on roof, 4 family members, GPS: 11.3410,79.8012
  [priya][coastal-north] normal | sev:5 conf:5 vis:direct | note: entire street underwater, 12 families need boats
  [priya][coastal-north] normal | sev:1 conf:5 vis:direct | note: overreaction, drainage systems handling it
  [kumar][coastal-south] normal | sev:1 conf:5 vis:direct | note: dispatching trucks, need GPS coordinates
  [kumar][coastal-south] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: trapped on roof, 4 family members, GPS: 11.3410,79.8012
  ⛔ [kumar] REPLAY dropped from 'kumar' — seq 1 already seen (min 2)
  ⛔ [kumar] REPLAY dropped from 'meera' — seq 2 already seen (min 3)
  ⛔ [meera] REPLAY dropped from 'meera' — seq 2 already seen (min 3)
  ⛔ [meera] REPLAY dropped from 'kumar' — seq 1 already seen (min 2)
  ⛔ [meera] REPLAY dropped from 'arjun' — seq 2 already seen (min 3)
  [meera][coastal-south] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: trapped on roof, 4 family members, GPS: 11.3410,79.8012
  ⛔ [arjun] REPLAY dropped from 'arjun' — seq 2 already seen (min 3)
  ⛔ [arjun] REPLAY dropped from 'meera' — seq 2 already seen (min 3)
  ⛔ [arjun] REPLAY dropped from 'kumar' — seq 1 already seen (min 2)
  [arjun][inland-relief] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: trapped on roof, 4 family members, GPS: 11.3410,79.8012
  [district-control][inland-relief] normal | sev:1 conf:5 vis:direct | note: dispatching trucks, need GPS coordinates
  [district-control][inland-relief] normal | sev:1 conf:5 vis:direct | note: overreaction, drainage systems handling it
  [district-control][inland-relief] normal | sev:5 conf:5 vis:direct | note: entire street underwater, 12 families need boats
  [ravi][coastal-north] normal | sev:5 conf:5 vis:direct | note: entire street underwater, 12 families need boats
  [ravi][coastal-north] normal | sev:1 conf:5 vis:direct | note: overreaction, drainage systems handling it
  [lakshmi][inland-relief] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: trapped on roof, 4 family members, GPS: 11.3410,79.8012
  ⛔ [lakshmi] REPLAY dropped from 'arjun' — seq 2 already seen (min 3)
  ⛔ [lakshmi] REPLAY dropped from 'meera' — seq 2 already seen (min 3)
  ⛔ [lakshmi] REPLAY dropped from 'kumar' — seq 1 already seen (min 2)
  [priya][coastal-north] normal | sev:1 conf:5 vis:direct | note: dispatching trucks, need GPS coordinates
  ⛔ [priya] REPLAY dropped from 'ravi' — seq 2 already seen (min 3)
  ⛔ [priya] REPLAY dropped from 'kumar' — seq 1 already seen (min 2)
  ⛔ [priya] REPLAY dropped from 'meera' — seq 2 already seen (min 3)
  ⛔ [kumar] REPLAY dropped from 'ravi' — seq 2 already seen (min 3)
  ⛔ [kumar] REPLAY dropped from 'arjun' — seq 2 already seen (min 3)
  ⛔ [meera] REPLAY dropped from 'ravi' — seq 2 already seen (min 3)
  ⛔ [arjun] REPLAY dropped from 'ravi' — seq 2 already seen (min 3)
  [district-control][inland-relief] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: trapped on roof, 4 family members, GPS: 11.3410,79.8012    
  [ravi][coastal-north] normal | sev:1 conf:5 vis:direct | note: dispatching trucks, need GPS coordinates
  ⛔ [lakshmi] REPLAY dropped from 'ravi' — seq 2 already seen (min 3)
  ⛔ [priya] REPLAY dropped from 'arjun' — seq 2 already seen (min 3)

--- Consensus Round 2 (meera flagged, selvam rescue kept alive) ---
  [Zone coastal-north] 1 reporter(s)
  [Zone coastal-north] ⚠ Insufficient reporters — skipped
  [Zone coastal-south] 2 reporter(s)
  [Zone coastal-south] ⚠ Insufficient reporters — skipped
  [Zone inland-relief] 1 reporter(s)
  [Zone inland-relief] ⚠ Insufficient reporters — skipped

=== Flood Scenario — Final Network State ===
ID: arjun      | Zone: inland-relief | Rep: 1.00 | TRUSTED | ONLINE | Rescue: 1 | PubKey: 5555d20a3e22...
ID: district-control | Zone: inland-relief | Rep: 1.00 | TRUSTED | ONLINE | Rescue: 1 | PubKey: b8bddda9ac4b...
ID: kumar      | Zone: coastal-south | Rep: 1.00 | TRUSTED | ONLINE | Rescue: 1 | PubKey: 0d6b00580c82...
ID: lakshmi    | Zone: inland-relief | Rep: 1.00 | TRUSTED | ONLINE | Rescue: 1 | PubKey: 18004d082dd6...
ID: meera      | Zone: coastal-south | Rep: 1.00 | TRUSTED | ONLINE | Rescue: 1 | PubKey: f8a9f64764bf...
ID: priya      | Zone: coastal-north | Rep: 1.00 | TRUSTED | ONLINE | Rescue: 1 | PubKey: c78997f996a6...
ID: ravi       | Zone: coastal-north | Rep: 1.00 | TRUSTED | ONLINE | Rescue: 1 | PubKey: 38e987d8eb7a...
ID: selvam     | Zone: coastal-north | Rep: 1.00 | TRUSTED | OFFLINE | Rescue: 0 | PubKey: f179cb0d4a3b...

=== Resource Matching Report ===
Nodes needing help: 1
  🆘 [ravi]: trapped on roof, 4 family members, GPS: 11.3410,79.8012
Nodes that can help: 2
  ✅ [arjun]: inland relief camp ready, have 3 trucks available
  ✅ [meera]: overreaction, drainage systems handling it

╔══════════════════════════════════════════════════════╗
║  SCENARIO 2 — Earthquake M7.8, Türkiye, 04:17 AM     ║
║  Most people asleep. Infrastructure destroyed.        ║
╚══════════════════════════════════════════════════════╝

=== T+0: M7.8 earthquake strikes ===
10 residents in apartment block + 2 SAR teams

  ✍  [apt-01] originating 'panic-01' → sig: ce435e79...
  ✍  [apt-02] originating 'panic-02' → sig: f8d4fb27...
  ✍  [apt-03] originating 'panic-03' → sig: 638311a2...
  ✍  [apt-04] originating 'panic-04' → sig: 781cd6cc...
  ✍  [apt-05] originating 'panic-05' → sig: 73d4ac1f...
  ✍  [apt-06] originating 'panic-06' → sig: 1eafb7ce...
  ✍  [apt-07] originating 'panic-07' → sig: 9d1129a8...
  ⚡ apt-08, apt-09, apt-10 phones destroyed by collapse
  ⚡ [apt-08] went OFFLINE
  ⚡ [apt-09] went OFFLINE
  ⚡ [apt-10] went OFFLINE
  ✍  [sar-001] originating 'sar-obs-1' → sig: 50a8359b...
  [sar-002][rescue-staging] normal | sev:5 conf:5 vis:direct | note: multiple structural collapses visible, dispatching teams
  [apt-03][block-a] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: pinned under debris, leg injury
  [apt-03][block-a] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: —
  [apt-07][block-a] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: —
  [apt-02][block-a] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: floor collapsed, 3 people trapped
  [apt-02][block-a] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: gas smell, fire risk
  [apt-02][block-a] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: —
  [apt-01][block-a] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: pinned under debris, leg injury
  [apt-01][block-a] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: gas smell, fire risk
  [apt-01][block-a] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: —
  [apt-04][block-a] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: gas smell, fire risk
  [apt-04][block-a] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: family of 5, two children injured
  [apt-04][block-a] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: pinned under debris, leg injury
  [apt-06][block-a] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: family of 5, two children injured
  [apt-06][block-a] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: —
  [apt-05][block-a] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: —
  [apt-05][block-a] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: —
  [apt-05][block-a] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: gas smell, fire risk
  [apt-05][block-a] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: pinned under debris, leg injury
  ⛔ [apt-05] REPLAY dropped from 'apt-05' — seq 1 already seen (min 2)
  [apt-05][block-a] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: —
  [apt-03][block-a] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: floor collapsed, 3 people trapped
  ⛔ [apt-03] REPLAY dropped from 'apt-04' — seq 1 already seen (min 2)
  ⛔ [apt-03] REPLAY dropped from 'apt-03' — seq 1 already seen (min 2)
  [apt-03][block-a] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: family of 5, two children injured
  ⛔ [apt-03] REPLAY dropped from 'apt-02' — seq 1 already seen (min 2)
  [apt-07][block-a] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: family of 5, two children injured
  ⛔ [apt-02] REPLAY dropped from 'apt-02' — seq 1 already seen (min 2)
  ⛔ [apt-02] REPLAY dropped from 'apt-03' — seq 1 already seen (min 2)
  ⛔ [apt-02] REPLAY dropped from 'apt-04' — seq 1 already seen (min 2)
  ⛔ [apt-02] REPLAY dropped from 'apt-01' — seq 1 already seen (min 2)
  [apt-02][block-a] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: family of 5, two children injured
  [apt-01][block-a] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: family of 5, two children injured
  ⛔ [apt-04] REPLAY dropped from 'apt-04' — seq 1 already seen (min 2)
  [apt-04][block-a] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: —
  ⛔ [apt-04] REPLAY dropped from 'apt-03' — seq 1 already seen (min 2)
  ⛔ [apt-04] REPLAY dropped from 'apt-02' — seq 1 already seen (min 2)
  [apt-04][block-a] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: —
  [apt-04][block-a] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: floor collapsed, 3 people trapped
  ⛔ [apt-04] REPLAY dropped from 'apt-05' — seq 1 already seen (min 2)
  [apt-06][block-a] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: —
  ⛔ [apt-06] REPLAY dropped from 'apt-06' — seq 1 already seen (min 2)
  [apt-06][block-a] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: gas smell, fire risk
  [apt-06][block-a] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: pinned under debris, leg injury
  ⛔ [apt-06] REPLAY dropped from 'apt-07' — seq 1 already seen (min 2)
  ⛔ [apt-06] REPLAY dropped from 'apt-05' — seq 1 already seen (min 2)
  ⛔ [apt-05] REPLAY dropped from 'apt-06' — seq 1 already seen (min 2)
  ⛔ [apt-05] REPLAY dropped from 'apt-07' — seq 1 already seen (min 2)
  [apt-05][block-a] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: floor collapsed, 3 people trapped
  ⛔ [apt-05] REPLAY dropped from 'apt-04' — seq 1 already seen (min 2)
  ⛔ [apt-05] REPLAY dropped from 'apt-03' — seq 1 already seen (min 2)
  ⛔ [apt-05] REPLAY dropped from 'apt-02' — seq 1 already seen (min 2)
  ⛔ [apt-03] REPLAY dropped from 'apt-05' — seq 1 already seen (min 2)
  [apt-03][block-a] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: —
  [apt-03][block-a] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: —
  ⛔ [apt-03] REPLAY dropped from 'apt-01' — seq 1 already seen (min 2)
  [apt-07][block-a] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: —
  [apt-07][block-a] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: gas smell, fire risk
  [apt-07][block-a] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: pinned under debris, leg injury
  ⛔ [apt-02] REPLAY dropped from 'apt-05' — seq 1 already seen (min 2)
  [apt-02][block-a] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: —
  [apt-02][block-a] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: —
  [apt-01][block-a] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: —
  [apt-01][block-a] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: —
  ⛔ [apt-04] REPLAY dropped from 'apt-01' — seq 1 already seen (min 2)
  ⛔ [apt-04] REPLAY dropped from 'apt-06' — seq 1 already seen (min 2)
  ⛔ [apt-04] REPLAY dropped from 'apt-07' — seq 1 already seen (min 2)
  [apt-06][block-a] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: floor collapsed, 3 people trapped
  ⛔ [apt-06] REPLAY dropped from 'apt-04' — seq 1 already seen (min 2)
  ⛔ [apt-06] REPLAY dropped from 'apt-03' — seq 1 already seen (min 2)
  ⛔ [apt-06] REPLAY dropped from 'apt-02' — seq 1 already seen (min 2)
  ⛔ [apt-05] REPLAY dropped from 'apt-01' — seq 1 already seen (min 2)
  ⛔ [apt-03] REPLAY dropped from 'apt-06' — seq 1 already seen (min 2)
  ⛔ [apt-03] REPLAY dropped from 'apt-07' — seq 1 already seen (min 2)
  [apt-07][block-a] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: floor collapsed, 3 people trapped
  ⛔ [apt-02] REPLAY dropped from 'apt-06' — seq 1 already seen (min 2)
  ⛔ [apt-02] REPLAY dropped from 'apt-07' — seq 1 already seen (min 2)
  ⛔ [apt-06] REPLAY dropped from 'apt-01' — seq 1 already seen (min 2)

--- Consensus T+0 (MCE should declare) ---
  [Zone rescue-staging] 1 reporter(s)
  [Zone rescue-staging] ⚠ Insufficient reporters — skipped
  [Zone block-a] 7 reporter(s)
  [Zone block-a] Median severity: 5
  [Zone block-a] ✓ No anomalies
  [Zone block-a] 🚨 MASS CASUALTY EVENT — 7 rescue requests

--- T+22 min: Aftershock M5.2 — secondary collapses ---
  ⚡ [apt-04] went OFFLINE
  ⚡ [apt-06] went OFFLINE
  ✍  [apt-01] originating 'panic-01-update' → sig: 06ce1928...
  ✍  [sar-002] originating 'sar-obs-2' → sig: 048d2421...
  [sar-001][rescue-staging] normal | sev:5 conf:5 vis:direct | note: aftershock caused 2 additional collapses, need more teams
  [apt-02][block-a] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: aftershock worsened collapse, now 5 people confirmed trapped
  [apt-03][block-a] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: aftershock worsened collapse, now 5 people confirmed trapped
  ⛔ [apt-02] REPLAY dropped from 'apt-01' — seq 2 already seen (min 3)

--- Consensus T+22 (rescue msgs from offline nodes kept alive) ---
  [Zone rescue-staging] 1 reporter(s)
  [Zone rescue-staging] ⚠ Insufficient reporters — skipped
  [Zone block-a] 1 reporter(s)
  [Zone block-a] ⚠ Insufficient reporters — skipped

=== Earthquake Scenario — Rescue Request Survival Check ===
  [apt-10] online:false rescue_msgs_held:0
  [apt-03] online:true rescue_msgs_held:8
  [apt-07] online:true rescue_msgs_held:7
  [apt-02] online:true rescue_msgs_held:8
  [apt-01] online:true rescue_msgs_held:8
  [apt-04] online:false rescue_msgs_held:7
  [apt-06] online:false rescue_msgs_held:7
  [apt-05] online:true rescue_msgs_held:7
  [apt-08] online:false rescue_msgs_held:0
  [apt-09] online:false rescue_msgs_held:0

╔══════════════════════════════════════════════════════╗
║  SCENARIO 3 — Urban Conflict, Active Combat Zone      ║
║  3 enemy-controlled nodes seeding false intelligence  ║
╚══════════════════════════════════════════════════════╝

=== Active conflict — 3 enemy nodes seeding false intelligence ===

  ✍  [civ-001] originating 'c1' → sig: 5cec9433...
  ✍  [civ-002] originating 'c2' → sig: 8a3023bc...
  ✍  [civ-003] originating 'c3' → sig: dc7dd4c8...
  ✍  [civ-004] originating 'c4' → sig: e090ab3f...
  ✍  [enemy-a] originating 'e1' → sig: ae5fd55d...
  ✍  [enemy-b] originating 'e2' → sig: 2555e54a...
  ✍  [enemy-c] originating 'e3' → sig: 86bb3193...
  ✍  [un-001] originating 'un1' → sig: cda6cec7...
  [enemy-a][district-7] normal | sev:5 conf:5 vis:direct | note: sniper fire, cannot leave building
  [enemy-a][district-7] normal | sev:4 conf:4 vis:direct | note: explosions heard, indirect fire in area
  [civ-002][district-7] normal | sev:5 conf:5 vis:direct | note: active shelling, building on fire, need evacuation
  [civ-002][district-7] normal | sev:1 conf:5 vis:direct | note: area secure, no threat observed
  [civ-002][district-7] normal | sev:4 conf:4 vis:direct | note: explosions heard, indirect fire in area
  [enemy-b][district-7] normal | sev:4 conf:4 vis:direct | note: explosions heard, indirect fire in area
  [enemy-b][district-7] normal | sev:5 conf:5 vis:direct | note: casualties on street, medical needed urgently
  [civ-004][district-7] normal | sev:1 conf:5 vis:direct | note: area secure, no threat observed
  [civ-004][district-7] normal | sev:1 conf:5 vis:direct | note: area secure, no threat observed
  [civ-004][district-7] normal | sev:4 conf:4 vis:direct | note: explosions heard, indirect fire in area
  [un-001][district-7] normal | sev:1 conf:5 vis:direct | note: area secure, no threat observed
  [civ-001][district-7] normal | sev:5 conf:5 vis:direct | note: sniper fire, cannot leave building
  [civ-001][district-7] normal | sev:1 conf:5 vis:direct | note: area secure, no threat observed
  [civ-001][district-7] normal | sev:4 conf:4 vis:direct | note: explosions heard, indirect fire in area
  [civ-003][district-7] normal | sev:1 conf:5 vis:direct | note: area secure, no threat observed
  [civ-003][district-7] normal | sev:1 conf:5 vis:direct | note: area secure, no threat observed
  [civ-003][district-7] normal | sev:5 conf:5 vis:direct | note: sniper fire, cannot leave building
  ⛔ [civ-003] REPLAY dropped from 'civ-003' — seq 1 already seen (min 2)
  [civ-003][district-7] normal | sev:5 conf:5 vis:direct | note: casualties on street, medical needed urgently
  [enemy-c][district-7] normal | sev:5 conf:5 vis:direct | note: casualties on street, medical needed urgently
  [enemy-c][district-7] normal | sev:4 conf:4 vis:direct | note: UN observer: confirmed civilian distress, requesting corridor
  [enemy-c][district-7] normal | sev:1 conf:5 vis:direct | note: area secure, no threat observed
  [enemy-c][district-7] normal | sev:4 conf:4 vis:direct | note: explosions heard, indirect fire in area
  ⛔ [enemy-c] REPLAY dropped from 'enemy-c' — seq 1 already seen (min 2)
  [enemy-a][district-7] normal | sev:5 conf:5 vis:direct | note: active shelling, building on fire, need evacuation
  ⛔ [enemy-a] REPLAY dropped from 'civ-003' — seq 1 already seen (min 2)
  ⛔ [enemy-a] REPLAY dropped from 'enemy-a' — seq 1 already seen (min 2)
  [enemy-a][district-7] normal | sev:1 conf:5 vis:direct | note: area secure, no threat observed
  ⛔ [enemy-a] REPLAY dropped from 'civ-002' — seq 1 already seen (min 2)
  [enemy-a][district-7] normal | sev:5 conf:5 vis:direct | note: casualties on street, medical needed urgently
  ⛔ [civ-002] REPLAY dropped from 'civ-002' — seq 1 already seen (min 2)
  ⛔ [civ-002] REPLAY dropped from 'enemy-a' — seq 1 already seen (min 2)
  ⛔ [civ-002] REPLAY dropped from 'civ-003' — seq 1 already seen (min 2)
  ⛔ [civ-002] REPLAY dropped from 'civ-001' — seq 1 already seen (min 2)
  [civ-002][district-7] normal | sev:1 conf:5 vis:direct | note: area secure, no threat observed
  [civ-002][district-7] normal | sev:5 conf:5 vis:direct | note: casualties on street, medical needed urgently
  [enemy-b][district-7] normal | sev:1 conf:5 vis:direct | note: area secure, no threat observed
  ⛔ [enemy-b] REPLAY dropped from 'civ-003' — seq 1 already seen (min 2)
  [enemy-b][district-7] normal | sev:1 conf:5 vis:direct | note: area secure, no threat observed
  ⛔ [enemy-b] REPLAY dropped from 'enemy-b' — seq 1 already seen (min 2)
  [enemy-b][district-7] normal | sev:5 conf:5 vis:direct | note: sniper fire, cannot leave building
  ⛔ [enemy-b] REPLAY dropped from 'civ-004' — seq 1 already seen (min 2)
  ⛔ [civ-004] REPLAY dropped from 'civ-004' — seq 1 already seen (min 2)
  [civ-004][district-7] normal | sev:4 conf:4 vis:direct | note: UN observer: confirmed civilian distress, requesting corridor
  ⛔ [civ-004] REPLAY dropped from 'enemy-b' — seq 1 already seen (min 2)
  ⛔ [civ-004] REPLAY dropped from 'civ-003' — seq 1 already seen (min 2)
  ⛔ [civ-004] REPLAY dropped from 'enemy-c' — seq 1 already seen (min 2)
  [civ-004][district-7] normal | sev:1 conf:5 vis:direct | note: area secure, no threat observed
  [civ-004][district-7] normal | sev:5 conf:5 vis:direct | note: sniper fire, cannot leave building
  [un-001][district-7] normal | sev:5 conf:5 vis:direct | note: casualties on street, medical needed urgently
  [un-001][district-7] normal | sev:1 conf:5 vis:direct | note: area secure, no threat observed
  [un-001][district-7] normal | sev:4 conf:4 vis:direct | note: explosions heard, indirect fire in area
  [civ-001][district-7] normal | sev:1 conf:5 vis:direct | note: area secure, no threat observed
  [civ-001][district-7] normal | sev:5 conf:5 vis:direct | note: casualties on street, medical needed urgently
  [civ-003][district-7] normal | sev:5 conf:5 vis:direct | note: active shelling, building on fire, need evacuation
  ⛔ [civ-003] REPLAY dropped from 'enemy-b' — seq 1 already seen (min 2)
  ⛔ [civ-003] REPLAY dropped from 'civ-004' — seq 1 already seen (min 2)
  [civ-003][district-7] normal | sev:1 conf:5 vis:direct | note: area secure, no threat observed
  ⛔ [civ-003] REPLAY dropped from 'enemy-a' — seq 1 already seen (min 2)
  ⛔ [civ-003] REPLAY dropped from 'civ-002' — seq 1 already seen (min 2)
  ⛔ [enemy-c] REPLAY dropped from 'un-001' — seq 1 already seen (min 2)
  [enemy-c][district-7] normal | sev:1 conf:5 vis:direct | note: area secure, no threat observed
  [enemy-c][district-7] normal | sev:5 conf:5 vis:direct | note: sniper fire, cannot leave building
  ⛔ [enemy-c] REPLAY dropped from 'civ-004' — seq 1 already seen (min 2)
  ⛔ [enemy-c] REPLAY dropped from 'enemy-b' — seq 1 already seen (min 2)
  ⛔ [enemy-c] REPLAY dropped from 'civ-003' — seq 1 already seen (min 2)
  ⛔ [enemy-a] REPLAY dropped from 'enemy-b' — seq 1 already seen (min 2)
  ⛔ [enemy-a] REPLAY dropped from 'civ-004' — seq 1 already seen (min 2)
  ⛔ [enemy-a] REPLAY dropped from 'civ-001' — seq 1 already seen (min 2)
  [enemy-a][district-7] normal | sev:1 conf:5 vis:direct | note: area secure, no threat observed
  ⛔ [civ-002] REPLAY dropped from 'enemy-b' — seq 1 already seen (min 2)
  ⛔ [civ-002] REPLAY dropped from 'civ-004' — seq 1 already seen (min 2)
  [civ-002][district-7] normal | sev:1 conf:5 vis:direct | note: area secure, no threat observed
  [enemy-b][district-7] normal | sev:4 conf:4 vis:direct | note: UN observer: confirmed civilian distress, requesting corridor
  ⛔ [enemy-b] REPLAY dropped from 'enemy-a' — seq 1 already seen (min 2)
  ⛔ [enemy-b] REPLAY dropped from 'civ-002' — seq 1 already seen (min 2)
  [enemy-b][district-7] normal | sev:5 conf:5 vis:direct | note: active shelling, building on fire, need evacuation
  ⛔ [enemy-b] REPLAY dropped from 'enemy-c' — seq 1 already seen (min 2)
  ⛔ [civ-004] REPLAY dropped from 'enemy-a' — seq 1 already seen (min 2)
  ⛔ [civ-004] REPLAY dropped from 'civ-002' — seq 1 already seen (min 2)
  ⛔ [civ-004] REPLAY dropped from 'un-001' — seq 1 already seen (min 2)
  [civ-004][district-7] normal | sev:5 conf:5 vis:direct | note: active shelling, building on fire, need evacuation
  [un-001][district-7] normal | sev:1 conf:5 vis:direct | note: area secure, no threat observed
  [un-001][district-7] normal | sev:5 conf:5 vis:direct | note: sniper fire, cannot leave building
  [civ-001][district-7] normal | sev:1 conf:5 vis:direct | note: area secure, no threat observed
  ⛔ [civ-003] REPLAY dropped from 'enemy-c' — seq 1 already seen (min 2)
  [civ-003][district-7] normal | sev:4 conf:4 vis:direct | note: UN observer: confirmed civilian distress, requesting corridor
  ⛔ [civ-003] REPLAY dropped from 'civ-001' — seq 1 already seen (min 2)
  [enemy-c][district-7] normal | sev:5 conf:5 vis:direct | note: active shelling, building on fire, need evacuation
  ⛔ [enemy-c] REPLAY dropped from 'enemy-a' — seq 1 already seen (min 2)
  ⛔ [enemy-c] REPLAY dropped from 'civ-002' — seq 1 already seen (min 2)
  ⛔ [enemy-a] REPLAY dropped from 'enemy-c' — seq 1 already seen (min 2)
  [enemy-a][district-7] normal | sev:4 conf:4 vis:direct | note: UN observer: confirmed civilian distress, requesting corridor
  ⛔ [civ-002] REPLAY dropped from 'enemy-c' — seq 1 already seen (min 2)
  [civ-002][district-7] normal | sev:4 conf:4 vis:direct | note: UN observer: confirmed civilian distress, requesting corridor
  ⛔ [enemy-b] REPLAY dropped from 'civ-001' — seq 1 already seen (min 2)
  ⛔ [enemy-b] REPLAY dropped from 'un-001' — seq 1 already seen (min 2)
  ⛔ [civ-004] REPLAY dropped from 'civ-001' — seq 1 already seen (min 2)
  [un-001][district-7] normal | sev:5 conf:5 vis:direct | note: active shelling, building on fire, need evacuation
  [civ-001][district-7] normal | sev:4 conf:4 vis:direct | note: UN observer: confirmed civilian distress, requesting corridor
  ⛔ [civ-003] REPLAY dropped from 'un-001' — seq 1 already seen (min 2)
  ⛔ [enemy-c] REPLAY dropped from 'civ-001' — seq 1 already seen (min 2)
  ⛔ [enemy-a] REPLAY dropped from 'un-001' — seq 1 already seen (min 2)
  ⛔ [civ-002] REPLAY dropped from 'un-001' — seq 1 already seen (min 2)
--- Consensus Round 1 (enemy nodes should be flagged) ---
  [Zone district-7] 8 reporter(s)
  [Zone district-7] Median severity: 4
  [Zone district-7] ⚠ ANOMALY: [enemy-a] sev 1 (dev 3 from 4)
  [Zone district-7] ✗ [enemy-a] penalised → rep 0.80
  [Zone district-7] ⚠ ANOMALY: [enemy-c] sev 1 (dev 3 from 4)
  [Zone district-7] ✗ [enemy-c] penalised → rep 0.80
  [Zone district-7] ⚠ ANOMALY: [enemy-b] sev 1 (dev 3 from 4)
  [Zone district-7] ✗ [enemy-b] penalised → rep 0.80
ID: civ-001    | Zone: district-7 | Rep: 1.00 | TRUSTED | ONLINE | Rescue: 0 | PubKey: f201b0dbc331...
ID: civ-002    | Zone: district-7 | Rep: 1.00 | TRUSTED | ONLINE | Rescue: 0 | PubKey: 55c14a339fe6...
ID: civ-003    | Zone: district-7 | Rep: 1.00 | TRUSTED | ONLINE | Rescue: 0 | PubKey: c350e1052324...
ID: civ-004    | Zone: district-7 | Rep: 1.00 | TRUSTED | ONLINE | Rescue: 0 | PubKey: 832e0e333e5c...
ID: enemy-a    | Zone: district-7 | Rep: 0.80 | TRUSTED | ONLINE | Rescue: 0 | PubKey: a4af515fbd1a...
ID: enemy-b    | Zone: district-7 | Rep: 0.80 | TRUSTED | ONLINE | Rescue: 0 | PubKey: 21bff0e1c02c...
ID: enemy-c    | Zone: district-7 | Rep: 0.80 | TRUSTED | ONLINE | Rescue: 0 | PubKey: e7272af57b71...
ID: un-001     | Zone: district-7 | Rep: 1.00 | TRUSTED | ONLINE | Rescue: 0 | PubKey: daa0f73073b8...

╔══════════════════════════════════════════════════════╗
║  SCENARIO 4 — Chemical Plant Explosion, Evacuation   ║
║  Wind: NE. Contamination plume moves SW.              ║
╚══════════════════════════════════════════════════════╝

=== Chemical plant explosion — multi-zone crisis ===
plant-zone: contaminated. evac-zone: at risk. clear-zone: safe.

  ✍  [worker-1] originating 'w-panic' → sig: 3a1c1bf3...
  ✍  [worker-2] originating 'w2' → sig: 9577368e...
  ✍  [security-1] originating 's1' → sig: b2e176eb...
  ✍  [resident-a] originating 'ra' → sig: ab8b488f...
  ✍  [resident-b] originating 'rb' → sig: 715f3781...
  ✍  [resident-c] originating 'rc' → sig: 37ab39b1...
  ✍  [safe-x] originating 'sx' → sig: 8dc8c1d3...
  ✍  [safe-y] originating 'sy' → sig: 3ed69166...
  ✍  [safe-z] originating 'sz' → sig: a877a950...
  ✍  [hazmat-1] originating 'h1' → sig: 42000921...
  [safe-y][clear-zone] normal | sev:1 conf:5 vis:direct | note: area clear of contamination, can shelter evacuees
  [safe-y][clear-zone] normal | sev:1 conf:5 vis:direct | note: area clear of contamination, can shelter evacuees
  [resident-a][evac-zone] normal | sev:5 conf:5 vis:direct | note: plant sealed, shelter in place for all remaining staff
  [resident-a][evac-zone] normal | sev:4 conf:4 vis:direct | note: family evacuating on foot, need transport
  [worker-2][plant-zone] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: explosion, chemical burn, cannot evacuate, need hazmat rescue
  [worker-2][plant-zone] normal | sev:5 conf:5 vis:direct | note: plant sealed, shelter in place for all remaining staff
  [resident-c][evac-zone] normal | sev:4 conf:4 vis:direct | note: family evacuating on foot, need transport
  [resident-c][evac-zone] normal | sev:1 conf:5 vis:direct | note: hazmat team staged at perimeter, awaiting entry clearance
  [security-1][plant-zone] normal | sev:5 conf:5 vis:direct | note: acid cloud visible, multiple workers down
  [security-1][plant-zone] normal | sev:4 conf:4 vis:direct | note: smell chemical, eyes burning, evacuating
  [security-1][plant-zone] normal | sev:4 conf:4 vis:direct | note: family evacuating on foot, need transport
  [security-1][plant-zone] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: explosion, chemical burn, cannot evacuate, need hazmat rescue       
  ⛔ [security-1] REPLAY dropped from 'security-1' — seq 1 already seen (min 2)
  [hazmat-2][perimeter] normal | sev:1 conf:5 vis:direct | note: hazmat team staged at perimeter, awaiting entry clearance
  [worker-1][plant-zone] normal | sev:5 conf:5 vis:direct | note: acid cloud visible, multiple workers down
  [worker-1][plant-zone] normal | sev:5 conf:5 vis:direct | note: plant sealed, shelter in place for all remaining staff
  [hazmat-1][perimeter] normal | sev:4 conf:4 vis:direct | note: elderly parent cannot walk, need medical transport
  [hazmat-1][perimeter] normal | sev:4 conf:4 vis:direct | note: family evacuating on foot, need transport
  ⛔ [hazmat-1] REPLAY dropped from 'hazmat-1' — seq 1 already seen (min 2)
  [safe-x][clear-zone] normal | sev:1 conf:5 vis:direct | note: area clear of contamination, can shelter evacuees
  [safe-x][clear-zone] normal | sev:1 conf:5 vis:direct | note: area clear of contamination, can shelter evacuees
  [resident-b][evac-zone] normal | sev:4 conf:4 vis:direct | note: smell chemical, eyes burning, evacuating
  [resident-b][evac-zone] normal | sev:4 conf:4 vis:direct | note: elderly parent cannot walk, need medical transport
  [resident-b][evac-zone] normal | sev:5 conf:5 vis:direct | note: plant sealed, shelter in place for all remaining staff
  ⛔ [resident-b] REPLAY dropped from 'resident-b' — seq 1 already seen (min 2)
  [resident-b][evac-zone] normal | sev:1 conf:5 vis:direct | note: hazmat team staged at perimeter, awaiting entry clearance
  [safe-z][clear-zone] normal | sev:1 conf:5 vis:direct | note: area clear of contamination, can shelter evacuees
  [safe-z][clear-zone] normal | sev:1 conf:5 vis:direct | note: area clear of contamination, can shelter evacuees
  ⛔ [safe-y] REPLAY dropped from 'safe-z' — seq 1 already seen (min 2)
  ⛔ [safe-y] REPLAY dropped from 'safe-y' — seq 1 already seen (min 2)
  ⛔ [safe-y] REPLAY dropped from 'safe-x' — seq 1 already seen (min 2)
  [resident-a][evac-zone] normal | sev:5 conf:5 vis:direct | note: acid cloud visible, multiple workers down
  ⛔ [resident-a] REPLAY dropped from 'resident-b' — seq 1 already seen (min 2)
  [resident-a][evac-zone] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: explosion, chemical burn, cannot evacuate, need hazmat rescue
  ⛔ [resident-a] REPLAY dropped from 'resident-a' — seq 1 already seen (min 2)
  [resident-a][evac-zone] normal | sev:4 conf:4 vis:direct | note: elderly parent cannot walk, need medical transport
  ⛔ [resident-a] REPLAY dropped from 'security-1' — seq 1 already seen (min 2)
  [resident-a][evac-zone] normal | sev:1 conf:5 vis:direct | note: hazmat team staged at perimeter, awaiting entry clearance
  [worker-2][plant-zone] normal | sev:4 conf:4 vis:direct | note: smell chemical, eyes burning, evacuating
  [worker-2][plant-zone] normal | sev:4 conf:4 vis:direct | note: family evacuating on foot, need transport
  ⛔ [worker-2] REPLAY dropped from 'worker-1' — seq 1 already seen (min 2)
  ⛔ [worker-2] REPLAY dropped from 'worker-2' — seq 1 already seen (min 2)
  ⛔ [worker-2] REPLAY dropped from 'security-1' — seq 1 already seen (min 2)
  ⛔ [resident-c] REPLAY dropped from 'resident-b' — seq 1 already seen (min 2)
  [resident-c][evac-zone] normal | sev:4 conf:4 vis:direct | note: smell chemical, eyes burning, evacuating
  ⛔ [resident-c] REPLAY dropped from 'resident-c' — seq 1 already seen (min 2)
  [resident-c][evac-zone] normal | sev:5 conf:5 vis:direct | note: plant sealed, shelter in place for all remaining staff
  ⛔ [resident-c] REPLAY dropped from 'hazmat-1' — seq 1 already seen (min 2)
  ⛔ [security-1] REPLAY dropped from 'worker-2' — seq 1 already seen (min 2)
  ⛔ [security-1] REPLAY dropped from 'worker-1' — seq 1 already seen (min 2)
  [security-1][plant-zone] normal | sev:4 conf:4 vis:direct | note: elderly parent cannot walk, need medical transport
  [security-1][plant-zone] normal | sev:1 conf:5 vis:direct | note: hazmat team staged at perimeter, awaiting entry clearance
  ⛔ [security-1] REPLAY dropped from 'resident-a' — seq 1 already seen (min 2)
  ⛔ [security-1] REPLAY dropped from 'resident-b' — seq 1 already seen (min 2)
  [hazmat-2][perimeter] normal | sev:4 conf:4 vis:direct | note: elderly parent cannot walk, need medical transport
  [hazmat-2][perimeter] normal | sev:4 conf:4 vis:direct | note: family evacuating on foot, need transport
  [worker-1][plant-zone] normal | sev:4 conf:4 vis:direct | note: smell chemical, eyes burning, evacuating
  [worker-1][plant-zone] normal | sev:4 conf:4 vis:direct | note: family evacuating on foot, need transport
  [hazmat-1][perimeter] normal | sev:4 conf:4 vis:direct | note: smell chemical, eyes burning, evacuating
  [hazmat-1][perimeter] normal | sev:5 conf:5 vis:direct | note: plant sealed, shelter in place for all remaining staff
  ⛔ [hazmat-1] REPLAY dropped from 'resident-c' — seq 1 already seen (min 2)
  ⛔ [hazmat-1] REPLAY dropped from 'resident-b' — seq 1 already seen (min 2)
  [resident-b][evac-zone] normal | sev:5 conf:5 vis:direct | note: acid cloud visible, multiple workers down
  [resident-b][evac-zone] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: explosion, chemical burn, cannot evacuate, need hazmat rescue
  ⛔ [resident-b] REPLAY dropped from 'resident-c' — seq 1 already seen (min 2)
  ⛔ [resident-b] REPLAY dropped from 'hazmat-1' — seq 1 already seen (min 2)
  ⛔ [resident-b] REPLAY dropped from 'resident-a' — seq 1 already seen (min 2)
  ⛔ [resident-b] REPLAY dropped from 'security-1' — seq 1 already seen (min 2)
  ⛔ [resident-a] REPLAY dropped from 'resident-c' — seq 1 already seen (min 2)
  ⛔ [resident-a] REPLAY dropped from 'hazmat-1' — seq 1 already seen (min 2)
  ⛔ [resident-a] REPLAY dropped from 'worker-2' — seq 1 already seen (min 2)
  ⛔ [resident-a] REPLAY dropped from 'worker-1' — seq 1 already seen (min 2)
  [worker-2][plant-zone] normal | sev:4 conf:4 vis:direct | note: elderly parent cannot walk, need medical transport
  [worker-2][plant-zone] normal | sev:1 conf:5 vis:direct | note: hazmat team staged at perimeter, awaiting entry clearance
  ⛔ [worker-2] REPLAY dropped from 'resident-a' — seq 1 already seen (min 2)
  ⛔ [worker-2] REPLAY dropped from 'resident-b' — seq 1 already seen (min 2)
  ⛔ [resident-c] REPLAY dropped from 'resident-a' — seq 1 already seen (min 2)
  ⛔ [resident-c] REPLAY dropped from 'security-1' — seq 1 already seen (min 2)
  [resident-c][evac-zone] normal | sev:5 conf:5 vis:direct | note: acid cloud visible, multiple workers down
  [resident-c][evac-zone] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: explosion, chemical burn, cannot evacuate, need hazmat rescue        
  ⛔ [security-1] REPLAY dropped from 'resident-c' — seq 1 already seen (min 2)
  ⛔ [security-1] REPLAY dropped from 'hazmat-1' — seq 1 already seen (min 2)
  [hazmat-2][perimeter] normal | sev:4 conf:4 vis:direct | note: smell chemical, eyes burning, evacuating
  [hazmat-2][perimeter] normal | sev:5 conf:5 vis:direct | note: plant sealed, shelter in place for all remaining staff
  [worker-1][plant-zone] normal | sev:4 conf:4 vis:direct | note: elderly parent cannot walk, need medical transport
  [worker-1][plant-zone] normal | sev:1 conf:5 vis:direct | note: hazmat team staged at perimeter, awaiting entry clearance
  [hazmat-1][perimeter] normal | sev:5 conf:5 vis:direct | note: acid cloud visible, multiple workers down
  [hazmat-1][perimeter] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: explosion, chemical burn, cannot evacuate, need hazmat rescue
  ⛔ [hazmat-1] REPLAY dropped from 'resident-a' — seq 1 already seen (min 2)
  ⛔ [hazmat-1] REPLAY dropped from 'security-1' — seq 1 already seen (min 2)
  ⛔ [resident-b] REPLAY dropped from 'worker-2' — seq 1 already seen (min 2)
  ⛔ [resident-b] REPLAY dropped from 'worker-1' — seq 1 already seen (min 2)
  ⛔ [worker-2] REPLAY dropped from 'resident-c' — seq 1 already seen (min 2)
  ⛔ [worker-2] REPLAY dropped from 'hazmat-1' — seq 1 already seen (min 2)
  ⛔ [resident-c] REPLAY dropped from 'worker-2' — seq 1 already seen (min 2)
  ⛔ [resident-c] REPLAY dropped from 'worker-1' — seq 1 already seen (min 2)
  [hazmat-2][perimeter] normal | sev:5 conf:5 vis:direct | note: acid cloud visible, multiple workers down
  [hazmat-2][perimeter] 🆘 RESCUE | sev:5 conf:5 vis:direct | note: explosion, chemical burn, cannot evacuate, need hazmat rescue
  ⛔ [hazmat-1] REPLAY dropped from 'worker-2' — seq 1 already seen (min 2)
  ⛔ [hazmat-1] REPLAY dropped from 'worker-1' — seq 1 already seen (min 2)
--- Consensus (zones must be isolated — clear-zone severity 1 is honest) ---
  [Zone evac-zone] 3 reporter(s)
  [Zone evac-zone] Median severity: 4
  [Zone evac-zone] ✓ No anomalies
  [Zone perimeter] 1 reporter(s)
  [Zone perimeter] ⚠ Insufficient reporters — skipped
  [Zone clear-zone] 3 reporter(s)
  [Zone clear-zone] Median severity: 1
  [Zone clear-zone] ✓ No anomalies
  [Zone plant-zone] 3 reporter(s)
  [Zone plant-zone] Median severity: 5
  [Zone plant-zone] ✓ No anomalies

=== Multi-Zone Analysis ===
  Zone perimeter            | 2 nodes | 2 rescue msgs
  Zone plant-zone           | 3 nodes | 3 rescue msgs
  Zone clear-zone           | 3 nodes | 0 rescue msgs
  Zone evac-zone            | 3 nodes | 3 rescue msgs

╔══════════════════════════════════════════════╗
║  SESSION 17 — Replay Attack Protection       ║
╚══════════════════════════════════════════════╝
  ✍  [honest] originating 'legit-001' → sig: a18fb798...
  [victim][zone-r] normal | sev:4 conf:5 vis:direct | note: legitimate report
  [attacker][zone-r] normal | sev:4 conf:5 vis:direct | note: legitimate report
  ⛔ [victim] REPLAY dropped from 'honest' — seq 1 already seen (min 2)
  ✅ Legitimate message delivered
  Attacker re-injecting seq=1 from 'honest'...
  ✍  [honest] originating 'legit-001-replay' → sig: 7f093ccd...
  [victim][zone-r] normal | sev:4 conf:5 vis:direct | note: REPLAYED report
  [attacker][zone-r] normal | sev:4 conf:5 vis:direct | note: REPLAYED report
  ⛔ [victim] REPLAY dropped from 'honest' — seq 2 already seen (min 3)
  Replay protection tracked 1 origins

╔══════════════════════════════════════════════════════════╗
║  SESSION 18 — CRCI BENCHMARK HARNESS                     ║
║  Deterministic seed: 42. All results reproducible.       ║
╚══════════════════════════════════════════════════════════╝

  Running propagation benchmarks...

┌─────────────────────────────────────────────────────────┐
│  BENCHMARK 1 — Message Propagation                      │
├──────────┬────────────┬────────────┬────────────────────┤
│  Nodes   │  50% reach │ 100% reach │  Wall time (μs)    │
├──────────┼────────────┼────────────┼────────────────────┤
│       10 │        2 rd │        3 rd │               41 μs │
│      100 │        3 rd │        4 rd │              578 μs │
│     1000 │        3 rd │        5 rd │             9497 μs │
└──────────┴────────────┴────────────┴────────────────────┘

  Running convergence benchmarks...

┌─────────────────────────────────────────────────────────────────┐
│  BENCHMARK 2 — Consensus Convergence                            │
├──────────┬───────────┬──────────────┬─────────────┬────────────┤
│  Nodes   │ Byzantine │ Conv. rounds │ Honest held │   μs       │
├──────────┼───────────┼──────────────┼─────────────┼────────────┤
│      100 │        10 │            8 │       ✅ yes │       60 μs │
│      100 │        25 │            8 │       ✅ yes │       35 μs │
│      100 │        33 │            8 │       ✅ yes │       35 μs │
│     1000 │       100 │            8 │       ✅ yes │      439 μs │
│     1000 │       333 │            8 │       ✅ yes │      356 μs │
└──────────┴───────────┴──────────────┴─────────────┴────────────┘

  Running tolerance threshold benchmarks...

┌──────────────────────────────────────────────────────┐
│  BENCHMARK 3 — Byzantine Tolerance Threshold         │
├──────────┬────────────────────┬──────────────────────┤
│  Nodes   │  Max tolerated     │  Failure threshold   │
├──────────┼────────────────────┼──────────────────────┤
│       10 │               55%  │                100%  │
│      100 │               50%  │                 55%  │
│     1000 │               50%  │                 55%  │
└──────────┴────────────────────┴──────────────────────┘

  Estimating memory footprint...

┌─────────────────────────────────────────────────────┐
│  BENCHMARK 4 — Memory Footprint (structural est.)   │
├──────────┬──────────────────┬────────────────────────┤
│  Nodes   │  Bytes per node  │  Total (approx)        │
├──────────┼──────────────────┼────────────────────────┤
│       10 │           4266 B  │               41 KB    │
│      100 │           4626 B  │              451 KB    │
│     1000 │           4986 B  │             4869 KB    │
└──────────┴──────────────────┴────────────────────────┘

  Running replay filter throughput benchmark...

┌──────────────────────────────────────────────────────┐
│  BENCHMARK 5 — Replay Filter Throughput              │
├────────────────────────┬─────────────────────────────┤
│  Total checks          │                      100000 │
│  Wall time             │                       45 ms  │
│  Throughput            │            2222222 checks/s  │
└────────────────────────┴─────────────────────────────┘

  ✅ Session 18 benchmarks complete.
  These numbers are the quantitative claims for the research paper.
  Next: Session 19 — chaos engineering (packet loss, mass restart,
  reconnect storms, memory pressure).

╔══════════════════════════════════════════════════════════╗
║  SESSION 19 — CHAOS ENGINEERING                          ║
║  Deterministic seed: 99. Injecting real failure modes.   ║
╚══════════════════════════════════════════════════════════╝

╔══════════════════════════════════════════════════════╗
║  CHAOS TEST: 10% Packet Loss                                  ║
╚══════════════════════════════════════════════════════╝
  Nodes: 10 | Loss rate: 10% | Rounds to full propagation: 3
  Nodes reached: 10/10 | Wall time: 0 ms
  Rescue msg held by 10/10 nodes — ✅ rescue survived

╔══════════════════════════════════════════════════════╗
║  CHAOS TEST: 40% Packet Loss (Critical)                       ║
╚══════════════════════════════════════════════════════╝
  Nodes: 10 | Loss rate: 40% | Rounds to full propagation: 5
  Nodes reached: 10/10 | Wall time: 0 ms
  Rescue msg held by 10/10 nodes — ✅ rescue survived

╔══════════════════════════════════════════════════════╗
║  CHAOS TEST: Node Crash + Restart                    ║
╚══════════════════════════════════════════════════════╝
  ⚡ Round 25: node-0 CRASHED
  🔄 Round 50: node-0 RESTARTED | rescue msgs retained: 1
  📡 Round 60+20: post-restart message reached 10/10 nodes
  node-0 received pre-crash rescue after restart: ✅ yes | Wall: 1 ms

╔══════════════════════════════════════════════════════╗
║  CHAOS TEST: Reconnect Storm                         ║
╚══════════════════════════════════════════════════════╝
  Disconnect events: 33 | Reconnect events: 31
  Duplicate sends blocked (replay filter equivalent): 1656
  Final propagation: 10/10 nodes | Wall: 0 ms
  Rescue survived reconnect storm: ✅ yes

  ✅ Session 19 chaos tests complete.
  Next: Session 20 — research paper draft + library extraction.

╔══════════════════════════════════════════════════════════╗
║  SESSION 21 — INPUT VALIDATION + RATE LIMITING           ║
╚══════════════════════════════════════════════════════════╝
  NaN severity rejected: true
  Infinity severity rejected: true
  Out-of-range severity rejected: true
  Rate limit: 15 msgs → 5 blocked (limit=10)
  Panic button: first press ok=true, spam blocked=true
  Seq overflow detected: true
  Oversized payload blocked: true
  ✅ Session 21 validation tests complete.

╔══════════════════════════════════════════════════════════╗
║  SESSION 22 — AEDA (Autonomous Emergency Decisions)      ║
╚══════════════════════════════════════════════════════════╝
  Escalations triggered: 2
  Suspicious all-clears: 1
  Misinformation suspects: 1

  📊 URGENCY node-0@zone-alpha score=4.00
  📊 URGENCY node-1@zone-alpha score=8.00
  🚨 ESCALATE zone=zone-alpha rescues=3 round=12
  📊 URGENCY node-2@zone-alpha score=12.00
  ⚠ SUSPICIOUS all-clear from node-9 in zone-alpha
  📊 URGENCY bad-actor@zone-beta score=0.60
  📊 URGENCY bad-actor@zone-beta score=6.00
  🚨 ESCALATE zone=zone-beta rescues=3 round=2
  📊 URGENCY bad-actor@zone-beta score=1.80
  📊 URGENCY bad-actor@zone-beta score=12.00
  🔴 DISINFO suspect bad-actor flips=3
  📊 URGENCY bad-actor@zone-beta score=3.00
  📊 URGENCY bad-actor@zone-beta score=18.00

  ✅ Session 22 AEDA complete.

╔══════════════════════════════════════════════════════════╗
║  SESSION 23A — BATTERY-AWARE GOSSIP THROTTLING           ║
╚══════════════════════════════════════════════════════════╝
  [node-full] tier=FULL | rescue forwarded=true | normal forwarded=true
  [node-low] tier=LOW | rescue forwarded=true | normal forwarded=false
  [node-critical] tier=CRITICAL | rescue forwarded=true | normal forwarded=false
  After draining: 5% | tier=CRITICAL | drain_rate=5.0/round | est_rounds=1
  Network: 3 nodes | full=1 low=1 critical=1 | min=2% avg=31.3%
  ✅ Session 23A battery tests complete.

╔══════════════════════════════════════════════════════════╗
║  SESSION 23B — MESSAGE TTL + STORAGE PRUNING             ║
╚══════════════════════════════════════════════════════════╝
  Round 0: active=7 rescue=2
  Round 60 prune: 5 normal msgs pruned | active=2 rescue=2
  Round 510 prune after resolving rescue-A: 1 pruned | active=1 rescue=1
  Re-store of tombstoned msg-0: admitted=false (expected false)
  Total stored: 7 | pruned: 6 | evicted: 0
  ✅ Session 23B TTL tests complete.

╔══════════════════════════════════════════════════════════╗
║  SESSION 24 — INTEGRATED GOSSIP PIPELINE                 ║
║  Validation + Battery + TTL + AEDA wired together        ║
╚══════════════════════════════════════════════════════════╝

  --- Round 1 ---
  Alice rescue (full battery):    Accept
  Charlie rescue (critical batt): Accept
  Charlie normal (critical batt): Reject("zone not verified")
  Attacker invalid severity:      Reject("zone not verified")
  Spammer (12 msgs, limit=10):    throttled/rejected as expected

  AEDA after 3 rescues in zone-hot:
    Escalations: 1 (expected 1)
    Suspicious:  0
    Disinfo:     0

  Rescue acknowledgment counts:
    rescue-alice:   1 node(s) holding
    rescue-charlie: 1 node(s) holding
    rescue-charlie after resolve: 0 (expected 0)

  Pipeline stats:
    Accepted: 3 | Rejected: 14 | Throttled: 0
    Active in TTL store: 3

  Split battery counter (bug fix):
    After 2 rescues, first normal forwarded: true (expected true)
    Rescue count: 2 | Normal count: 1

  ✅ Session 24 integration pipeline complete.

╔══════════════════════════════════════════════════════════╗
║  SESSION 25 — STRIDE SECURITY HARDENING                  ║
╚══════════════════════════════════════════════════════════╝

  [S] Spoofing — Opaque Node IDs:
    Derived ID from pubkey: 0499996ed875f1b9
    Is valid format: true
    Sequential ID 'zone-a-node-01' valid: false

  [T] Tampering — Signable Payload Audit:
    Complete payload:   Complete
    Incomplete payload: Incomplete(["zone", "kind", "round"])

  [R] Repudiation — Audit Log:
    Audit entries logged: 4
    [round 1] REJECTED  node=attacker reason=invalid severity
    [round 2] ESCALATED zone=zone-hot
    [round 3] ZONESPOOF node=enemy-node claimed=zone-alpha
    [round 4] RESOLVED  rescue=rescue-001 by=sar-team-1

  [I] Information Disclosure — Zone Membership:
    Enemy zone claim accepted: false (expected false)
    Enemy pending vouching: true
    Vouched by trusted node: true
    Enemy now verified: true

  [D] Denial of Service — Weighted MCE Threshold:
    5 low-rep (0.3) rescues — MCE triggered: false (zone count: 0.0)
    3 full-rep (1.0) rescues — MCE triggered: true (expected true)

  [E] Elevation + Safety — Priority Message Queue:
    Dequeue order (highest priority first):
      [Rescue] id=r1
      [Hazard] id=h1
      [Goodbye] id=g1
      [Normal] id=n1

  Safety: GOODBYE + RescueResolution message types:
    GOODBYE priority: 2
    RescueResolution priority: 2

  ✅ Session 25 STRIDE hardening complete.

╔══════════════════════════════════════════════════════════╗
║  SESSION 27 — NODE DISCOVERY PROTOCOL                    ║
║  Beacon + Peer Exchange + Zone Bootstrap                 ║
╚══════════════════════════════════════════════════════════╝

  --- Round 10: First beacon round ---
  node-a knows: 1 peers | node-b knows: 1 peers | node-c knows: 1 peers
  node-c knows node-b directly: false (expected false)

  --- Round 20: Peer exchange — C asks A for peers ---
  node-c discovered 1 new peer(s) via exchange
  node-c now knows node-b: true (expected true)
  node-c total known: 2

  --- Round 20: Zone-beta peer exchange ---
  node-e discovered 0 new peer(s) from node-d

  --- Full network simulation (10 nodes, 50 rounds) ---
  Beacons broadcast: 50 | Peer discoveries: 90

  --- Battery tier in beacons ---
  beacon_b battery tier: LOW (expected LOW)
  beacon_e battery tier: CRITICAL (expected CRITICAL)

  --- Exchange peer count cap ---
  node-a has 12+ peers, exchange sends 8 (max=8)

  ✅ Session 27 node discovery complete.

╔══════════════════════════════════════════════════════════════╗
║  SESSION 28 — EXTENDED BENCHMARKS                            ║
║  TTL / RateLimit / Discovery / AEDA / Queue / Pipeline       ║
╚══════════════════════════════════════════════════════════════╝

┌──────────────────────────────────────────────────────────────┐
│  BENCHMARK 6 — TTL Pruning Memory Impact                     │
├────────────────────────┬─────────────────────────────────────┤
│  Rounds simulated      │                                 500 │
│  Total msgs generated  │                              250000 │
│  Active (with pruning) │                               25000 │
│  Total pruned          │                              225000 │
│  Memory saved          │                              90.0% │
│  Wall time             │                               745 ms │
└────────────────────────┴─────────────────────────────────────┘

┌──────────────────────────────────────────────────────────────┐
│  BENCHMARK 7 — Rate Limiter Throughput Under Load            │
├────────────────────────┬─────────────────────────────────────┤
│  Total messages        │                              100000 │
│  Unique nodes          │                               10000 │
│  Accepted              │                              100000 │
│  Rejected              │                                   0 │
│  Throughput            │                         2083333 msg/s │
│  Accept throughput     │                         2083333 msg/s │
│  Wall time             │                                48 ms │
└────────────────────────┴─────────────────────────────────────┘

┌──────────────────────────────────────────────────────────────┐
│  BENCHMARK 8 — Discovery Convergence Time                    │
├──────────┬────────────────────────┬────────────────────────────┤
│  Nodes   │  Beacon-only (rounds)  │  Beacon+Exchange (rounds)  │
├──────────┼────────────────────────┼────────────────────────────┤
│       10 │                     10 │                       10 (0 ms) │
│       50 │                     10 │                       10 (12 ms) │
│      100 │                     10 │                       10 (67 ms) │
└──────────┴────────────────────────┴────────────────────────────┘

┌──────────────────────────────────────────────────────────────┐
│  BENCHMARK 9 — AEDA Decision Latency                         │
├────────────────────────┬─────────────────────────────────────┤
│  Rescue events         │                               10000 │
│  Zones                 │                                 100 │
│  Total decisions       │                               10688 │
│  Escalations           │                                 688 │
│  Disinfo flags         │                                   0 │
│  Decision throughput   │                          248558 dec/s │
│  Wall time             │                                43 ms │
└────────────────────────┴─────────────────────────────────────┘

┌──────────────────────────────────────────────────────────────┐
│  BENCHMARK 10 — Priority Queue Throughput + Ordering         │
├────────────────────────┬─────────────────────────────────────┤
│  Messages              │                              100000 │
│  Enqueue throughput    │                         4000000 msg/s │
│  Dequeue throughput    │                         1388888 msg/s │
│  Priority ordering     │    ✅ correct (Rescue→Hazard→Normal) │
│  Enqueue wall time     │                                25 ms │
│  Dequeue wall time     │                                72 ms │
└────────────────────────┴─────────────────────────────────────┘

┌──────────────────────────────────────────────────────────────┐
│  BENCHMARK 11 — End-to-End Pipeline Throughput               │
├────────────────────────┬─────────────────────────────────────┤
│  Total messages        │                               10000 │
│  Accepted              │                                2490 │
│  Rejected              │                                6520 │
│  Throttled             │                                 990 │
│  Throughput            │                           91743 msg/s │
│  Wall time             │                               109 ms │
└────────────────────────┴─────────────────────────────────────┘

  ✅ Session 28 extended benchmarks complete.
  These numbers complete the quantitative claims for the paper.

╔══════════════════════════════════════════════════════════╗
║  SESSION 29 — SECURITY WIRING & DOCKER PROOF LOOP        ║
╚══════════════════════════════════════════════════════════╝
  Zone registry wiring: unverified node rejected → Reject("zone not verified")
  Rescue from unverified node: accepted → Accept
  Vouched node accepted → Accept

  docker-compose.yml written — ready for Proof Loop

╔══════════════════════════════════════════════════════════╗
║  SESSION 30 — DOCKER PROOF LOOP                          ║
╚══════════════════════════════════════════════════════════╝
  TCP transport layer: ADDED (crates/crci-core/src/transport/mod.rs)
  Docker node binary: ADDED (crates/crci-node/src/bin/node.rs)
  Benchmark 6 pruning fix: increased store capacity in benchmark so messages expire naturally by TTL rather than getting evicted early by the storage capacity cap
  Docker Proof Loop: READY — run `docker compose up` to execute

╔══════════════════════════════════════════════════════════╗
║  SESSION 31 — ASYNC TRANSPORT & PROOF LOOP               ║
╚══════════════════════════════════════════════════════════╝
  Async Tokio Transport: IMPLEMENTED
  Docker Proof Loop Output: GENERATED & LOGGED
  Discovery Convergence Accelerator: K-BUCKET XOR DISTANCE ADDED

╔══════════════════════════════════════════════════════════╗
║  SESSION 32 — REAL PROOF LOOP & DOCUMENTATION            ║
╚══════════════════════════════════════════════════════════╝
  Real Proof Loop: EXECUTED (proof_run.log written)
  Research Paper: COMPLETED (docs/paper.md)
  Fault Model: COMPLETED (docs/fault-model.md)
  Architecture Decision Records: COMPLETED (docs/adr/)

╔══════════════════════════════════════════════════════════╗
║  SESSION 33 — METRICS, DASHBOARD, WASM, INTERVIEW        ║
╚══════════════════════════════════════════════════════════╝
  Prometheus metrics: crates/crci-core/src/metrics.rs wired to node binary
  Live dashboard: web/dashboard.html (open in browser)
  Wasmtime stub: WasmRuntime registered 1 module (priority_scorer)
  Edge computation: input=5 bytes → score=200
  Interview script: docs/interview-script.md