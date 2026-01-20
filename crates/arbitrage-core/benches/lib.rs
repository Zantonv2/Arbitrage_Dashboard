use arbitrage_core::{
    strategies::{
        base::Strategy, cex_arbitrage::CexArbitrageStrategy,
        convergence_arbitrage::ConvergenceArbitrageStrategy,
        cross_exchange_arbitrage::CrossExchangeArbitrageStrategy,
        funding_rate_arbitrage::FundingRateArbitrageStrategy,
        hedged_funding::HedgedFundingStrategy, latency_arbitrage::LatencyArbitrageStrategy,
        new_listing_arbitrage::NewListingArbitrageStrategy, registry::StrategyRegistry,
        spot_perp_arbitrage::SpotPerpArbitrageStrategy, spread_capture::SpreadCaptureStrategy,
        stablecoin_arbitrage::StablecoinArbitrageStrategy,
    },
    types::{
        exchange_constants::ALL_EXCHANGES, ExchangeId, FeeSchedule, FundingRate, OrderBook,
        OrderBookLevel, OrderType, Side, Symbol, TickerData,
    },
    FeeSchedule as CoreFeeSchedule, Order, Signal, ToBps,
};
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use std::sync::Arc;

fn create_test_symbol() -> Symbol {
    Symbol::new("BTC", "USDT")
}

fn create_test_orderbook(
    exchange: ExchangeId,
    symbol: Symbol,
    bid_price: Decimal,
    ask_price: Decimal,
) -> OrderBook {
    OrderBook::new(
        exchange,
        symbol,
        vec![OrderBookLevel::new(bid_price, Decimal::from(1))],
        vec![OrderBookLevel::new(ask_price, Decimal::from(1))],
    )
}

fn create_test_fee_schedule(exchange: ExchangeId) -> CoreFeeSchedule {
    CoreFeeSchedule::new(exchange, Decimal::from(10), Decimal::from(20))
}

mod detection_benchmarks {
    use super::*;

    pub fn benchmark_cex_arbitrage_detection(c: &mut Criterion) {
        let mut group = c.benchmark_group("detection/cex_arbitrage");
        let strategy = CexArbitrageStrategy::new();

        group.bench_function("strategy_detect", |b| {
            b.iter(|| {
                let bundle = arbitrage_core::strategies::MarketBundle::new();
                black_box(strategy.detect(black_box(&bundle)))
            })
        });

        group.finish();
    }

    pub fn benchmark_funding_rate_detection(c: &mut Criterion) {
        let mut group = c.benchmark_group("detection/funding_rate");
        let strategy = FundingRateArbitrageStrategy::new();

        group.bench_function("strategy_detect", |b| {
            b.iter(|| {
                let bundle = arbitrage_core::strategies::MarketBundle::new();
                black_box(strategy.detect(black_box(&bundle)))
            })
        });

        group.finish();
    }

    pub fn benchmark_latency_detection(c: &mut Criterion) {
        let mut group = c.benchmark_group("detection/latency");
        let strategy = LatencyArbitrageStrategy::new();

        group.bench_function("strategy_detect", |b| {
            b.iter(|| {
                let bundle = arbitrage_core::strategies::MarketBundle::new();
                black_box(strategy.detect(black_box(&bundle)))
            })
        });

        group.finish();
    }

    pub fn benchmark_convergence_detection(c: &mut Criterion) {
        let mut group = c.benchmark_group("detection/convergence");
        let strategy = ConvergenceArbitrageStrategy::new();

        group.bench_function("strategy_detect", |b| {
            b.iter(|| {
                let bundle = arbitrage_core::strategies::MarketBundle::new();
                black_box(strategy.detect(black_box(&bundle)))
            })
        });

        group.finish();
    }
}

mod calculation_benchmarks {
    use super::*;

    pub fn benchmark_vwap_calculation(c: &mut Criterion) {
        let mut group = c.benchmark_group("calculations/vwap");
        let symbol = create_test_symbol();
        let orderbook = OrderBook::new(
            ExchangeId::OKX,
            symbol.clone(),
            (1..=100)
                .map(|i| {
                    OrderBookLevel::new(
                        Decimal::from(50000 - i),
                        Decimal::from(10) * Decimal::from(i),
                    )
                })
                .collect(),
            (1..=100)
                .map(|i| {
                    OrderBookLevel::new(
                        Decimal::from(50000 + i),
                        Decimal::from(10) * Decimal::from(i),
                    )
                })
                .collect(),
        );
        let quantity = Decimal::from(100);

        group.bench_function("vwap_buy_100_levels", |b| {
            b.iter(|| black_box(orderbook.vwap_buy(black_box(quantity))))
        });

        group.bench_function("vwap_sell_100_levels", |b| {
            b.iter(|| black_box(orderbook.vwap_sell(black_box(quantity))))
        });

        group.finish();
    }

    pub fn benchmark_profit_calculation(c: &mut Criterion) {
        let mut group = c.benchmark_group("calculations/profit");
        let buy_price = Decimal::from(50000);
        let sell_price = Decimal::from(50100);

        group.bench_function("calculate_profit_bps", |b| {
            b.iter(|| {
                let profit_ratio = (sell_price - buy_price) / buy_price;
                let bps = profit_ratio * Decimal::from(10000);
                black_box(bps.to_i32())
            })
        });

        group.finish();
    }

    pub fn benchmark_fee_calculation(c: &mut Criterion) {
        let mut group = c.benchmark_group("calculations/fees");
        let fee_schedule = create_test_fee_schedule(ExchangeId::OKX);
        let quantity = Decimal::from(1);
        let price = Decimal::from(50000);

        group.bench_function("calculate_taker_fee", |b| {
            b.iter(|| {
                let notional = price * quantity;
                let fee = notional * (fee_schedule.taker_fee / Decimal::from(100000));
                black_box(fee)
            })
        });

        group.bench_function("calculate_maker_fee", |b| {
            b.iter(|| {
                let notional = price * quantity;
                let fee = notional * (fee_schedule.maker_fee / Decimal::from(100000));
                black_box(fee)
            })
        });

        group.finish();
    }

    pub fn benchmark_spread_calculation(c: &mut Criterion) {
        let mut group = c.benchmark_group("calculations/spread");
        let symbol = create_test_symbol();
        let orderbook = create_test_orderbook(
            ExchangeId::OKX,
            symbol.clone(),
            Decimal::from(50000),
            Decimal::from(50010),
        );

        group.bench_function("calculate_spread", |b| {
            b.iter(|| {
                let spread = orderbook.spread();
                black_box(spread)
            })
        });

        group.bench_function("calculate_mid_price", |b| {
            b.iter(|| {
                let mid = orderbook.mid_price();
                black_box(mid)
            })
        });

        group.finish();
    }
}

mod scalability_benchmarks {
    use super::*;

    pub fn benchmark_100_symbols(c: &mut Criterion) {
        let mut group = c.benchmark_group("scalability/100_symbols");
        let strategy = CexArbitrageStrategy::new();
        let bundle = arbitrage_core::strategies::MarketBundle::new();

        group.bench_function("detect_100_symbols", |b| {
            b.iter(|| {
                for _ in 0..100 {
                    black_box(strategy.detect(black_box(&bundle)));
                }
            })
        });

        group.finish();
    }

    pub fn benchmark_1000_symbols(c: &mut Criterion) {
        let mut group = c.benchmark_group("scalability/1000_symbols");
        let strategy = CexArbitrageStrategy::new();
        let bundle = arbitrage_core::strategies::MarketBundle::new();

        group.bench_function("detect_1000_symbols", |b| {
            b.iter(|| {
                for _ in 0..1000 {
                    black_box(strategy.detect(black_box(&bundle)));
                }
            })
        });

        group.finish();
    }

