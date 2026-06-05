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

  useEffect(() => {
    let timeoutId: number;

    const connect = () => {
      setConnectionStatus('connecting');
      const ws = new WebSocket('ws://localhost:8080/ws');
      wsRef.current = ws;

      ws.onopen = () => {
        setConnectionStatus('connected');
      };

      ws.onmessage = (event) => {
        try {
          const msg: ApiMessage = JSON.parse(event.data);
          setMessages(prev => [msg, ...prev].slice(0, 50));
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

  return { messages, connectionStatus };
}
