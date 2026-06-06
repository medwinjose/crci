import { useState, useEffect, useRef } from 'react';

export interface ApiMessage {
  from: string;
  severity: number;
  content: string;
  timestamp_ms: number;
}

export function useWebSocket() {
  const [messages, setMessages] = useState<ApiMessage[]>([]);
  const [connectionStatus, setConnectionStatus] = useState<'connecting' | 'connected' | 'disconnected'>('connecting');
  const wsRef = useRef<WebSocket | null>(null);
  const totalMessagesRef = useRef(0);

  useEffect(() => {
    let timeoutId: number;

    const connect = () => {
      setConnectionStatus('connecting');
      const base = import.meta.env.VITE_NODE_URL || 'http://localhost:8080';
      const wsUrl = base.replace(/^http/, 'ws') + '/ws';
      const ws = new WebSocket(wsUrl);
      wsRef.current = ws;

      ws.onopen = () => {
        setConnectionStatus('connected');
      };

      ws.onmessage = (event) => {
        try {
          const msg: ApiMessage = JSON.parse(event.data);
          setMessages(prev => [msg, ...prev].slice(0, 50));
          totalMessagesRef.current += 1;
        } catch (e) {
          console.error('Failed to parse WebSocket message', e);
        }
      };

      ws.onclose = () => {
        setConnectionStatus('disconnected');
        timeoutId = window.setTimeout(connect, 3000);
      };

      ws.onerror = () => {
        ws.close();
      };
    };

    connect();

    return () => {
      clearTimeout(timeoutId);
      if (wsRef.current) {
        wsRef.current.onclose = null;
        wsRef.current.close();
      }
    };
  }, []);

  return { messages, connectionStatus, totalMessagesRef };
}
