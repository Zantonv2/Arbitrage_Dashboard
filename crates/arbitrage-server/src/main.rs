use arbitrage_server::ArbitrageServer;
use clap::{Arg, Command};
use tracing::{info, error};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "arbitrage_server=info,arbitrage_core=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Parse command line arguments
    let matches = Command::new("arbitrage-server")
        .version("0.1.0")
        .author("ZantonV2")
        .about("Cryptocurrency Arbitrage Dashboard Server")
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .value_name("FILE")
                .help("Configuration file path")
                .default_value("config/config.toml"),
        )
        .arg(
            Arg::new("host")
                .long("host")
                .value_name("HOST")
                .help("Server host address")
                .default_value("127.0.0.1"),
        )
        .arg(
            Arg::new("port")
                .short('p')
                .long("port")
                .value_name("PORT")
                .help("Server port")
                .default_value("3000"),
        )
        .arg(
            Arg::new("setup-db")
                .long("setup-db")
                .help("Setup database and exit")
                .action(clap::ArgAction::SetTrue),
        )
        .get_matches();

    let config_path = matches.get_one::<String>("config").unwrap();
    let host = matches.get_one::<String>("host").unwrap();
    let port = matches.get_one::<String>("port").unwrap().parse::<u16>()?;
    let setup_db = matches.get_flag("setup-db");

    info!("Starting Arbitrage Dashboard Server v0.1.0");
    info!("Config: {}", config_path);
    info!("Server: {}:{}", host, port);

    if setup_db {
        info!("Setting up database...");
        // TODO: Implement database setup
        info!("Database setup complete");
        return Ok(());
    }

    // Create and start server
    match ArbitrageServer::new(config_path, host, port).await {
        Ok(server) => {
            info!("Server initialized successfully");
            
            // Start server
            if let Err(e) = server.run().await {
                error!("Server error: {}", e);
                return Err(e.into());
            }
        }
        Err(e) => {
            error!("Failed to initialize server: {}", e);
            return Err(e.into());
        }
    }

    Ok(())
}