    pub fn benchmark_10_exchanges(c: &mut Criterion) {
        let mut group = c.benchmark_group("scalability/10_exchanges");
        let exchanges: Vec<ExchangeId> = ALL_EXCHANGES.iter().take(10).copied().collect();
        let base_price = Decimal::from(50000);

        group.bench_function("compare_10_exchanges", |b| {
            b.iter(|| {
                let mut opportunities = 0;
                for i in 0..exchanges.len() {
                    for j in (i + 1)..exchanges.len() {
                        let _bid = base_price - Decimal::from(i);
                        let _ask = base_price + Decimal::from(j);
                        opportunities += 1;
                    }
                }
                black_box(opportunities)
            })
        });

        group.finish();
    }
}

mod memory_benchmarks {
    use super::*;
    use std::collections::HashMap;

    pub fn benchmark_order_book_allocation(c: &mut Criterion) {
        let mut group = c.benchmark_group("memory/orderbook");

        group.bench_function("allocate_1000_orderbooks", |b| {
            b.iter(|| {
                let mut orderbooks = Vec::new();
                for i in 0..1000 {
                    let orderbook = OrderBook::new(
                        ALL_EXCHANGES[i % ALL_EXCHANGES.len()],
                        Symbol::new(format!("SYM{}", i), "USDT"),
                        (1..=10)
                            .map(|j| {
                                OrderBookLevel::new(Decimal::from(50000 + j), Decimal::from(j))
                            })
                            .collect(),
                        (1..=10)
                            .map(|j| {
                                OrderBookLevel::new(Decimal::from(50000 - j), Decimal::from(j))
                            })
                            .collect(),
                    );
                    orderbooks.push(orderbook);
                }
                black_box(orderbooks)
            })
        });

        group.finish();
    }

    pub fn benchmark_signal_creation(c: &mut Criterion) {
        let mut group = c.benchmark_group("memory/signals");

        group.bench_function("create_1000_signals", |b| {
            b.iter(|| {
                let mut signals = Vec::new();
                for i in 0..1000 {
                    let signal = Signal::new(
                        Symbol::new(format!("BTC{}", i), "USDT"),
                        ExchangeId::OKX,
                        ExchangeId::ByBit,
                        Decimal::from(50000 + i),
                        Decimal::from(50010 + i),
                        chrono::Utc::now(),
                    );
                    signals.push(signal);
                }
                black_box(signals)
            })
        });

        group.finish();
    }

    pub fn benchmark_market_bundle_build(c: &mut Criterion) {
        let mut group = c.benchmark_group("memory/market_bundle");

        group.bench_function("build_100_symbol_bundles", |b| {
            b.iter(|| {
                let mut bundles = Vec::new();
                for i in 0..100 {
                    let mut orderbooks = HashMap::new();
                    for (idx, &exchange) in ALL_EXCHANGES.iter().enumerate().take(6) {
                        orderbooks.insert(
                            exchange,
                            Arc::new(OrderBook::new(
                                exchange,
                                Symbol::new(format!("SYM{}", i), "USDT"),
                                vec![OrderBookLevel::new(
                                    Decimal::from(50000 + idx),
                                    Decimal::from(10),
                                )],
                                vec![OrderBookLevel::new(
                                    Decimal::from(50010 + idx),
                                    Decimal::from(10),
                                )],
                            )),
                        );
                    }
                    bundles.push(orderbooks);
                }
                black_box(bundles)
            })
        });

        group.finish();
    }
}

mod strategy_benchmarks {
    use super::*;

    pub fn benchmark_cross_exchange_arbitrage(c: &mut Criterion) {
        let mut group = c.benchmark_group("strategies/cross_exchange");
        let strategy = CrossExchangeArbitrageStrategy::new();
        let bundle = arbitrage_core::strategies::MarketBundle::new();

        group.bench_function("strategy_detect", |b| {
            b.iter(|| black_box(strategy.detect(black_box(&bundle))))
        });

        group.finish();
    }

    pub fn benchmark_stablecoin_arbitrage(c: &mut Criterion) {
        let mut group = c.benchmark_group("strategies/stablecoin");
        let strategy = StablecoinArbitrageStrategy::new();
        let bundle = arbitrage_core::strategies::MarketBundle::new();

        group.bench_function("strategy_detect", |b| {
            b.iter(|| black_box(strategy.detect(black_box(&bundle))))
        });

        group.finish();
    }

    pub fn benchmark_spot_perp_arbitrage(c: &mut Criterion) {
        let mut group = c.benchmark_group("strategies/spot_perp");
        let strategy = SpotPerpArbitrageStrategy::new();
        let bundle = arbitrage_core::strategies::MarketBundle::new();

        group.bench_function("strategy_detect", |b| {
            b.iter(|| black_box(strategy.detect(black_box(&bundle))))
        });

        group.finish();
    }

    pub fn benchmark_new_listing_arbitrage(c: &mut Criterion) {
        let mut group = c.benchmark_group("strategies/new_listing");
        let strategy = NewListingArbitrageStrategy::new();
        let bundle = arbitrage_core::strategies::MarketBundle::new();

        group.bench_function("strategy_detect", |b| {
            b.iter(|| black_box(strategy.detect(black_box(&bundle))))
        });

        group.finish();
    }

    pub fn benchmark_hedged_funding(c: &mut Criterion) {
        let mut group = c.benchmark_group("strategies/hedged_funding");
        let strategy = HedgedFundingStrategy::new();
        let bundle = arbitrage_core::strategies::MarketBundle::new();

        group.bench_function("strategy_detect", |b| {
            b.iter(|| black_box(strategy.detect(black_box(&bundle))))
        });

        group.finish();
    }

    pub fn benchmark_spread_capture(c: &mut Criterion) {
        let mut group = c.benchmark_group("strategies/spread_capture");
        let strategy = SpreadCaptureStrategy::new();
        let bundle = arbitrage_core::strategies::MarketBundle::new();

        group.bench_function("strategy_detect", |b| {
            b.iter(|| black_box(strategy.detect(black_box(&bundle))))
        });

        group.finish();
    }

    pub fn benchmark_strategy_registry(c: &mut Criterion) {
        let mut group = c.benchmark_group("strategies/registry");

        group.bench_function("create_registry", |b| {
            b.iter(|| {
                let registry = StrategyRegistry::new();
                black_box(registry)
            })
        });

        group.bench_function("register_all_strategies", |b| {
            b.iter(|| {
                let mut registry = StrategyRegistry::new();
                registry.register(Arc::new(CexArbitrageStrategy::new()));
                registry.register(Arc::new(FundingRateArbitrageStrategy::new()));
                registry.register(Arc::new(StablecoinArbitrageStrategy::new()));
                registry.register(Arc::new(CrossExchangeArbitrageStrategy::new()));
                registry.register(Arc::new(NewListingArbitrageStrategy::new()));
                registry.register(Arc::new(SpotPerpArbitrageStrategy::new()));
                registry.register(Arc::new(ConvergenceArbitrageStrategy::new()));
                registry.register(Arc::new(HedgedFundingStrategy::new()));
                registry.register(Arc::new(LatencyArbitrageStrategy::new()));
                registry.register(Arc::new(SpreadCaptureStrategy::new()));
                black_box(registry)
            })
        });

        group.finish();
    }
}

mod types_benchmarks {
    use super::*;

    pub fn benchmark_symbol_parsing(c: &mut Criterion) {
        let mut group = c.benchmark_group("types/symbol");

        group.bench_function("symbol_from_pair", |b| {
            b.iter(|| black_box(Symbol::from_pair("BTC/USDT")))
        });

        group.bench_function("symbol_to_pair", |b| {
            b.iter(|| {
                let symbol = Symbol::new("BTC", "USDT");
                black_box(symbol.to_pair())
            })
        });

        group.finish();
    }

