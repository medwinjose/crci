import React, { useEffect, useState } from 'react';
import { fetchPeers } from '../api';

interface PeerData {
    peers: string[];
    count: number;
}

export function MeshTopology() {
    const [peerData, setPeerData] = useState<PeerData>({ peers: [], count: 0 });
    const [error, setError] = useState(false);

    useEffect(() => {
        const load = async () => {
            try {
                const data = await fetchPeers();
                setPeerData(data);
                setError(false);
            } catch (e) {
                setError(true);
            }
        };
        load();
        const int = setInterval(load, 5000);
        return () => clearInterval(int);
    }, []);

    if (error) {
        return (
            <div className="bg-gray-800 rounded-lg p-4 flex flex-col items-center justify-center h-full min-h-[300px]">
                <span className="text-gray-500 italic">Node offline</span>
            </div>
        );
    }

    return (
        <div className="bg-gray-800 rounded-lg p-4 flex flex-col h-full">
            <h2 className="text-xl font-semibold mb-4 border-b border-gray-700 pb-2 flex justify-between">
                <span>Mesh Topology</span>
                <span className="text-sm font-normal text-gray-400 bg-gray-700 px-2 py-1 rounded">
                    {peerData.count} Peers
                </span>
            </h2>
            <div className="flex-1 overflow-y-auto space-y-2">
                {peerData.peers.map((peer, idx) => (
                    <div key={peer} className="bg-gray-700 p-3 rounded text-sm font-mono flex justify-between items-center border-l-4 border-green-500">
                        <span>{peer.substring(0, 12)}...</span>
                        <span className="text-gray-400">Node #{idx + 1}</span>
                    </div>
                ))}
                {peerData.peers.length === 0 && (
                    <div className="text-gray-500 italic text-center mt-8">No peers connected...</div>
                )}
            </div>
        </div>
    );
}
