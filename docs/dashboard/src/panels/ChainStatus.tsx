import React, { useEffect, useState } from 'react';
import { fetchChainHead, fetchHealth } from '../api';

export function ChainStatus() {
    const [status, setStatus] = useState<{ head_hash: string; head_seq: number; divergence_count: number } | null>(null);
    const [error, setError] = useState(false);

    useEffect(() => {
        const load = async () => {
            try {
                const [head, health] = await Promise.all([
                    fetchChainHead(),
                    fetchHealth()
                ]);
                setStatus({
                    head_hash: head.head_hash,
                    head_seq: head.head_seq,
                    divergence_count: health.divergence_alerts
                });
                setError(false);
            } catch (e) {
                setError(true);
            }
        };
        load();
        const int = setInterval(load, 3000);
        return () => clearInterval(int);
    }, []);

    if (error) {
        return (
            <div className="bg-gray-800 rounded-lg p-4 flex flex-col items-center justify-center h-full min-h-[150px]">
                <span className="text-gray-500 italic">Node offline</span>
            </div>
        );
    }

    if (!status) return null;

    const hasDivergence = status.divergence_count > 0;

    return (
        <div className={`bg-gray-800 rounded-lg p-4 flex flex-col h-full border-2 transition-colors duration-500 ${hasDivergence ? 'border-amber-500' : 'border-transparent'}`}>
            <h2 className="text-xl font-semibold mb-4 border-b border-gray-700 pb-2">
                Chain Status
            </h2>
            <div className="flex-1 flex flex-col space-y-4 justify-center">
                <div className="flex justify-between items-center">
                    <span className="text-gray-400">Block Index</span>
                    <span className="font-mono font-bold text-lg">{status.head_seq}</span>
                </div>
                <div className="flex justify-between items-center">
                    <span className="text-gray-400">Head Hash</span>
                    <span className="font-mono bg-gray-900 px-2 py-1 rounded text-sm text-blue-300">
                        {status.head_hash.substring(0, 12)}...
                    </span>
                </div>
                <div className="flex justify-between items-center">
                    <span className="text-gray-400">Divergences</span>
                    <span className={`font-bold px-2 py-0.5 rounded ${hasDivergence ? 'bg-amber-500/20 text-amber-500' : 'bg-green-500/20 text-green-500'}`}>
                        {status.divergence_count}
                    </span>
                </div>
            </div>
        </div>
    );
}
