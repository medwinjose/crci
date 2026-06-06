import React, { useEffect, useState, useRef } from 'react';
import {
    LineChart,
    Line,
    XAxis,
    YAxis,
    Tooltip,
    ResponsiveContainer,
} from 'recharts';

interface MessageThroughputProps {
    totalMessagesRef: React.MutableRefObject<number>;
}

interface DataPoint {
    time: string;
    throughput: number;
}

export function MessageThroughput({ totalMessagesRef }: MessageThroughputProps) {
    const [data, setData] = useState<DataPoint[]>([]);
    const lastCountRef = useRef(0);

    useEffect(() => {
        // Initialize last count to current so we don't get a huge spike on first tick
        lastCountRef.current = totalMessagesRef.current;
        
        const interval = setInterval(() => {
            const currentTotal = totalMessagesRef.current;
            const diff = currentTotal - lastCountRef.current;
            lastCountRef.current = currentTotal;

            const now = new Date();
            const timeLabel = now.toLocaleTimeString([], { hour12: false, hour: '2-digit', minute: '2-digit', second: '2-digit' });

            setData(prev => {
                const next = [...prev, { time: timeLabel, throughput: diff }];
                // Keep 60 seconds rolling window
                if (next.length > 60) return next.slice(next.length - 60);
                return next;
            });
        }, 1000);

        return () => clearInterval(interval);
    }, [totalMessagesRef]);

    return (
        <div className="bg-gray-800 rounded-lg p-4 flex flex-col h-full min-h-[300px]">
            <h2 className="text-xl font-semibold mb-4 border-b border-gray-700 pb-2">
                Message Throughput
            </h2>
            <div className="flex-1 min-h-0">
                <ResponsiveContainer width="100%" height="100%">
                    <LineChart
                        data={data}
                        margin={{ top: 20, right: 30, left: 0, bottom: 5 }}
                    >
                        <XAxis dataKey="time" stroke="#9ca3af" minTickGap={30} />
                        <YAxis stroke="#9ca3af" label={{ value: 'msgs / sec', angle: -90, position: 'insideLeft', fill: '#9ca3af' }} />
                        <Tooltip
                            contentStyle={{
                                backgroundColor: '#1f2937',
                                border: 'none',
                                color: '#fff',
                            }}
                            labelStyle={{ color: '#9ca3af' }}
                            itemStyle={{ color: '#3b82f6' }}
                        />
                        <Line
                            type="monotone"
                            dataKey="throughput"
                            stroke="#3b82f6"
                            strokeWidth={2}
                            dot={false}
                            isAnimationActive={false}
                        />
                    </LineChart>
                </ResponsiveContainer>
            </div>
        </div>
    );
}
