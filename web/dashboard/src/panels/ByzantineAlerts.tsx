import React, { useEffect, useState, useRef } from 'react';

interface Alert {
    peer_id: string;
    expected_head_hash: string;
    actual_head_hash: string;
    divergence_seq: number;
    detected_at_ms: number;
}

export function ByzantineAlerts() {
    const [alerts, setAlerts] = useState<Alert[]>([]);
    const [unread, setUnread] = useState(0);
    const [error, setError] = useState(false);
    const wsRef = useRef<WebSocket | null>(null);

    useEffect(() => {
        let isMounted = true;
        
        const connect = () => {
            const base = import.meta.env.VITE_NODE_URL || 'http://localhost:8080';
            const wsUrl = base.replace(/^http/, 'ws') + '/ws/divergences';
            
            const ws = new WebSocket(wsUrl);
            wsRef.current = ws;

            ws.onmessage = (e) => {
                if (!isMounted) return;
                try {
                    const data = JSON.parse(e.data);
                    if (Array.isArray(data)) {
                        // Initial load
                        setAlerts(data.slice(-50));
                        setUnread(data.length);
                    } else {
                        // Single alert
                        setAlerts(prev => [...prev, data].slice(-50));
                        setUnread(prev => prev + 1);
                    }
                } catch (err) {
                    console.error('Failed to parse alert', err);
                }
            };

            ws.onclose = () => {
                if (isMounted) {
                    setError(true);
                    setTimeout(connect, 3000);
                }
            };

            ws.onopen = () => {
                if (isMounted) setError(false);
            };
        };

        connect();

        return () => {
            isMounted = false;
            if (wsRef.current) {
                wsRef.current.close();
            }
        };
    }, []);

    const handleClear = () => setUnread(0);

    if (error && alerts.length === 0) {
        return (
            <div className="bg-gray-800 rounded-lg p-4 flex flex-col items-center justify-center h-full min-h-[300px]">
                <span className="text-gray-500 italic">Node offline</span>
            </div>
        );
    }

    return (
        <div className="bg-gray-800 rounded-lg p-4 flex flex-col h-full">
            <h2 className="text-xl font-semibold mb-4 border-b border-gray-700 pb-2 flex justify-between items-center">
                <span>Byzantine Alerts</span>
                <div className="flex items-center space-x-3">
                    {unread > 0 && (
                        <span className="bg-red-500 text-white text-xs font-bold px-2 py-1 rounded-full animate-pulse">
                            {unread} New
                        </span>
                    )}
                    <button 
                        onClick={handleClear}
                        className="text-xs bg-gray-700 hover:bg-gray-600 px-2 py-1 rounded transition-colors"
                    >
                        Clear
                    </button>
                </div>
            </h2>
            <div className="flex-1 overflow-y-auto space-y-3">
                {alerts.map((alert, idx) => (
                    <div key={`${alert.divergence_seq}-${idx}`} className="bg-red-900/20 border border-red-900/50 p-3 rounded flex flex-col space-y-2">
                        <div className="flex justify-between items-start">
                            <span className="font-mono text-xs text-red-400 font-bold">
                                BYZANTINE FAULT DETECTED
                            </span>
                            <span className="text-xs text-gray-400">
                                {new Date(alert.detected_at_ms).toLocaleTimeString()}
                            </span>
                        </div>
                        <div className="text-sm">
                            <span className="text-gray-400">Peer:</span> <span className="font-mono">{alert.peer_id.substring(0, 12)}...</span>
                        </div>
                        <div className="text-sm">
                            <span className="text-gray-400">At block:</span> <span className="font-bold">{alert.divergence_seq}</span>
                        </div>
                        <div className="grid grid-cols-2 gap-2 text-xs font-mono mt-2">
                            <div className="bg-gray-900 p-2 rounded">
                                <div className="text-gray-500 mb-1">Expected</div>
                                <div className="text-green-400 truncate" title={alert.expected_head_hash}>{alert.expected_head_hash}</div>
                            </div>
                            <div className="bg-gray-900 p-2 rounded">
                                <div className="text-gray-500 mb-1">Actual</div>
                                <div className="text-red-400 truncate" title={alert.actual_head_hash}>{alert.actual_head_hash}</div>
                            </div>
                        </div>
                    </div>
                ))}
                {alerts.length === 0 && (
                    <div className="text-gray-500 italic text-center mt-8">No Byzantine activity detected.</div>
                )}
            </div>
        </div>
    );
}
