# Arbitrage Dashboard

A sophisticated desktop application for monitoring cryptocurrency arbitrage opportunities across multiple exchanges in real-time. Built with Rust backend and Svelte frontend.

## 🚀 Features

- **Real-time Monitoring**: Live order book data from multiple exchanges (ByBit, BingX, Hyperliquid, and more)
- **Arbitrage Detection**: Automated identification of profitable price discrepancies
- **Confidence Scoring**: AI-powered confidence ratings for each opportunity
- **Risk Management**: Smart position sizing based on order book depth and slippage analysis
- **Execution Assistance**: Pre-filled trade instructions for manual execution
- **Analytics Dashboard**: Comprehensive performance tracking and reporting
- **Simulation Engine**: Backtest strategies against historical data
- **Secure Storage**: Encrypted API key management
- **Desktop Notifications**: Real-time alerts for high-confidence opportunities

## 🏗️ Architecture

- **Backend**: Rust with Axum HTTP server and WebSocket support
- **Frontend**: Svelte 5 + Tailwind CSS 4 + Vite 7
- **Database**: SQLite with SQLx for persistence
- **Communication**: REST API for queries, WebSocket for real-time updates
- **Security**: AES-256-GCM encryption for sensitive data

## 📋 Requirements

- Rust 1.84+ (latest stable)
- Node.js 18+ (for frontend development)
- SQLite 3.35+

## 🛠️ Installation

1. **Clone the repository**:
   ```bash
   git clone https://github.com/ZantonV2/Arbitrage_Dashboard.git
   cd Arbitrage_Dashboard
   ```

2. **Build the project**:
   ```bash
   cargo build --release
   ```

3. **Set up the database**:
   ```bash
   cargo run --bin arbitrage-server -- --setup-db
   ```

4. **Configure exchanges** (copy and edit):
   ```bash
   cp config/config.example.toml config/config.toml
   ```

5. **Run the application**:
   ```bash
   cargo run --bin arbitrage-server
   ```

The dashboard will be available at `http://localhost:3000`

## ⚙️ Configuration

Edit `config/config.toml` to configure:

- Exchange API credentials
- Trading pairs to monitor
- Profit thresholds
- Risk parameters
- Notification settings

## 🔒 Security

- API keys are encrypted at rest using AES-256-GCM
- Master password required for key decryption
- No sensitive data in logs or frontend
- Secure credential storage per platform

## 📊 Supported Exchanges

- **ByBit**: Full WebSocket support with order book streaming
- **BingX**: Real-time data with rate limit handling
- **Hyperliquid**: Native integration with L2 order books
- More exchanges coming soon...

## 🧪 Testing

Run the comprehensive test suite:

```bash
# Unit tests
cargo test

# Property-based tests
cargo test --features proptest

# Integration tests
cargo test --test integration
```

## 📈 Performance

- **Latency**: <50ms internal processing time
- **Throughput**: Supports 10+ concurrent exchange connections
- **Memory**: <500MB RAM under normal operation
- **UI**: 60fps responsive interface

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## 📝 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## ⚠️ Disclaimer

This software is for educational and informational purposes only. Cryptocurrency trading involves substantial risk of loss. The authors are not responsible for any financial losses incurred through the use of this software. Always conduct your own research and consider consulting with a financial advisor before making investment decisions.

## 🙏 Acknowledgments

- Built with the amazing Rust ecosystem
- Inspired by the DeFi and arbitrage trading community
- Special thanks to all exchange API providers

---

**Repository**: https://github.com/ZantonV2/Arbitrage_Dashboard  
**Author**: ZantonV2  
**Version**: 0.1.0