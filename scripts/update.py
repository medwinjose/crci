import re
with open('crates/crci-node/src/bin/node.rs', 'r', encoding='utf-8') as f:
    content = f.read()

content = content.replace('use crci::transport::AsyncTransport;', 'use crci::transport::{TcpTransport, TransportMultiplexer, Transport, TransportType, LoraTransport, LoraConfig, BleTransport, BleConfig};\nuse std::str::FromStr;')

content = re.sub(r'let transport = Arc::new\([\s\S]*?AsyncTransport::bind\([\s\S]*?\.expect\("Failed to create transport"\),\n    \);', '''let tcp_transport = TcpTransport::bind(node_id.clone(), listen_addr)
        .await
        .expect("Failed to create TCP transport");

    let lora_transport = LoraTransport::new(LoraConfig {
        frequency_hz: 915_000_000,
        spreading_factor: 7,
        bandwidth_khz: 125,
        coding_rate: 5,
    });

    let ble_transport = BleTransport::new(BleConfig {
        device_name: format!("CRCI-{}!", node_id),
        service_uuid: "0000180F-0000-1000-8000-00805F9B34FB".to_string(),
        mtu: 512,
    });

    let multiplexer = TransportMultiplexer::new(vec![
        Box::new(tcp_transport),
        Box::new(lora_transport),
        Box::new(ble_transport),
    ]);

    let transport = Arc::new(multiplexer);''', content)

content = content.replace('recv_transport.recv_loop(recv_pipeline, shutdown_rx).await;', '''loop {
            tokio::select! {
                res = recv_transport.receive() => {
                    if let Ok((_peer, msg)) = res {
                        let verdict = {
                            let mut pipeline_guard = recv_pipeline.lock().unwrap();
                            pipeline_guard.process(&msg)
                        };

                        if let crci::integration::PipelineVerdict::Accept = verdict {
                            // Forward the accepted message to all our peers
                            for p in &peers {
                                let _ = recv_transport.send(&p.to_string(), &msg).await;
                            }
                        }
                    } else {
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                }
                _ = shutdown_rx.recv() => {
                    break;
                }
            }
        }''')

content = content.replace('let _ = transport.broadcast(&msg).await;', '''for p in &peers {
                let _ = transport.send(&p.to_string(), &msg).await;
            }''')

content = content.replace('let _ = transport.broadcast(&rescue_msg).await;', '''for p in &peers {
                let _ = transport.send(&p.to_string(), &rescue_msg).await;
            }''')

content = content.replace('sync_transport.peers.len()', 'peers.len()')

content = re.sub(r'let accepted = transport[\s\S]*?\.load\(std::sync::atomic::Ordering::SeqCst\);', 'let accepted = p.accepted;', content)

with open('crates/crci-node/src/bin/node.rs', 'w', encoding='utf-8') as f:
    f.write(content)
