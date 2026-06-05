const BASE = 'http://localhost:8080';

export async function fetchPeers(): Promise<{ peers: string[]; count: number }> {
    const res = await fetch(`${BASE}/peers`);
    if (!res.ok) throw new Error('Failed to fetch peers');
    return res.json();
}

export async function fetchStatus(): Promise<{ node_id: string; uptime_secs: number; node_count: number; version: string }> {
    const res = await fetch(`${BASE}/status`);
    if (!res.ok) throw new Error('Failed to fetch status');
    return res.json();
}

export async function triggerPanic(): Promise<{ queued: boolean }> {
    const res = await fetch(`${BASE}/simulate/panic`, { method: 'POST' });
    if (!res.ok) throw new Error('Failed to trigger panic');
    return res.json();
}