    pub fn benchmark_order_creation(c: &mut Criterion) {
        let mut group = c.benchmark_group("types/order");

        group.bench_function("create_order", |b| {
            b.iter(|| {
                black_box(Order::new(
                    black_box(ExchangeId::OKX),
                    black_box(Symbol::new("BTC", "USDT")),
                    black_box(Side::Buy),
                    black_box(OrderType::Market),
                    black_box(Decimal::from(1)),
                    black_box(None),
                ))
            })
        });

        group.finish();
    }

    pub fn benchmark_orderbook_validation(c: &mut Criterion) {
        let mut group = c.benchmark_group("types/orderbook_validation");

        group.bench_function("validate_valid_orderbook", |b| {
            b.iter(|| {
                let orderbook = OrderBook::new(
                    ExchangeId::OKX,
                    Symbol::new("BTC", "USDT"),
                    vec![
                        OrderBookLevel::new(Decimal::from(50000), Decimal::from(1)),
                        OrderBookLevel::new(Decimal::from(49999), Decimal::from(2)),
                    ],
                    vec![
                        OrderBookLevel::new(Decimal::from(50001), Decimal::from(1)),
                        OrderBookLevel::new(Decimal::from(50002), Decimal::from(2)),
                    ],
                );
                black_box(orderbook.is_valid())
            })
        });

        group.finish();
    }

    pub fn benchmark_exchange_id_parsing(c: &mut Criterion) {
        let mut group = c.benchmark_group("types/exchange_id");

        group.bench_function("parse_exchange_id", |b| {
            b.iter(|| black_box("okx".parse::<ExchangeId>()))
        });

        group.finish();
    }

    pub fn benchmark_decimal_to_bps(c: &mut Criterion) {
        let mut group = c.benchmark_group("types/to_bps");

        group.bench_function("decimal_to_bps", |b| {
            b.iter(|| {
                let value = Decimal::from(100) / Decimal::from(10000);
                black_box(value.to_bps())
            })
        });

        group.finish();
    }
}

mod fee_benchmarks {
    use super::*;

    pub fn benchmark_fee_schedules(c: &mut Criterion) {
        let mut group = c.benchmark_group("fees/schedules");

        group.bench_function("create_all_fee_schedules", |b| {
            b.iter(|| {
                let mut schedules = Vec::new();
                for &exchange in &ALL_EXCHANGES {
                    schedules.push(create_test_fee_schedule(exchange));
                }
                black_box(schedules)
            })
        });

        group.finish();
    }

    pub fn benchmark_net_profit_calculation(c: &mut Criterion) {
        let mut group = c.benchmark_group("fees/net_profit");

        group.bench_function("calculate_net_profit", |b| {
            b.iter(|| {
                let gross_profit = Decimal::from(100);
                let fee_a = create_test_fee_schedule(ExchangeId::OKX);
                let fee_b = create_test_fee_schedule(ExchangeId::ByBit);
                let quantity = Decimal::from(1);
                let avg_price = Decimal::from(50000);
                let total_fees = (avg_price * quantity)
                    * ((fee_a.taker_fee + fee_b.taker_fee) / Decimal::from(100000));
                let net_profit = gross_profit - total_fees;
                black_box(net_profit)
            })
        });

        group.finish();
    }
}

mod utilities_benchmarks {
    use super::*;

    pub fn benchmark_best_price_selection(c: &mut Criterion) {
        let mut group = c.benchmark_group("utilities/best_price");

        group.bench_function("select_best_bid_across_exchanges", |b| {
            b.iter(|| {
                let prices = [
                    (ExchangeId::OKX, Decimal::from(50001)),
                    (ExchangeId::ByBit, Decimal::from(50000)),
                    (ExchangeId::MEXC, Decimal::from(50002)),
                    (ExchangeId::GateIo, Decimal::from(49999)),
                ];
                let best_price = prices
                    .iter()
                    .map(|(_, p)| *p)
                    .min()
                    .unwrap_or(Decimal::ZERO);
                black_box(best_price)
            })
        });

        group.bench_function("select_best_ask_across_exchanges", |b| {
            b.iter(|| {
                let prices = [
                    (ExchangeId::OKX, Decimal::from(50010)),
                    (ExchangeId::ByBit, Decimal::from(50011)),
                    (ExchangeId::MEXC, Decimal::from(50009)),
                    (ExchangeId::GateIo, Decimal::from(50012)),
                ];
                let best_price = prices
                    .iter()
                    .map(|(_, p)| *p)
                    .min()
                    .unwrap_or(Decimal::ZERO);
                black_box(best_price)
            })
        });

        group.finish();
    }

    pub fn benchmark_opportunity_ranking(c: &mut Criterion) {
        let mut group = c.benchmark_group("utilities/ranking");

        group.bench_function("rank_opportunities", |b| {
            b.iter(|| {
                let mut opportunities: Vec<(Signal, Decimal)> = (0..100)
                    .map(|i| {
                        let signal = Signal::new(
                            Symbol::new(format!("SYM{}", i), "USDT"),
                            ExchangeId::OKX,
                            ExchangeId::ByBit,
                            Decimal::from(50000 + i),
                            Decimal::from(50010 + i),
                            chrono::Utc::now(),
                        );
                        let profit = Decimal::from(i % 100);
                        (signal, profit)
                    })
                    .collect();
                opportunities.sort_by_key(|(_, profit)| *profit);
                black_box(opportunities)
            })
        });

        group.finish();
    }

    pub fn benchmark_liquidity_calculation(c: &mut Criterion) {
        let mut group = c.benchmark_group("utilities/liquidity");

        group.bench_function("calculate_available_liquidity", |b| {
            b.iter(|| {
                let orderbook = OrderBook::new(
                    ExchangeId::OKX,
                    Symbol::new("BTC", "USDT"),
                    (1..=50)
                        .map(|i| {
                            OrderBookLevel::new(
                                Decimal::from(50000 - i),
                                Decimal::from(10) * Decimal::from(i),
                            )
                        })
                        .collect(),
                    (1..=50)
                        .map(|i| {
                            OrderBookLevel::new(
                                Decimal::from(50000 + i),
                                Decimal::from(10) * Decimal::from(i),
                            )
                        })
                        .collect(),
                );
                black_box(orderbook.liquidity_at_price(Decimal::from(50050), true))
            })
        });

        group.finish();
    }
}

mod additional_core_benchmarks {
    use super::*;

    pub fn benchmark_decimal_addition(c: &mut Criterion) {
        let mut group = c.benchmark_group("core/decimal_add");
        let val1 = Decimal::from(50000);
        let val2 = Decimal::from(100);
        group.bench_function("add_50000_100", |b| b.iter(|| black_box(val1 + val2)));
        group.finish();
    }

    pub fn benchmark_decimal_subtraction(c: &mut Criterion) {
        let mut group = c.benchmark_group("core/decimal_sub");
        let val1 = Decimal::from(50000);
        let val2 = Decimal::from(100);
        group.bench_function("sub_50000_100", |b| b.iter(|| black_box(val1 - val2)));
        group.finish();
    }

    pub fn benchmark_decimal_multiplication(c: &mut Criterion) {
        let mut group = c.benchmark_group("core/decimal_mul");
        let val1 = Decimal::from(50000);
        let val2 = Decimal::from(2);
        group.bench_function("mul_50000_2", |b| b.iter(|| black_box(val1 * val2)));
        group.finish();
    }

    pub fn benchmark_decimal_division(c: &mut Criterion) {
        let mut group = c.benchmark_group("core/decimal_div");
        let val1 = Decimal::from(50000);
        let val2 = Decimal::from(2);
        group.bench_function("div_50000_2", |b| b.iter(|| black_box(val1 / val2)));
        group.finish();
    }

