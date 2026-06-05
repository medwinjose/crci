import React, { useEffect, useState } from 'react';
import { BarChart, Bar, XAxis, YAxis, Tooltip, ResponsiveContainer, Cell } from 'recharts';
import { useWebSocket } from './hooks/useWebSocket';
import { fetchPeers, triggerPanic } from './api';

function App() {
  const { messages, connectionStatus } = useWebSocket();
  const [peers, setPeers] = useState<string[]>([]);
  const [peerCount, setPeerCount] = useState(0);
  const [panicStatus, setPanicStatus] = useState<'idle' | 'sent'>('idle');

  useEffect(() => {
    const loadPeers = async () => {
      try {
        const data = await fetchPeers();
        setPeers(data.peers);
        setPeerCount(data.count);
      } catch (e) {
        console.error(e);
      }
    };
    loadPeers();
    const interval = setInterval(loadPeers, 5000);
    return () => clearInterval(interval);
  }, []);

  const handlePanic = async () => {
    try {
      await triggerPanic();
      setPanicStatus('sent');
      setTimeout(() => setPanicStatus('idle'), 1000);
    } catch (e) {
      console.error(e);
    }
  };

  const getSeverityColor = (severity: number) => {
    if (severity >= 5) return '#ef4444'; // red-500
    if (severity === 4) return '#f97316'; // orange-500
    if (severity === 3) return '#eab308'; // yellow-500
    return '#22c55e'; // green-500
  };

  const chartData = [1, 2, 3, 4, 5].map(level => ({
    name: `Sev ${level}`,
    level,
    count: messages.filter(m => m.severity === level).length
  }));

  return (
    <div className="min-h-screen bg-gray-900 text-white flex flex-col p-6 space-y-6">
      <header className="flex justify-between items-center border-b border-gray-700 pb-4">
        <div className="flex items-center space-x-6">
          <h1 className="text-2xl font-bold">CRCI Live Dashboard</h1>
          <button 
            onClick={handlePanic}
            className={`px-4 py-2 rounded font-bold transition-colors ${panicStatus === 'sent' ? 'bg-red-600' : 'bg-red-900 hover:bg-red-800'}`}
          >
            {panicStatus === 'sent' ? 'Sent!' : '🚨 Inject Panic'}
          </button>
        </div>
        <div className="flex items-center space-x-2 font-mono text-sm font-bold">
          {connectionStatus === 'connected' ? (
            <span className="text-green-500">● CONNECTED</span>
          ) : connectionStatus === 'disconnected' ? (
            <span className="text-red-500">● DISCONNECTED</span>
          ) : (
            <span className="text-yellow-500">● CONNECTING</span>
          )}
        </div>
      </header>

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6 flex-1">
        {/* Node Status */}
        <div className="bg-gray-800 rounded-lg p-4 flex flex-col">
          <h2 className="text-xl font-semibold mb-4 border-b border-gray-700 pb-2">Node Status</h2>
          <div className="text-sm text-gray-400 mb-4">{peerCount} peers</div>
          <div className="flex-1 overflow-y-auto space-y-2">
            {peers.map((peer, idx) => (
              <div key={peer} className="bg-gray-700 p-3 rounded text-sm font-mono flex justify-between items-center">
                <span>{peer.substring(0, 12)}...</span>
                <span className="text-gray-400">#{idx}</span>
              </div>
            ))}
            {peers.length === 0 && <div className="text-gray-500 italic">No peers found...</div>}
          </div>
        </div>

        {/* Message Feed */}
        <div className="bg-gray-800 rounded-lg p-4 flex flex-col">
          <h2 className="text-xl font-semibold mb-4 border-b border-gray-700 pb-2">Message Feed</h2>
          <div className="flex-1 overflow-y-auto space-y-3">
            {messages.map((msg, idx) => (
              <div key={idx} className="bg-gray-700 p-3 rounded flex flex-col space-y-2">
                <div className="flex justify-between items-start">
                  <span className="font-mono text-xs text-blue-300">{msg.from.substring(0, 12)}...</span>
                  <span 
                    className="text-xs px-2 py-0.5 rounded font-bold" 
                    style={{ backgroundColor: getSeverityColor(msg.severity) + '40', color: getSeverityColor(msg.severity) }}
                  >
                    SEV {msg.severity}
                  </span>
                </div>
                <div className="text-sm truncate" title={msg.content}>
                  {msg.content.length > 60 ? msg.content.substring(0, 60) + '...' : msg.content}
                </div>
                <div className="text-xs text-gray-400">
                  {new Date(msg.timestamp_ms).toLocaleTimeString()}
                </div>
              </div>
            ))}
            {messages.length === 0 && <div className="text-gray-500 italic">Waiting for messages...</div>}
          </div>
        </div>

        {/* Severity Chart */}
        <div className="bg-gray-800 rounded-lg p-4 flex flex-col">
          <h2 className="text-xl font-semibold mb-4 border-b border-gray-700 pb-2">Severity Distribution</h2>
          <div className="flex-1 min-h-[300px]">
            <ResponsiveContainer width="100%" height="100%">
              <BarChart data={chartData} margin={{ top: 20, right: 30, left: 0, bottom: 5 }}>
                <XAxis dataKey="name" stroke="#9ca3af" />
                <YAxis stroke="#9ca3af" allowDecimals={false} />
                <Tooltip cursor={{ fill: '#374151' }} contentStyle={{ backgroundColor: '#1f2937', border: 'none', color: '#fff' }} />
                <Bar dataKey="count" radius={[4, 4, 0, 0]}>
                  {chartData.map((entry, index) => (
                    <Cell key={`cell-${index}`} fill={getSeverityColor(entry.level)} />
                  ))}
                </Bar>
              </BarChart>
            </ResponsiveContainer>
          </div>
        </div>
      </div>
    </div>
  );
}

export default App;
