import React from "react";
import { useWebSocket } from "./hooks/useWebSocket";
import { triggerPanic } from "./api";
import { MeshTopology } from "./panels/MeshTopology";
import { ChainStatus } from "./panels/ChainStatus";
import { ByzantineAlerts } from "./panels/ByzantineAlerts";
import { MessageThroughput } from "./panels/MessageThroughput";

function App() {
  const { messages, connectionStatus, totalMessagesRef } = useWebSocket();
  const [panicStatus, setPanicStatus] = React.useState<"idle" | "sent">("idle");

  const handlePanic = async () => {
    try {
      await triggerPanic();
      setPanicStatus("sent");
      setTimeout(() => setPanicStatus("idle"), 1000);
    } catch (e) {
      console.error(e);
    }
  };

  const getSeverityColor = (severity: number) => {
    if (severity >= 5) return "#ef4444"; // red-500
    if (severity === 4) return "#f97316"; // orange-500
    if (severity === 3) return "#eab308"; // yellow-500
    return "#22c55e"; // green-500
  };

  return (
    <div className="min-h-screen bg-gray-900 text-white flex flex-col p-6 space-y-6">
      <header className="flex justify-between items-center border-b border-gray-700 pb-4">
        <div className="flex items-center space-x-6">
          <h1 className="text-2xl font-bold">CRCI Live Dashboard</h1>
          <button
            onClick={handlePanic}
            className={`px-4 py-2 rounded font-bold transition-colors ${panicStatus === "sent" ? "bg-red-600" : "bg-red-900 hover:bg-red-800"}`}
          >
            {panicStatus === "sent" ? "Sent!" : "🚨 Inject Panic"}
          </button>
        </div>
        <div className="flex items-center space-x-2 font-mono text-sm font-bold">
          {connectionStatus === "connected" ? (
            <span className="text-green-500">● CONNECTED</span>
          ) : connectionStatus === "disconnected" ? (
            <span className="text-red-500">● DISCONNECTED</span>
          ) : (
            <span className="text-yellow-500">● CONNECTING</span>
          )}
        </div>
      </header>

      <div className="grid grid-cols-1 lg:grid-cols-4 gap-6 flex-1">
        
        {/* Left Column (Topology & Chain Status) */}
        <div className="col-span-1 flex flex-col space-y-6">
          <div className="flex-1">
            <MeshTopology />
          </div>
          <div className="flex-none">
            <ChainStatus />
          </div>
        </div>

        {/* Center Column (Message Throughput & Message Feed) */}
        <div className="col-span-1 lg:col-span-2 flex flex-col space-y-6">
          <div className="h-64 flex-none">
            <MessageThroughput totalMessagesRef={totalMessagesRef} />
          </div>
          <div className="bg-gray-800 rounded-lg p-4 flex flex-col flex-1 min-h-[300px]">
            <h2 className="text-xl font-semibold mb-4 border-b border-gray-700 pb-2 flex justify-between">
              <span>Live Message Stream</span>
              <span className="text-sm font-normal text-gray-400 bg-gray-700 px-2 py-1 rounded">
                  Latest 50
              </span>
            </h2>
            <div className="flex-1 overflow-y-auto space-y-3">
              {messages.map((msg, idx) => (
                <div
                  key={`${msg.timestamp_ms}-${idx}`}
                  className="bg-gray-700 p-3 rounded flex flex-col space-y-2"
                >
                  <div className="flex justify-between items-start">
                    <span className="font-mono text-xs text-blue-300">
                      {msg.from.substring(0, 12)}...
                    </span>
                    <span
                      className="text-xs px-2 py-0.5 rounded font-bold"
                      style={{
                        backgroundColor: getSeverityColor(msg.severity) + "40",
                        color: getSeverityColor(msg.severity),
                      }}
                    >
                      SEV {msg.severity}
                    </span>
                  </div>
                  <div className="text-sm truncate" title={msg.content}>
                    {msg.content.length > 80
                      ? msg.content.substring(0, 80) + "..."
                      : msg.content}
                  </div>
                  <div className="text-xs text-gray-400">
                    {new Date(msg.timestamp_ms).toLocaleTimeString()}
                  </div>
                </div>
              ))}
              {messages.length === 0 && (
                <div className="text-gray-500 italic text-center mt-8">
                  Waiting for messages...
                </div>
              )}
            </div>
          </div>
        </div>

        {/* Right Column (Byzantine Alerts) */}
        <div className="col-span-1 flex flex-col space-y-6">
          <div className="flex-1">
            <ByzantineAlerts />
          </div>
        </div>

      </div>
    </div>
  );
}

export default App;