    pub fn benchmark_format_exchange(c: &mut Criterion) {
        let mut group = c.benchmark_group("core/format");
        group.bench_function("format_okx", |b| {
            b.iter(|| black_box(format!("{}", ExchangeId::OKX)))
        });
        group.finish();
    }

    pub fn benchmark_symbol_pair(c: &mut Criterion) {
        let mut group = c.benchmark_group("core/symbol_pair");
        group.bench_function("btc_usdt_pair", |b| {
            b.iter(|| {
                let s = Symbol::new("BTC", "USDT");
                black_box(s.to_pair())
            })
        });
        group.finish();
    }

    pub fn benchmark_best_bid(c: &mut Criterion) {
        let mut group = c.benchmark_group("core/best_bid");
        let orderbook = create_test_orderbook(
            ExchangeId::OKX,
            Symbol::new("BTC", "USDT"),
            Decimal::from(50000),
            Decimal::from(50010),
        );
        group.bench_function("get_best_bid", |b| {
            b.iter(|| black_box(orderbook.best_bid().cloned()))
        });
        group.finish();
    }

    pub fn benchmark_best_ask(c: &mut Criterion) {
        let mut group = c.benchmark_group("core/best_ask");
        let orderbook = create_test_orderbook(
            ExchangeId::OKX,
            Symbol::new("BTC", "USDT"),
            Decimal::from(50000),
            Decimal::from(50010),
        );
        group.bench_function("get_best_ask", |b| {
            b.iter(|| black_box(orderbook.best_ask().cloned()))
        });
        group.finish();
    }

    pub fn benchmark_create_buy_order(c: &mut Criterion) {
        let mut group = c.benchmark_group("core/create_order");
        group.bench_function("buy_market_order", |b| {
            b.iter(|| {
                black_box(Order::new(
                    ExchangeId::OKX,
                    Symbol::new("BTC", "USDT"),
                    Side::Buy,
                    OrderType::Market,
                    Decimal::from(1),
                    None,
                ))
            })
        });
        group.finish();
    }

    pub fn benchmark_create_sell_order(c: &mut Criterion) {
        let mut group = c.benchmark_group("core/sell_order");
        group.bench_function("sell_limit_order", |b| {
            b.iter(|| {
                black_box(Order::new(
                    ExchangeId::OKX,
                    Symbol::new("BTC", "USDT"),
                    Side::Sell,
                    OrderType::Limit,
                    Decimal::from(1),
                    Some(Decimal::from(50000)),
                ))
            })
        });
        group.finish();
    }

    pub fn benchmark_create_signal(c: &mut Criterion) {
        let mut group = c.benchmark_group("core/create_signal");
        group.bench_function("new_signal", |b| {
            b.iter(|| {
                black_box(Signal::new(
                    Symbol::new("BTC", "USDT"),
                    ExchangeId::OKX,
                    ExchangeId::ByBit,
                    Decimal::from(50000),
                    Decimal::from(50010),
                    chrono::Utc::now(),
                ))
            })
        });
        group.finish();
    }

    pub fn benchmark_signal_expired(c: &mut Criterion) {
        let mut group = c.benchmark_group("core/signal_expired");
        let signal = Signal::new(
            Symbol::new("BTC", "USDT"),
            ExchangeId::OKX,
            ExchangeId::ByBit,
            Decimal::from(50000),
            Decimal::from(50010),
            chrono::Utc::now(),
        );
        group.bench_function("check_expired", |b| {
            b.iter(|| black_box(signal.is_expired()))
        });
        group.finish();
    }

    pub fn benchmark_empty_bundle(c: &mut Criterion) {
        let mut group = c.benchmark_group("core/empty_bundle");
        group.bench_function("new_market_bundle", |b| {
            b.iter(|| black_box(arbitrage_core::strategies::MarketBundle::new()))
        });
        group.finish();
    }

    pub fn benchmark_bundle_has_data(c: &mut Criterion) {
        let mut group = c.benchmark_group("core/bundle_has_data");
        let bundle = arbitrage_core::strategies::MarketBundle::new();
        group.bench_function("has_data_check", |b| {
            b.iter(|| black_box(bundle.has_data(ExchangeId::OKX, &Symbol::new("BTC", "USDT"))))
        });
        group.finish();
    }

    pub fn benchmark_strategy_id(c: &mut Criterion) {
        let mut group = c.benchmark_group("core/strategy_id");
        let strategy = CexArbitrageStrategy::new();
        group.bench_function("get_strategy_id", |b| b.iter(|| black_box(strategy.id())));
        group.finish();
    }

    pub fn benchmark_strategy_name(c: &mut Criterion) {
        let mut group = c.benchmark_group("core/strategy_name");
        let strategy = CexArbitrageStrategy::new();
        group.bench_function("get_strategy_name", |b| {
            b.iter(|| black_box(strategy.name()))
        });
        group.finish();
    }

    pub fn benchmark_filter_context(c: &mut Criterion) {
        let mut group = c.benchmark_group("core/filter_context");
        group.bench_function("new_filter_context", |b| {
            b.iter(|| black_box(arbitrage_core::strategies::FilterContext::new(10)))
        });
        group.finish();
    }

    pub fn benchmark_exchange_allowed(c: &mut Criterion) {
        let mut group = c.benchmark_group("core/exchange_allowed");
        let ctx = arbitrage_core::strategies::FilterContext::new(10);
        group.bench_function("check_exchange", |b| {
            b.iter(|| black_box(ctx.is_exchange_allowed(ExchangeId::OKX)))
        });
        group.finish();
    }

    pub fn benchmark_raw_signal(c: &mut Criterion) {
        let mut group = c.benchmark_group("core/raw_signal");
        group.bench_function("new_raw_signal", |b| {
            b.iter(|| {
                black_box(arbitrage_core::strategies::RawSignal::new(
                    "test",
                    Symbol::new("BTC", "USDT"),
                ))
            })
        });
        group.finish();
    }

    pub fn benchmark_trade_leg(c: &mut Criterion) {
        let mut group = c.benchmark_group("core/trade_leg");
        group.bench_function("new_trade_leg", |b| {
            b.iter(|| {
                black_box(arbitrage_core::strategies::TradeLeg::new(
                    ExchangeId::OKX,
                    Symbol::new("BTC", "USDT"),
                    Side::Buy,
                    Decimal::from(50000),
                    Decimal::from(1),
                ))
            })
        });
        group.finish();
    }

    pub fn benchmark_confidence_factors(c: &mut Criterion) {
        let mut group = c.benchmark_group("core/confidence");
        group.bench_function("default_confidence", |b| {
            b.iter(|| black_box(arbitrage_core::strategies::ConfidenceFactors::default()))
        });
        group.finish();
    }

    pub fn benchmark_vec_push(c: &mut Criterion) {
        let mut group = c.benchmark_group("core/vec_push");
        group.bench_function("push_1000", |b| {
            b.iter(|| {
                let mut v = Vec::new();
                for i in 0..1000 {
                    v.push(i);
                }
                black_box(v)
            })
        });
        group.finish();
    }

    pub fn benchmark_vec_iter_sum(c: &mut Criterion) {
        let mut group = c.benchmark_group("core/vec_iter");
        group.bench_function("iter_sum_1000", |b| {
            b.iter(|| {
                let v: Vec<i32> = (0..1000).collect();
                black_box(v.iter().sum::<i32>())
            })
        });
        group.finish();
    }

    pub fn benchmark_hashmap_insert(c: &mut Criterion) {
        let mut group = c.benchmark_group("core/hashmap");
        group.bench_function("insert_100", |b| {
            b.iter(|| {
                let mut m = std::collections::HashMap::new();
                for i in 0..100 {
                    m.insert(i, i * 2);
                }
                black_box(m)
            })
        });
        group.finish();
    }

