const BASE = import.meta.env.VITE_NODE_URL || 'http://localhost:8080';

export async function fetchPeers(): Promise<{ peers: string[]; count: number }> {
    const res = await fetch(`${BASE}/peers`);
    if (!res.ok) throw new Error('Failed to fetch peers');
    return res.json();
}

export async function fetchStatus(): Promise<{ node_id: string; uptime_secs: number; message_count: number; peer_count: number; chain_head: string }> {
    const res = await fetch(`${BASE}/api/v1/status`);
    if (!res.ok) throw new Error('Failed to fetch status');
    return res.json();
}

export async function fetchChainHead(): Promise<{ head_hash: string; head_seq: number }> {
    const res = await fetch(`${BASE}/chain/head`);
    if (!res.ok) throw new Error('Failed to fetch chain head');
    return res.json();
}

export async function fetchHealth(): Promise<{ status: string; node_id: string; uptime_secs: number; byzantine_events: number; divergence_alerts: number }> {
    const res = await fetch(`${BASE}/health`);
    if (!res.ok) throw new Error('Failed to fetch health');
    return res.json();
}

export async function triggerPanic(): Promise<{ queued: boolean }> {
    const res = await fetch(`${BASE}/simulate/panic`, { method: 'POST' });
    if (!res.ok) throw new Error('Failed to trigger panic');
    return res.json();
}
