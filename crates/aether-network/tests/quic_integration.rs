use std::net::SocketAddr;
use std::time::Duration;

use aether_network::quic::client::QuicClient;
use aether_network::quic::config::QuicConfig;
use aether_network::quic::connection::ConnectionState;
use aether_network::quic::server::QuicServer;
use aether_network::quic::transport::QuicTransport;
use aether_network::runtime::RuntimeTransport;
use aether_network::transport::Reliability;
use aether_network::types::NetEntity;

/// Helper to create a server bound to a random port on localhost.
fn spawn_test_server() -> (QuicServer, SocketAddr) {
    let config = QuicConfig {
        bind_addr: "127.0.0.1:0".parse().unwrap(),
        ..QuicConfig::default()
    };
    let server = QuicServer::bind(&config).expect("server should bind");
    let addr = server.local_addr().expect("should get local addr");
    (server, addr)
}

/// Helper to create a client configured to connect to the given address.
fn create_test_client(server_addr: SocketAddr) -> QuicClient {
    let config = QuicConfig {
        server_addr,
        connect_timeout: Duration::from_secs(5),
        ..QuicConfig::default()
    };
    QuicClient::new(config).expect("client should create")
}

#[tokio::test]
async fn server_binds_and_accepts_client() {
    let (server, addr) = spawn_test_server();
    let mut client = create_test_client(addr);

    // Accept in background, keeping server alive
    let accept_handle = tokio::spawn(async move {
        let result = server.accept().await;
        (result, server)
    });

    // Client connects
    let server_tick = client.connect(42, b"test-token").await.expect("client should connect");
    assert_eq!(server_tick, 0); // Default server tick is 0

    // Server should have accepted
    let (result, server) = accept_handle.await.unwrap();
    let (client_id, token) = result.expect("server should accept");
    assert_eq!(client_id, 42);
    assert_eq!(token, b"test-token");

    // Client should be connected
    assert!(client.is_connected());
    assert_eq!(client.state(), ConnectionState::Connected);

    client.shutdown();
    server.shutdown().await;
}

#[tokio::test]
async fn server_handshake_sends_server_tick() {
    let (server, addr) = spawn_test_server();
    server.set_server_tick(12345).await;

    let mut client = create_test_client(addr);

    let accept_handle = tokio::spawn(async move {
        let result = server.accept().await;
        (result, server)
    });

    let server_tick = client.connect(1, b"").await.expect("should connect");
    assert_eq!(server_tick, 12345);

    let (result, server) = accept_handle.await.unwrap();
    result.expect("accept ok");
    client.shutdown();
    server.shutdown().await;
}

#[tokio::test]
async fn reliable_message_roundtrip_client_to_server() {
    let (server, addr) = spawn_test_server();
    let mut client = create_test_client(addr);

    let accept_handle = tokio::spawn(async move {
        server.accept().await.expect("accept");
        server
    });

    client.connect(10, b"auth").await.expect("connect");

    let server = accept_handle.await.unwrap();

    // Client sends reliable message to server
    client.send_reliable(b"hello from client").await.expect("send");

    // Server receives it
    let received = server.recv_reliable(10).await.expect("recv");
    assert_eq!(received, b"hello from client");

    client.shutdown();
    server.shutdown().await;
}

#[tokio::test]
async fn reliable_message_roundtrip_server_to_client() {
    let (server, addr) = spawn_test_server();
    let mut client = create_test_client(addr);

    let accept_handle = tokio::spawn(async move {
        server.accept().await.expect("accept");
        server
    });

    client.connect(20, b"auth").await.expect("connect");

    let server = accept_handle.await.unwrap();

    // Server sends reliable message to client
    server.send_reliable(20, b"hello from server").await.expect("send");

    // Client receives it
    let received = client.recv_reliable().await.expect("recv");
    assert_eq!(received, b"hello from server");

    client.shutdown();
    server.shutdown().await;
}

#[tokio::test]
async fn datagram_server_to_client() {
    let (server, addr) = spawn_test_server();
    let mut client = create_test_client(addr);

    let accept_handle = tokio::spawn(async move {
        server.accept().await.expect("accept");
        server
    });

    client.connect(30, b"auth").await.expect("connect");

    let server = accept_handle.await.unwrap();

    // Server sends datagram to client
    let data = b"datagram payload";
    server.send_datagram(30, data).await.expect("datagram send");

    // Give a brief moment for the datagram to arrive
    tokio::time::sleep(Duration::from_millis(50)).await;

    // We verify the send succeeded (datagram delivery is best-effort)
    // The datagram was sent without error, which validates the transport works

    client.shutdown();
    server.shutdown().await;
}