    pub fn benchmark_sort_1000(c: &mut Criterion) {
        let mut group = c.benchmark_group("core/sort");
        group.bench_function("sort_1000", |b| {
            b.iter(|| {
                let mut v: Vec<i32> = (0..1000).map(|_| rand::random()).collect();
                v.sort();
                black_box(v)
            })
        });
        group.finish();
    }

    pub fn benchmark_json_serialize(c: &mut Criterion) {
        let mut group = c.benchmark_group("core/json");
        let value = serde_json::json!({"price": 50000, "symbol": "BTC/USDT"});
        group.bench_function("to_string", |b| {
            b.iter(|| black_box(serde_json::to_string(&value)))
        });
        group.finish();
    }

    pub fn benchmark_utc_now(c: &mut Criterion) {
        let mut group = c.benchmark_group("core/time");
        group.bench_function("utc_now", |b| b.iter(|| black_box(chrono::Utc::now())));
        group.finish();
    }

    pub fn benchmark_uuid_gen(c: &mut Criterion) {
        let mut group = c.benchmark_group("core/uuid");
        group.bench_function("new_v4", |b| b.iter(|| black_box(uuid::Uuid::new_v4())));
        group.finish();
    }

    pub fn benchmark_option_unwrap(c: &mut Criterion) {
        let mut group = c.benchmark_group("core/option");
        let opt: Option<i32> = None;
        group.bench_function("unwrap_or", |b| b.iter(|| black_box(opt.unwrap_or(42))));
        group.finish();
    }

    pub fn benchmark_result_map(c: &mut Criterion) {
        let mut group = c.benchmark_group("core/result");
        let res: Result<i32, &str> = Ok(10);
        group.bench_function("map", |b| b.iter(|| black_box(res.map(|x| x * 2))));
        group.finish();
    }
}

mod additional_strategy_benchmarks {
    use super::*;

    pub fn benchmark_convergence_detect(c: &mut Criterion) {
        let mut group = c.benchmark_group("strategies/convergence_detect");
        let strategy = ConvergenceArbitrageStrategy::new();
        group.bench_function("detect", |b| {
            b.iter(|| {
                let bundle = arbitrage_core::strategies::MarketBundle::new();
                black_box(strategy.detect(black_box(&bundle)))
            })
        });
        group.finish();
    }

    pub fn benchmark_hedged_funding_detect(c: &mut Criterion) {
        let mut group = c.benchmark_group("strategies/hedged_funding_detect");
        let strategy = HedgedFundingStrategy::new();
        group.bench_function("detect", |b| {
            b.iter(|| {
                let bundle = arbitrage_core::strategies::MarketBundle::new();
                black_box(strategy.detect(black_box(&bundle)))
            })
        });
        group.finish();
    }

    pub fn benchmark_latency_detect(c: &mut Criterion) {
        let mut group = c.benchmark_group("strategies/latency_detect");
        let strategy = LatencyArbitrageStrategy::new();
        group.bench_function("detect", |b| {
            b.iter(|| {
                let bundle = arbitrage_core::strategies::MarketBundle::new();
                black_box(strategy.detect(black_box(&bundle)))
            })
        });
        group.finish();
    }

    pub fn benchmark_funding_rate_detect(c: &mut Criterion) {
        let mut group = c.benchmark_group("strategies/funding_rate_detect");
        let strategy = FundingRateArbitrageStrategy::new();
        group.bench_function("detect", |b| {
            b.iter(|| {
                let bundle = arbitrage_core::strategies::MarketBundle::new();
                black_box(strategy.detect(black_box(&bundle)))
            })
        });
        group.finish();
    }
}

mod additional_calculation_benchmarks {
    use super::*;

    pub fn benchmark_spread_percentage(c: &mut Criterion) {
        let mut group = c.benchmark_group("calculations/spread_percentage");
        let bid = Decimal::from(50000);
        let ask = Decimal::from(50050);
        group.bench_function("spread_pct", |b| {
            b.iter(|| {
                let spread = (ask - bid) / bid * Decimal::from(100);
                black_box(spread)
            })
        });
        group.finish();
    }

    pub fn benchmark_profit_loss(c: &mut Criterion) {
        let mut group = c.benchmark_group("calculations/pnl");
        let entry_price = Decimal::from(50000);
        let exit_price = Decimal::from(50500);
        let quantity = Decimal::from(1);
        group.bench_function("calculate_pnl", |b| {
            b.iter(|| {
                let pnl = (exit_price - entry_price) * quantity;
                black_box(pnl)
            })
        });
        group.finish();
    }

    pub fn benchmark_notional_value(c: &mut Criterion) {
        let mut group = c.benchmark_group("calculations/notional");
        let price = Decimal::from(50000);
        let quantity = Decimal::from(10);
        group.bench_function("notional", |b| {
            b.iter(|| {
                let notional = price * quantity;
                black_box(notional)
            })
        });
        group.finish();
    }

    pub fn benchmark_order_cost(c: &mut Criterion) {
        let mut group = c.benchmark_group("calculations/order_cost");
        let price = Decimal::from(50000);
        let quantity = Decimal::from(1);
        let fee_rate = Decimal::from(10) / Decimal::from(100000);
        group.bench_function("order_cost", |b| {
            b.iter(|| {
                let notional = price * quantity;
                let fee = notional * fee_rate;
                let cost = notional + fee;
                black_box(cost)
            })
        });
        group.finish();
    }

    pub fn benchmark_price_impact(c: &mut Criterion) {
        let mut group = c.benchmark_group("calculations/impact");
        let base_price = Decimal::from(50000);
        let order_quantity = Decimal::from(10);
        let total_liquidity = Decimal::from(1000);
        group.bench_function("price_impact", |b| {
            b.iter(|| {
                let impact = (order_quantity / total_liquidity) * Decimal::from(100);
                let adjusted_price = base_price * (Decimal::ONE + impact / Decimal::from(10000));
                black_box(adjusted_price)
            })
        });
        group.finish();
    }

    pub fn benchmark_arbitrage_profit(c: &mut Criterion) {
        let mut group = c.benchmark_group("calculations/arbitrage");
        let buy_price = Decimal::from(50000);
        let sell_price = Decimal::from(50100);
        let quantity = Decimal::from(1);
        let fee_rate = Decimal::from(10) / Decimal::from(100000);
        group.bench_function("gross_profit", |b| {
            b.iter(|| {
                let buy_cost = buy_price * quantity * (Decimal::ONE + fee_rate);
                let sell_value = sell_price * quantity * (Decimal::ONE - fee_rate);
                let profit = sell_value - buy_cost;
                black_box(profit)
            })
        });
        group.finish();
    }

    pub fn benchmark_annualized_rate(c: &mut Criterion) {
        let mut group = c.benchmark_group("calculations/annualized");
        let funding_rate = Decimal::from(1) / Decimal::from(10000);
        let hours_per_day = Decimal::from(24);
        group.bench_function("annualized", |b| {
            b.iter(|| {
                let annualized = funding_rate * hours_per_day * Decimal::from(365);
                black_box(annualized)
            })
        });
        group.finish();
    }

    pub fn benchmark_weighted_average_price(c: &mut Criterion) {
        let mut group = c.benchmark_group("calculations/wap");
        let prices = vec![
            (Decimal::from(50000), Decimal::from(10)),
            (Decimal::from(49999), Decimal::from(20)),
            (Decimal::from(49998), Decimal::from(30)),
        ];
        group.bench_function("calculate_wap", |b| {
            b.iter(|| {
                let total_value: Decimal = prices.iter().map(|(p, q)| *p * *q).sum();
                let total_qty: Decimal = prices.iter().map(|(_, q)| *q).sum();
                let wap = if total_qty > Decimal::ZERO {
                    total_value / total_qty
                } else {
                    Decimal::ZERO
                };
                black_box(wap)
            })
        });
        group.finish();
    }
}

