# ArbFinder

A high-performance cryptocurrency arbitrage finder and trading bot built in Rust.

## Features

- **Multi-Exchange Support**: Binance, Coinbase Pro, and Kraken
- **Real-time Market Data**: WebSocket connections for live price feeds
- **Arbitrage Detection**: Triangular and cross-exchange arbitrage strategies
- **ML-Powered Predictions**: XGBoost and Neural Network models for opportunity classification
- **Risk Management**: Position limits, stop-loss, and drawdown protection
- **Paper Trading**: Test strategies without real money
- **Monitoring & Alerts**: Comprehensive logging, metrics, and notifications
- **High Performance**: Built with Rust for speed and reliability

## Architecture

```
ArbFinder/
├── crates/
│   ├── core/           # Core types and utilities
│   ├── exchange/       # Exchange trait and common functionality
│   ├── orderbook/      # Order book management
│   ├── strategy/       # Trading strategies
│   ├── execution/      # Trade execution engine
│   ├── monitoring/     # Logging, metrics, and alerts
│   └── ml/             # ML inference with ONNX Runtime
├── adapters/
│   ├── binance/        # Binance exchange adapter
│   ├── coinbase/       # Coinbase Pro exchange adapter
│   └── kraken/         # Kraken exchange adapter
├── models/             # Trained ML models
│   ├── arbitrage_net.onnx    # PyTorch model (ONNX)
│   ├── xgboost_classifier.json
│   └── scaler_params.json
├── scripts/            # Training scripts
│   ├── train.py        # Full training pipeline
│   ├── train_xgb.py    # XGBoost training
│   ├── train_nn.py     # Neural network training
│   └── export.py       # ONNX export
├── data/               # Training data
│   └── arbitrage_training_data.csv
├── analysis/           # Jupyter notebooks
│   └── analysis.ipynb
└── src/
    ├── main.rs         # Main application entry point
    └── lib.rs          # Library exports
```

## Quick Start

### Prerequisites

- Rust 1.70+
- Exchange API credentials (for live trading)

### Installation

1. Clone the repository:

```bash
git clone https://github.com/elcruzo/arb-finder.git
cd arbfinder
```

2. Build the project:

```bash
cargo build --release
```

3. Copy and configure the settings:

```bash
cp config.toml my-config.toml
# Edit my-config.toml with your exchange credentials
```

### Configuration

Edit `config.toml` with your exchange API credentials:

```toml
[exchanges.binance]
api_key = "your_binance_api_key"
api_secret = "your_binance_api_secret"
sandbox = true  # Set to false for live trading

[exchanges.coinbase]
api_key = "your_coinbase_api_key"
api_secret = "your_coinbase_api_secret"
passphrase = "your_coinbase_passphrase"
sandbox = true

[exchanges.kraken]
api_key = "your_kraken_api_key"
api_secret = "your_kraken_api_secret"
```

### Usage

#### Paper Trading (Recommended for testing)

```bash
# Run with paper trading enabled
cargo run -- run --paper-trading --config my-config.toml
```

#### Live Trading

```bash
# Run with live trading (use with caution!)
cargo run -- run --config my-config.toml
```

#### Health Check

```bash
# Check system health
cargo run -- health
```

#### Command Line Options

```bash
# Show help
cargo run -- --help

# Run with custom log level
cargo run -- run --log-level debug

# Show version
cargo run -- version
```

## Machine Learning

The project includes ML models for predicting profitable arbitrage opportunities.

### Models

| Model | Accuracy | F1 Score | ROC AUC |
|-------|----------|----------|---------|
| XGBoost | 92.0% | 88.9% | 98.0% |
| Neural Network | 92.5% | 89.6% | 98.1% |

### Training

```bash
# Train all models (XGBoost + Neural Network)
python3 scripts/train.py

# Export to ONNX for Rust inference
python3 scripts/export.py
```

### Requirements

```bash
pip install pandas numpy scikit-learn xgboost torch joblib onnx
```

### Features Used

- Spread between exchanges (Binance, Coinbase, Kraken)
- Trading volumes
- Market volatility
- Time-based features (hour, day of week)
- Liquidity scores

## Monitoring

### Metrics

The application exposes Prometheus metrics on port 9090 by default:

```bash
curl http://localhost:9090/metrics
```

### Logs

Logs are written to both console and file (if enabled):

- Console: Structured JSON logs
- File: `logs/arbfinder.log` (configurable)

### Alerts

Configure webhook URLs in `config.toml` for real-time alerts:

```toml
[monitoring]
enable_alerts = true
alert_webhook_url = "https://hooks.slack.com/services/YOUR/SLACK/WEBHOOK"
```

## Development

### Running Tests

```bash
# Run all tests
cargo test

# Run tests for specific crate
cargo test -p arbfinder-core
```

### Building Documentation

```bash
cargo doc --open
```

### Linting

```bash
cargo clippy -- -D warnings
```

### Formatting

```bash
cargo fmt
```

## Risk Management

**IMPORTANT SAFETY NOTES:**

1. **Start with Paper Trading**: Always test strategies with paper trading first
2. **Use Small Amounts**: Start with small position sizes when going live
3. **Monitor Closely**: Keep an eye on the bot's performance and logs
4. **Set Limits**: Configure appropriate risk limits in the config file
5. **Emergency Stop**: The bot includes emergency stop functionality

### Risk Controls

- Maximum position sizes per trade and symbol
- Daily loss limits
- Drawdown protection
- Rate limiting for API calls
- Order timeout handling
- Emergency stop conditions

## Strategies

### Triangular Arbitrage

Finds arbitrage opportunities within a single exchange using three currency pairs.

Example: BTC/USDT → ETH/BTC → ETH/USDT → USDT

### Cross-Exchange Arbitrage (Future)

Finds price differences for the same asset across different exchanges.

## API Rate Limits

The bot respects exchange API rate limits:

- **Binance**: 1200 requests per minute
- **Coinbase Pro**: 10 requests per second
- **Kraken**: 15-20 requests per minute

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests
5. Submit a pull request

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Disclaimer

This software is for educational and research purposes only. Cryptocurrency trading involves substantial risk of loss. The authors are not responsible for any financial losses incurred through the use of this software.

**Use at your own risk and never trade with money you cannot afford to lose.**

## Support

- Create an issue for bug reports or feature requests
- Check the documentation for common questions
- Review the logs for troubleshooting information

## Roadmap

- [ ] Additional exchange adapters (Bybit, OKX, etc.)
- [ ] More arbitrage strategies
- [ ] Web-based dashboard
- [ ] Backtesting framework
- [x] Machine learning integration
- [ ] Mobile notifications