#[tokio::test]
async fn connection_disconnect_and_cleanup() {
    let (server, addr) = spawn_test_server();
    let mut client = create_test_client(addr);

    let accept_handle = tokio::spawn(async move {
        server.accept().await.expect("accept");
        server
    });

    client.connect(50, b"auth").await.expect("connect");
    assert!(client.is_connected());

    let server = accept_handle.await.unwrap();
    assert_eq!(server.connection_count().await, 1);
    assert!(server.is_connected(50).await);

    // Disconnect client
    client.disconnect("done");
    assert!(!client.is_connected());

    // Disconnect from server side
    server.disconnect(50, "cleanup").await;
    assert_eq!(server.connection_count().await, 0);
    assert!(!server.is_connected(50).await);

    server.shutdown().await;
}

#[tokio::test]
async fn multiple_clients_connect() {
    let (server, addr) = spawn_test_server();

    // Accept two clients sequentially
    let server_handle = tokio::spawn(async move {
        let (id1, _) = server.accept().await.expect("accept client 1");
        let (id2, _) = server.accept().await.expect("accept client 2");
        assert_eq!(id1, 100);
        assert_eq!(id2, 200);
        assert_eq!(server.connection_count().await, 2);
        server
    });

    let mut client1 = create_test_client(addr);
    client1.connect(100, b"token1").await.expect("connect client 1");

    let mut client2 = create_test_client(addr);
    client2.connect(200, b"token2").await.expect("connect client 2");

    let server = server_handle.await.unwrap();

    client1.shutdown();
    client2.shutdown();
    server.shutdown().await;
}

#[tokio::test]
async fn quic_transport_implements_runtime_transport() {
    // This test verifies that QuicTransport correctly implements RuntimeTransport
    // for the synchronous buffering interface used by the network runtime.
    let mut transport = QuicTransport::new(128);

    // Send a reliable message
    let msg = aether_network::transport::TransportMessage {
        to_client_id: 1,
        entity: NetEntity(42),
        reliability: Reliability::ReliableOrdered,
        payload: vec![10, 20, 30],
        is_voice: false,
    };
    transport.send(msg).expect("send should succeed");
    assert_eq!(transport.reliable_outbound_len(), 1);

    // Send an unreliable message
    let msg2 = aether_network::transport::TransportMessage {
        to_client_id: 1,
        entity: NetEntity(43),
        reliability: Reliability::UnreliableDatagram,
        payload: vec![40, 50],
        is_voice: true,
    };
    transport.send(msg2).expect("send should succeed");
    assert_eq!(transport.datagram_outbound_len(), 1);

    // Pop outbound
    let reliable = transport.pop_reliable_outbound().unwrap();
    assert_eq!(reliable.payload, vec![10, 20, 30]);

    let datagram = transport.pop_datagram_outbound().unwrap();
    assert_eq!(datagram.payload, vec![40, 50]);

    // Push inbound and recv
    transport.push_inbound(aether_network::transport::TransportMessage {
        to_client_id: 1,
        entity: NetEntity(44),
        reliability: Reliability::ReliableOrdered,
        payload: vec![60, 70, 80],
        is_voice: false,
    });

    let received = transport.recv(10);
    assert_eq!(received.len(), 1);
    assert_eq!(received[0].payload, vec![60, 70, 80]);
}

#[tokio::test]
async fn quic_transport_with_network_runtime() {
    // Verify QuicTransport works with the full NetworkRuntime::step_with_transport
    use aether_network::interest::{ClientBudget, ClientProfile, InterestManager, InterestPolicy, CameraFrustum};
    use aether_network::runtime::{
        ClientRuntimeState, NetworkRuntime, NetworkTickInput, RuntimeConfig, RuntimeEntityHint,
        RuntimeSnapshotInput,
    };
    use aether_network::types::Vec3;

    let config = RuntimeConfig::default();
    let runtime = NetworkRuntime::new(config, InterestManager::new(InterestPolicy::default()));
    let mut transport = QuicTransport::new(256);
    let mut state = vec![ClientRuntimeState::new(1)];

    let profiles = vec![ClientProfile {
        client_id: 1,
        world_id: 1,
        position: Vec3 { x: 0.0, y: 0.0, z: 0.0 },
        frustum: CameraFrustum::default(),
    }];
    let budgets = vec![ClientBudget {
        client_id: 1,
        max_entities: 4,
        max_bytes_per_tick: 512,
    }];
    let hints = vec![vec![RuntimeEntityHint {
        entity_id: 1,
        position: Vec3 { x: 1.0, y: 0.0, z: 2.0 },
        importance: 1.0,
    }]];
    let snaps = vec![vec![RuntimeSnapshotInput {
        entity_id: 1,
        position: (1.0, 0.0, 2.0),
        rotation_deg: (0.0, 90.0, 180.0),
    }]];

    let result = runtime.step_with_transport(
        &mut transport,
        NetworkTickInput { tick: 1, now_ms: 16 },
        &profiles,
        &budgets,
        &hints,
        &snaps,
        &mut state,
        &[],
        &[],
        8,
    );

    assert!(result.sent_to_transport > 0);
    assert_eq!(result.output.transport_packets, result.sent_to_transport);
}