mod orderbook_benchmarks {
    use super::*;

    pub fn benchmark_best_bid_ask(c: &mut Criterion) {
        let mut group = c.benchmark_group("orderbook/best_price");
        let orderbook = create_test_orderbook(
            ExchangeId::OKX,
            Symbol::new("BTC", "USDT"),
            Decimal::from(50000),
            Decimal::from(50010),
        );
        group.bench_function("best_bid_ask", |b| {
            b.iter(|| {
                let best_bid = orderbook.best_bid().map(|l| l.price);
                let best_ask = orderbook.best_ask().map(|l| l.price);
                black_box((best_bid, best_ask))
            })
        });
        group.finish();
    }

    pub fn benchmark_orderbook_spread(c: &mut Criterion) {
        let mut group = c.benchmark_group("orderbook/spread");
        let orderbook = OrderBook::new(
            ExchangeId::OKX,
            Symbol::new("BTC", "USDT"),
            vec![OrderBookLevel::new(Decimal::from(50000), Decimal::from(1))],
            vec![OrderBookLevel::new(Decimal::from(50001), Decimal::from(1))],
        );
        group.bench_function("spread", |b| b.iter(|| black_box(orderbook.spread())));
        group.finish();
    }

    pub fn benchmark_orderbook_mid_price(c: &mut Criterion) {
        let mut group = c.benchmark_group("orderbook/mid_price");
        let orderbook = OrderBook::new(
            ExchangeId::OKX,
            Symbol::new("BTC", "USDT"),
            vec![OrderBookLevel::new(Decimal::from(49999), Decimal::from(1))],
            vec![OrderBookLevel::new(Decimal::from(50001), Decimal::from(1))],
        );
        group.bench_function("mid_price", |b| b.iter(|| black_box(orderbook.mid_price())));
        group.finish();
    }

    pub fn benchmark_orderbook_depth(c: &mut Criterion) {
        let mut group = c.benchmark_group("orderbook/depth");
        let orderbook = OrderBook::new(
            ExchangeId::OKX,
            Symbol::new("BTC", "USDT"),
            (1..=50)
                .map(|i| OrderBookLevel::new(Decimal::from(50000 - i), Decimal::from(i)))
                .collect(),
            (1..=50)
                .map(|i| OrderBookLevel::new(Decimal::from(50000 + i), Decimal::from(i)))
                .collect(),
        );
        group.bench_function("bid_depth", |b| b.iter(|| black_box(orderbook.bid_depth())));
        group.finish();
    }

    pub fn benchmark_liquidity_at_price(c: &mut Criterion) {
        let mut group = c.benchmark_group("orderbook/liquidity");
        let orderbook = OrderBook::new(
            ExchangeId::OKX,
            Symbol::new("BTC", "USDT"),
            (1..=20)
                .map(|i| OrderBookLevel::new(Decimal::from(50000 - i), Decimal::from(i * 10)))
                .collect(),
            (1..=20)
                .map(|i| OrderBookLevel::new(Decimal::from(50000 + i), Decimal::from(i * 10)))
                .collect(),
        );
        group.bench_function("liquidity_bid", |b| {
            b.iter(|| black_box(orderbook.liquidity_at_price(Decimal::from(49990), true)))
        });
        group.finish();
    }

    pub fn benchmark_vwap_execution(c: &mut Criterion) {
        let mut group = c.benchmark_group("orderbook/vwap");
        let orderbook = OrderBook::new(
            ExchangeId::OKX,
            Symbol::new("BTC", "USDT"),
            (1..=100)
                .map(|i| {
                    OrderBookLevel::new(
                        Decimal::from(50000 - i),
                        Decimal::from(10) * Decimal::from(i),
                    )
                })
                .collect(),
            (1..=100)
                .map(|i| {
                    OrderBookLevel::new(
                        Decimal::from(50000 + i),
                        Decimal::from(10) * Decimal::from(i),
                    )
                })
                .collect(),
        );
        let quantity = Decimal::from(100);
        group.bench_function("vwap_buy", |b| {
            b.iter(|| black_box(orderbook.vwap_buy(quantity)))
        });
        group.finish();
    }

    pub fn benchmark_orderbook_is_valid(c: &mut Criterion) {
        let mut group = c.benchmark_group("orderbook/validation");
        let orderbook = OrderBook::new(
            ExchangeId::OKX,
            Symbol::new("BTC", "USDT"),
            vec![OrderBookLevel::new(Decimal::from(49999), Decimal::from(1))],
            vec![OrderBookLevel::new(Decimal::from(50001), Decimal::from(1))],
        );
        group.bench_function("is_valid", |b| b.iter(|| black_box(orderbook.is_valid())));
        group.finish();
    }

    pub fn benchmark_orderbook_imbalance(c: &mut Criterion) {
        let mut group = c.benchmark_group("orderbook/imbalance");
        let orderbook = OrderBook::new(
            ExchangeId::OKX,
            Symbol::new("BTC", "USDT"),
            (1..=50)
                .map(|i| OrderBookLevel::new(Decimal::from(50000 - i), Decimal::from(i)))
                .collect(),
            (1..=50)
                .map(|i| OrderBookLevel::new(Decimal::from(50000 + i), Decimal::from(i)))
                .collect(),
        );
        group.bench_function("imbalance", |b| b.iter(|| black_box(orderbook.imbalance())));
        group.finish();
    }
}

mod signal_processing_benchmarks {
    use super::*;

    pub fn benchmark_signal_creation(c: &mut Criterion) {
        let mut group = c.benchmark_group("signal/create");
        group.bench_function("new_signal", |b| {
            b.iter(|| {
                black_box(Signal::new(
                    Symbol::new("BTC", "USDT"),
                    ExchangeId::OKX,
                    ExchangeId::ByBit,
                    Decimal::from(50000),
                    Decimal::from(50100),
                    chrono::Utc::now(),
                ))
            })
        });
        group.finish();
    }

    pub fn benchmark_signal_validation(c: &mut Criterion) {
        let mut group = c.benchmark_group("signal/validation");
        let signal = Signal::new(
            Symbol::new("BTC", "USDT"),
            ExchangeId::OKX,
            ExchangeId::ByBit,
            Decimal::from(50000),
            Decimal::from(50100),
            chrono::Utc::now(),
        );
        group.bench_function("is_valid", |b| b.iter(|| black_box(signal.is_valid())));
        group.finish();
    }

    pub fn benchmark_signal_expired(c: &mut Criterion) {
        let mut group = c.benchmark_group("signal/expired");
        let signal = Signal::new(
            Symbol::new("BTC", "USDT"),
            ExchangeId::OKX,
            ExchangeId::ByBit,
            Decimal::from(50000),
            Decimal::from(50100),
            chrono::Utc::now(),
        );
        group.bench_function("is_expired", |b| b.iter(|| black_box(signal.is_expired())));
        group.finish();
    }

    pub fn benchmark_signal_profitability(c: &mut Criterion) {
        let mut group = c.benchmark_group("signal/profitability");
        let signal = Signal::new(
            Symbol::new("BTC", "USDT"),
            ExchangeId::OKX,
            ExchangeId::ByBit,
            Decimal::from(50000),
            Decimal::from(50100),
            chrono::Utc::now(),
        );
        group.bench_function("profit_bps", |b| b.iter(|| black_box(signal.profit_bps())));
        group.finish();
    }

