use exchange_connectors::p2p_rest::P2PManager;

#[tokio::main]
async fn main() {
    println!("Testing P2P REST compilation...");
    let manager = P2PManager::new();
    println!("P2P Manager created successfully!");
}