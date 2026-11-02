#!/usr/bin/env python3
"""Generate a testbed topology, print its metrics and pick the roles for the
mid-hop eviction experiment. Standard library only.

Topologies:
  rgg   random geometric graph: N points uniform in the unit square, an edge
        when two points are closer than r = sqrt(2 ln N / (pi N)) (expected
        mean degree 2 ln N); resampled until connected.
  mesh  random 4-regular multi-hop mesh (configuration model, no self-loops
        or parallel edges); resampled until connected.

Roles: a source S and a relay B such that removing B keeps the graph
connected, S is at least two hops from B, and the set D of nodes all of whose
shortest paths from S pass through B is as large as possible. D is the
"downstream of B" set: after B is evicted, S's messages must reach D over
longer alternate paths.

Usage: topology.py KIND N SEED OUT.json
"""

import json
import math
import random
import sys
from collections import deque


def rgg(n, rng):
    r = math.sqrt(2 * math.log(n) / (math.pi * n))
    for attempt in range(1, 1001):
        pts = [(rng.random(), rng.random()) for _ in range(n)]
        edges = set()
        for i in range(n):
            for j in range(i + 1, n):
                if math.dist(pts[i], pts[j]) < r:
                    edges.add((i, j))
        if connected(n, edges):
            return edges, {"radius": round(r, 4), "attempts": attempt}
    raise SystemExit("rgg: no connected sample in 1000 attempts")


def regular_mesh(n, rng, k=4):
    for attempt in range(1, 10001):
        stubs = [i for i in range(n) for _ in range(k)]
        rng.shuffle(stubs)
        edges = set()
        ok = True
        for a, b in zip(stubs[::2], stubs[1::2]):
            e = (min(a, b), max(a, b))
            if a == b or e in edges:
                ok = False
                break
            edges.add(e)
        if ok and connected(n, edges):
            return edges, {"degree": k, "attempts": attempt}
    raise SystemExit("mesh: no simple connected sample in 10000 attempts")


def adjacency(n, edges, removed=None):
    adj = [set() for _ in range(n)]
    for a, b in edges:
        if removed in (a, b):
            continue
        adj[a].add(b)
        adj[b].add(a)
    return adj


def bfs(adj, src):
    dist = [-1] * len(adj)
    dist[src] = 0
    q = deque([src])
    while q:
        u = q.popleft()
        for v in adj[u]:
            if dist[v] < 0:
                dist[v] = dist[u] + 1
                q.append(v)
    return dist


def connected(n, edges, removed=None):
    adj = adjacency(n, edges, removed)
    start = 0 if removed != 0 else 1
    dist = bfs(adj, start)
    return all(d >= 0 for i, d in enumerate(dist) if i != removed)


def clustering(adj):
    """Mean local clustering coefficient (nodes of degree < 2 count as 0)."""
    total = 0.0
    for u, nbrs in enumerate(adj):
        k = len(nbrs)
        if k < 2:
            continue
        links = sum(1 for v in nbrs for w in nbrs if v < w and w in adj[v])
        total += 2 * links / (k * (k - 1))
    return total / len(adj)


def metrics(n, edges, adj, dists):
    degrees = [len(a) for a in adj]
    pairs = [dists[i][j] for i in range(n) for j in range(i + 1, n)]
    return {
        "nodes": n,
        "edges": len(edges),
        "mean_degree": round(sum(degrees) / n, 3),
        "min_degree": min(degrees),
        "max_degree": max(degrees),
        "diameter": max(pairs),
        "avg_shortest_path": round(sum(pairs) / len(pairs), 4),
        "clustering": round(clustering(adj), 4),
    }


def pick_roles(n, edges, adj, dists, candidates=12):
    # Candidate relays: highest degree first, articulation points excluded.
    order = sorted(range(n), key=lambda v: (-len(adj[v]), v))
    best = None
    tried = 0
    for b in order:
        if tried >= candidates:
            break
        if not connected(n, edges, removed=b):
            continue
        tried += 1
        adj_b = adjacency(n, edges, removed=b)
        for s in range(n):
            if s == b or dists[s][b] < 2:
                continue
            d_without = bfs(adj_b, s)
            downstream = [
                x for x in range(n)
                if x not in (s, b) and d_without[x] > dists[s][x]
            ]
            key = (len(downstream), -s, -b)
            if best is None or key > best[0]:
                best = (key, s, b, downstream, d_without)
    if best is None:
        return None
    _, s, b, downstream, d_without = best
    return {
        "source": s,
        "byzantine": b,
        "byzantine_degree": len(adj[b]),
        "hops_source_to_byzantine": dists[s][b],
        "downstream": downstream,
        "downstream_hops_before": [dists[s][x] for x in downstream],
        "downstream_hops_after": [d_without[x] for x in downstream],
    }


def main():
    if len(sys.argv) != 5:
        raise SystemExit(__doc__)
    kind, n, seed, out = sys.argv[1], int(sys.argv[2]), int(sys.argv[3]), sys.argv[4]
    rng = random.Random(seed)
    if kind == "rgg":
        edges, params = rgg(n, rng)
    elif kind == "mesh":
        edges, params = regular_mesh(n, rng)
    else:
        raise SystemExit(f"unknown topology kind: {kind}")
    adj = adjacency(n, edges)
    dists = [bfs(adj, i) for i in range(n)]
    topo = {
        "kind": kind,
        "seed": seed,
        "params": params,
        "metrics": metrics(n, edges, adj, dists),
        "roles": pick_roles(n, edges, adj, dists),
        "edges": sorted(edges),
    }
    with open(out, "w") as f:
        json.dump(topo, f)
    roles = topo["roles"]
    print(f"topology kind={kind} seed={seed} {json.dumps(params)}")
    print("metrics " + " ".join(f"{k}={v}" for k, v in topo["metrics"].items()))
    if roles:
        print(
            f"roles source={roles['source']} byzantine={roles['byzantine']} "
            f"byzantine_degree={roles['byzantine_degree']} "
            f"hops_source_to_byzantine={roles['hops_source_to_byzantine']} "
            f"downstream={len(roles['downstream'])}"
        )
    else:
        print("roles none (no non-articulation relay two hops from any source)")


if __name__ == "__main__":
    main()