    pub fn benchmark_signals_sort(c: &mut Criterion) {
        let mut group = c.benchmark_group("signal/sort");
        let mut signals: Vec<Signal> = (0..100)
            .map(|i| {
                Signal::new(
                    Symbol::new(format!("SYM{}", i), "USDT"),
                    ExchangeId::OKX,
                    ExchangeId::ByBit,
                    Decimal::from(50000 + i % 100),
                    Decimal::from(50100 + i % 100),
                    chrono::Utc::now(),
                )
            })
            .collect();
        group.bench_function("sort_by_profit", |b| {
            b.iter(|| {
                signals.sort_by_key(|s| s.profit_bps());
                black_box(&signals)
            })
        });
        group.finish();
    }

    pub fn benchmark_signals_filter(c: &mut Criterion) {
        let mut group = c.benchmark_group("signal/filter");
        let signals: Vec<Signal> = (0..100)
            .map(|i| {
                Signal::new(
                    Symbol::new(format!("SYM{}", i), "USDT"),
                    ExchangeId::OKX,
                    ExchangeId::ByBit,
                    Decimal::from(50000),
                    Decimal::from(50000 + i as i32),
                    chrono::Utc::now(),
                )
            })
            .collect();
        group.bench_function("filter_profitable", |b| {
            b.iter(|| {
                let profitable: Vec<_> = signals.iter().filter(|s| s.profit_bps() > 10).collect();
                black_box(profitable)
            })
        });
        group.finish();
    }

    pub fn benchmark_signal_clone(c: &mut Criterion) {
        let signal = Signal::new(
            Symbol::new("BTC", "USDT"),
            ExchangeId::OKX,
            ExchangeId::ByBit,
            Decimal::from(50000),
            Decimal::from(50100),
            chrono::Utc::now(),
        );
        let mut group = c.benchmark_group("signal/clone");
        group.bench_function("clone", |b| b.iter(|| black_box(signal.clone())));
        group.finish();
    }

    pub fn benchmark_signal_equality(c: &mut Criterion) {
        let signal1 = Signal::new(
            Symbol::new("BTC", "USDT"),
            ExchangeId::OKX,
            ExchangeId::ByBit,
            Decimal::from(50000),
            Decimal::from(50100),
            chrono::Utc::now(),
        );
        let signal2 = signal1.clone();
        let mut group = c.benchmark_group("signal/equality");
        group.bench_function("eq", |b| b.iter(|| black_box(signal1 == signal2)));
        group.finish();
    }
}

mod market_data_benchmarks {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Arc;

    pub fn benchmark_ticker_creation(c: &mut Criterion) {
        let mut group = c.benchmark_group("market/ticker_create");
        group.bench_function("new_ticker", |b| {
            b.iter(|| {
                black_box(TickerData {
                    symbol: Symbol::new("BTC", "USDT"),
                    exchange: ExchangeId::OKX,
                    last_price: Decimal::from(50000),
                    bid_price: Decimal::from(49999),
                    ask_price: Decimal::from(50001),
                    volume_24h: Decimal::from(1000000),
                    price_change_24h: Decimal::from(100),
                    timestamp: chrono::Utc::now(),
                })
            })
        });
        group.finish();
    }

    pub fn benchmark_funding_rate_creation(c: &mut Criterion) {
        let mut group = c.benchmark_group("market/funding_create");
        group.bench_function("new_funding", |b| {
            b.iter(|| {
                black_box(FundingRate {
                    symbol: Symbol::new("BTC", "USDT"),
                    exchange: ExchangeId::OKX,
                    funding_rate: Decimal::from(1) / Decimal::from(10000),
                    predicted_rate: Some(Decimal::from(1) / Decimal::from(10000)),
                    funding_time: chrono::Utc::now(),
                    timestamp: chrono::Utc::now(),
                })
            })
        });
        group.finish();
    }

    pub fn benchmark_orderbook_snapshot(c: &mut Criterion) {
        let mut group = c.benchmark_group("market/orderbook_snapshot");
        let symbol = Symbol::new("BTC", "USDT");
        group.bench_function("snapshot_100_levels", |b| {
            b.iter(|| {
                let orderbook = OrderBook::new(
                    ExchangeId::OKX,
                    symbol.clone(),
                    (1..=100)
                        .map(|i| OrderBookLevel::new(Decimal::from(50000 - i), Decimal::from(i)))
                        .collect(),
                    (1..=100)
                        .map(|i| OrderBookLevel::new(Decimal::from(50000 + i), Decimal::from(i)))
                        .collect(),
                );
                black_box(orderbook)
            })
        });
        group.finish();
    }

    pub fn benchmark_market_bundle_construction(c: &mut Criterion) {
        let mut group = c.benchmark_group("market/bundle_build");
        group.bench_function("bundle_10_symbols", |b| {
            b.iter(|| {
                let mut bundle = arbitrage_core::strategies::MarketBundle::new();
                for i in 0..10 {
                    let symbol = Symbol::new(format!("SYM{}", i), "USDT");
                    let orderbook = Arc::new(OrderBook::new(
                        ALL_EXCHANGES[i % ALL_EXCHANGES.len()],
                        symbol.clone(),
                        vec![OrderBookLevel::new(
                            Decimal::from(50000 + i),
                            Decimal::from(10),
                        )],
                        vec![OrderBookLevel::new(
                            Decimal::from(50010 + i),
                            Decimal::from(10),
                        )],
                    ));
                    bundle.insert_orderbook(
                        ALL_EXCHANGES[i % ALL_EXCHANGES.len()],
                        symbol,
                        orderbook,
                    );
                }
                black_box(bundle)
            })
        });
        group.finish();
    }

    pub fn benchmark_price_comparison(c: &mut Criterion) {
        let mut group = c.benchmark_group("market/price_compare");
        let prices: Vec<(ExchangeId, Decimal)> = ALL_EXCHANGES
            .iter()
            .enumerate()
            .map(|(i, &e)| (e, Decimal::from(50000 + i as i32)))
            .collect();
        group.bench_function("find_min_max", |b| {
            b.iter(|| {
                let min_price = prices.iter().map(|(_, p)| *p).min();
                let max_price = prices.iter().map(|(_, p)| *p).max();
                black_box((min_price, max_price))
            })
        });
        group.finish();
    }

    pub fn benchmark_cross_exchange_spread(c: &mut Criterion) {
        let mut group = c.benchmark_group("market/cross_spread");
        let orderbooks: HashMap<ExchangeId, Arc<OrderBook>> = ALL_EXCHANGES
            .iter()
            .enumerate()
            .map(|(i, &e)| {
                let orderbook = Arc::new(OrderBook::new(
                    e,
                    Symbol::new("BTC", "USDT"),
                    vec![OrderBookLevel::new(
                        Decimal::from(50000 + i as i32),
                        Decimal::from(10),
                    )],
                    vec![OrderBookLevel::new(
                        Decimal::from(50005 + i as i32),
                        Decimal::from(10),
                    )],
                ));
                (e, orderbook)
            })
            .collect();
        group.bench_function("max_spread", |b| {
            b.iter(|| {
                let mut max_spread = Decimal::ZERO;
                for i in 0..ALL_EXCHANGES.len() {
                    for j in (i + 1)..ALL_EXCHANGES.len() {
                        let bid = orderbooks[&ALL_EXCHANGES[i]].best_bid().map(|l| l.price);
                        let ask = orderbooks[&ALL_EXCHANGES[j]].best_ask().map(|l| l.price);
                        if let (Some(b), Some(a)) = (bid, ask) {
                            let spread = a - b;
                            if spread > max_spread {
                                max_spread = spread;
                            }
                        }
                    }
                }
                black_box(max_spread)
            })
        });
        group.finish();
    }
}

mod exchange_rate_benchmarks {
    use super::*;

    pub fn benchmark_fee_schedule_lookup(c: &mut Criterion) {
        let mut group = c.benchmark_group("fees/lookup");
        let schedules: HashMap<ExchangeId, CoreFeeSchedule> = ALL_EXCHANGES
            .iter()
            .map(|&e| {
                (
                    e,
                    CoreFeeSchedule::new(e, Decimal::from(10), Decimal::from(20)),
                )
            })
            .collect();
        group.bench_function("get_okx_fee", |b| {
            b.iter(|| {
                let fee = schedules.get(&ExchangeId::OKX).cloned();
                black_box(fee)
            })
        });
        group.finish();
    }

    pub fn benchmark_fee_calculation_detailed(c: &mut Criterion) {
        let mut group = c.benchmark_group("fees/detailed");
        let quantity = Decimal::from(1);
        let price = Decimal::from(50000);
        let taker_fee_bps = Decimal::from(10);
        group.bench_function("taker_fee", |b| {
            b.iter(|| {
                let notional = price * quantity;
                let fee = notional * taker_fee_bps / Decimal::from(10000);
                black_box(fee)
            })
        });
        group.finish();
    }

    pub fn benchmark_total_fee_calculation(c: &mut Criterion) {
        let mut group = c.benchmark_group("fees/total");
        let fee_a = CoreFeeSchedule::new(ExchangeId::OKX, Decimal::from(10), Decimal::from(20));
        let fee_b = CoreFeeSchedule::new(ExchangeId::ByBit, Decimal::from(10), Decimal::from(20));
        let quantity = Decimal::from(1);
        let avg_price = Decimal::from(50000);
        group.bench_function("round_trip", |b| {
            b.iter(|| {
                let notional = avg_price * quantity;
                let total_fee =
                    notional * (fee_a.taker_fee + fee_b.taker_fee) / Decimal::from(100000);
                black_box(total_fee)
            })
        });
        group.finish();
    }
}

criterion_group!(
    additional_benches,
    additional_strategy_benchmarks::benchmark_convergence_detect,
    additional_strategy_benchmarks::benchmark_hedged_funding_detect,
    additional_strategy_benchmarks::benchmark_latency_detect,
    additional_strategy_benchmarks::benchmark_funding_rate_detect,
    additional_calculation_benchmarks::benchmark_spread_percentage,
    additional_calculation_benchmarks::benchmark_profit_loss,
    additional_calculation_benchmarks::benchmark_notional_value,
    additional_calculation_benchmarks::benchmark_order_cost,
    additional_calculation_benchmarks::benchmark_price_impact,
    additional_calculation_benchmarks::benchmark_arbitrage_profit,
    additional_calculation_benchmarks::benchmark_annualized_rate,
    additional_calculation_benchmarks::benchmark_weighted_average_price,
    orderbook_benchmarks::benchmark_best_bid_ask,
    orderbook_benchmarks::benchmark_orderbook_spread,
    orderbook_benchmarks::benchmark_orderbook_mid_price,
    orderbook_benchmarks::benchmark_orderbook_depth,
    orderbook_benchmarks::benchmark_liquidity_at_price,
    orderbook_benchmarks::benchmark_vwap_execution,
    orderbook_benchmarks::benchmark_orderbook_is_valid,
    orderbook_benchmarks::benchmark_orderbook_imbalance,
    signal_processing_benchmarks::benchmark_signal_creation,
    signal_processing_benchmarks::benchmark_signal_validation,
    signal_processing_benchmarks::benchmark_signal_expired,
    signal_processing_benchmarks::benchmark_signal_profitability,
    signal_processing_benchmarks::benchmark_signals_sort,
    signal_processing_benchmarks::benchmark_signals_filter,
    signal_processing_benchmarks::benchmark_signal_clone,
    signal_processing_benchmarks::benchmark_signal_equality,
    market_data_benchmarks::benchmark_ticker_creation,
    market_data_benchmarks::benchmark_funding_rate_creation,
    market_data_benchmarks::benchmark_orderbook_snapshot,
    market_data_benchmarks::benchmark_market_bundle_construction,
    market_data_benchmarks::benchmark_price_comparison,
    market_data_benchmarks::benchmark_cross_exchange_spread,
    exchange_rate_benchmarks::benchmark_fee_schedule_lookup,
    exchange_rate_benchmarks::benchmark_fee_calculation_detailed,
    exchange_rate_benchmarks::benchmark_total_fee_calculation,
);

criterion_group!(
    benches,
    detection_benchmarks::benchmark_cex_arbitrage_detection,
    detection_benchmarks::benchmark_funding_rate_detection,
    detection_benchmarks::benchmark_latency_detection,
    detection_benchmarks::benchmark_convergence_detection,
    calculation_benchmarks::benchmark_vwap_calculation,
    calculation_benchmarks::benchmark_profit_calculation,
    calculation_benchmarks::benchmark_fee_calculation,
    calculation_benchmarks::benchmark_spread_calculation,
    scalability_benchmarks::benchmark_100_symbols,
    scalability_benchmarks::benchmark_1000_symbols,
    scalability_benchmarks::benchmark_10_exchanges,
    memory_benchmarks::benchmark_order_book_allocation,
    memory_benchmarks::benchmark_signal_creation,
    memory_benchmarks::benchmark_market_bundle_build,
    strategy_benchmarks::benchmark_cross_exchange_arbitrage,
    strategy_benchmarks::benchmark_stablecoin_arbitrage,
    strategy_benchmarks::benchmark_spot_perp_arbitrage,
    strategy_benchmarks::benchmark_new_listing_arbitrage,
    strategy_benchmarks::benchmark_hedged_funding,
    strategy_benchmarks::benchmark_spread_capture,
    strategy_benchmarks::benchmark_strategy_registry,
    types_benchmarks::benchmark_symbol_parsing,
    types_benchmarks::benchmark_order_creation,
    types_benchmarks::benchmark_orderbook_validation,
    types_benchmarks::benchmark_exchange_id_parsing,
    fee_benchmarks::benchmark_fee_schedules,
    fee_benchmarks::benchmark_net_profit_calculation,
    utilities_benchmarks::benchmark_best_price_selection,
    utilities_benchmarks::benchmark_opportunity_ranking,
    utilities_benchmarks::benchmark_liquidity_calculation,
    additional_core_benchmarks::benchmark_decimal_addition,
    additional_core_benchmarks::benchmark_decimal_subtraction,
    additional_core_benchmarks::benchmark_decimal_multiplication,
    additional_core_benchmarks::benchmark_decimal_division,
    additional_core_benchmarks::benchmark_format_exchange,
    additional_core_benchmarks::benchmark_symbol_pair,
    additional_core_benchmarks::benchmark_best_bid,
    additional_core_benchmarks::benchmark_best_ask,
    additional_core_benchmarks::benchmark_create_buy_order,
    additional_core_benchmarks::benchmark_create_sell_order,
    additional_core_benchmarks::benchmark_create_signal,
    additional_core_benchmarks::benchmark_signal_expired,
    additional_core_benchmarks::benchmark_empty_bundle,
    additional_core_benchmarks::benchmark_bundle_has_data,
    additional_core_benchmarks::benchmark_strategy_id,
    additional_core_benchmarks::benchmark_strategy_name,
    additional_core_benchmarks::benchmark_filter_context,
    additional_core_benchmarks::benchmark_exchange_allowed,
    additional_core_benchmarks::benchmark_raw_signal,
    additional_core_benchmarks::benchmark_trade_leg,
    additional_core_benchmarks::benchmark_confidence_factors,
    additional_core_benchmarks::benchmark_vec_push,
    additional_core_benchmarks::benchmark_vec_iter_sum,
    additional_core_benchmarks::benchmark_hashmap_insert,
    additional_core_benchmarks::benchmark_sort_1000,
    additional_core_benchmarks::benchmark_json_serialize,
    additional_core_benchmarks::benchmark_utc_now,
    additional_core_benchmarks::benchmark_uuid_gen,
    additional_core_benchmarks::benchmark_option_unwrap,
    additional_core_benchmarks::benchmark_result_map,
);

criterion_main!(benches);
