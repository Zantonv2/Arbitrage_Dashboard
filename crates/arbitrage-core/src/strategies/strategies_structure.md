---

# Общее архитектурное ядро для арбитражного движка

```text
+---------------------------------+
|       Data Layer (Feeds)        |
+---------------------------------+
| → Exchange Connectors (WS/REST) |
| → Normalized Market Data Cache  |
| → Latency / Health Monitors     |
+---------------------------------+
             ↓
+---------------------------------+
|      Signal Detection Layer     |
+---------------------------------+
| → Price diff calc (real-time)   |
| → Spread / basis / funding calc |
| → Liquidity/Depth evaluation    |
| → Strategy plug-ins             |
+---------------------------------+
             ↓
+---------------------------------+
|      Opportunity Filters        |
+---------------------------------+
| → Min basis/profit thresholds   |
| → Max exposure/funding filters  |
| → Exchange pair matching        |
| → Risk constraints              |
+---------------------------------+
             ↓
+---------------------------------+
|         Signal Scoring          |
+---------------------------------+
| → Confidence score evaluator    |
| → Custom weights (stability,    |
|    liquidity, latency)          |
| → Strategy weight normalization |
+---------------------------------+
             ↓
+---------------------------------+
|       Execution Router          |
+---------------------------------+
| → Order Planner                 |
| → Smart Order Routing (SOR)     |
| → Position management           |
| → Fees/Slippage calculator      |
+---------------------------------+
             ↓
+---------------------------------+
|  Execution & Risk Controller    |
+---------------------------------+
| → Order submission              |
| → Execution confirmation        |
| → Stop/Cancel / fallback logic  |
| → Real-time position tracking   |
+---------------------------------+
             ↓
+---------------------------------+
|   P&L / Analytics / Telemetry   |
+---------------------------------+
| → Persist trades/events         |
| → Real-time metrics             |
| → Alerts and dashboards         |
+---------------------------------+
```

---

# Компоненты и что они делают

## 1) **Data Layer (Feeds)**

Реализует источники данных через публичные WS/REST и нормализует:

* **Order book snapshots & deltas**
* **Trade streams**
* **Funding rates**
* **Tickers**
* **Exchange time / latency**

Каждый источник публикует **NormalizedTick**, **NormalizedBook**, **FundingRate**:

```rust
struct NormalizedTick { exchange: ExchangeId, symbol: Symbol, price: Decimal, timestamp: DateTime<Utc>, }
struct NormalizedBook { bids: Vec<OrderLevel>, asks: Vec<OrderLevel>, timestamp: DateTime<Utc>, }
struct FundingRate { rate: Decimal, nextUpdate: DateTime<Utc>, }
```

Для каждого обмена пишешь адаптер, нормализующий WS/REST в эту единообразную модель.

---

## 2) **Signal Detection Layer**

Это ядро всех стратегий.

Пушит normalized данные → высчитывает:

* Basis = perp_price – spot_price
* Spread = ask – bid
* Relative diff = (spread / mid_price) * 10000 bps
* Funding drift expected = funding_rate × time_to_next_funding ([arbitragescanner.io][1])

У каждой стратегии есть **plug-in функция**:

```rust
fn detect(&self, data: &NormalizedData) -> Option<Signal> { … }
```

---

## 3) **Opportunity Filters**

Универсальная фильтрация:

* Minimum basis
* Minimum expected profit after fees
* Liquidity thresholds (min depth @ top levels)
* Exchange compatibility (symbols present on both legs)
* Funding risk filters
* Latency risk filters

---

## 4) **Signal Scoring**

Твоя `confidence_score` становится частью:

```rust
score = w1 * stability + w2 * spread + w3 * liquidity - w4 * latency_penalty
```

где веса меняются через конфиг, не код.

---

## 5) **Execution Router**

Единый интерфейс:

```rust
fn plan_trade(signal: &Signal) -> ExecutionPlan { … }
fn execute(plan: ExecutionPlan) -> OrderResult { … }
```

Сюда входит:

* Размеры ордеров
* Желательные книги/ценовые уровни
* Минимальный отклик (~latency, slippage)
* Частичные fills
* Maker/Taker fee учёт

---

## 6) **Execution & Risk Controller**

Смотрит на такие вещи:

* **Position limits**
* **Exposure limits**
* **Error reconciliation**
* **Order retries/cancellations**
* **Fill tracking и rebalancing**

---

## Универсальная стратегия как plug-in

Каждая стратегия реализуется как модуль, который заполняет три функции:

```rust
pub trait Strategy {
    /// входящие normalized данные
    fn detect(&self, mb: &MarketBundle) -> Option<Signal>;
    fn filter(&self, s: &Signal, ctx: &Context) -> bool;
    fn plan(&self, s: &Signal, ctx: &Context) -> ExecutionPlan;
}
```

Пример расширения для spot/futures:

```rust
impl Strategy for SpotFutures {
    fn detect(&self, data: &MarketBundle) -> Option<Signal> { … }
    fn filter(&self, signal: &Signal, ctx: &Context) -> bool { … }
    fn plan(&self, signal: &Signal, ctx: &Context) -> ExecutionPlan { … }
}
```

---

## Что остаётся менять при добавлении новой стратегии

| Изменяемый модуль | Что туда писать       |
| ----------------- | --------------------- |
| Strategy detect   | Как вычислять edge    |
| Filter criteria   | Бизнес-ограничения    |
| Execution planner | Какие ордера и когда  |
| Scoring model     | Переопределение весов |

Все остальные слои **ни разу не трогаются**.

---

## Формат сигналов (ставим единый контракт)

```rust
struct Signal {
    strategy_id: StrategyId,
    symbol: Symbol,
    legs: Vec<TradeLeg>,   // spot, perp, etc.
    expected_profit_bps: i32,
    basis_bps: i32,
    timestamps: Vec<DateTime<Utc>>,
}
```

---

## Нормализованная абстракция рынков

```rust
enum MarketSide { Spot, Perp, Futures, Funding }
struct TradeLeg {
    exchange: ExchangeId,
    symbol: Symbol,
    price: Decimal,
    size: Decimal,
    side: Side,
}
```

---

## Почему это удобно

* **Сменил стратегию — не ломай движок.**
* **Все данные унифицированы.**
* **Execution отделён от сигналов.**
* **Любую метрику можно логгировать, back-testить или визуализировать.** ([CodesenSys][2])






---

# 📘 **Полная документация арбитражного движка**

---

## **1. Концепция арбитража в криптовалютах**

Арбитраж — это использование разницы цен на один и тот же актив на разных рынках или между инструментами для получения прибыли с минимальным риском рыночного движения. Основные типы арбитража в крипто рынках:

* Спотовый межбиржевой (между разными CEX)
* Spot ↔ Futures (арбитраж между спотом и бессрочными фьючерсами)
* Funding/roll yield арбитраж (получение ставок финансирования)
* Другие вариации (трёхсторонний, статистический и т.д.) ([Habr][1])

---

## **2. Ключевые стратегии для CEX-only движка**

### **2.1 CEX ↔ CEX Price Arbitrage**

**Essence:** Buy on the cheaper exchange, sell on the more expensive one; profit from price spread.
**Notes:** Must account for fees, execution latency, and deposit positions on exchanges.
**Related:** Classic “spatial arbitrage” in economics — exploiting price differences between markets. ([Википедия][2])

---

### **2.2 Spot ↔ Perpetual Arbitrage**

**Essence:**

* Take a long on spot and short on perpetual futures (or vice versa) to exploit divergence between spot and perp price.
* Hedge price risk while earning on the spread.
  **Notes:** Useful when futures price diverges from spot (contango/backwardation). ([Arbitrage Scanner][3])

---

### **2.3 Funding Rate Arbitrage (Roll Yield)**

**Essence:**

* Perpetual futures use funding rates to maintain price parity with the spot market.
* If funding is positive, short positions receive payments from long positions — this can be arbitraged by spot long + perp short.
  **Notes:** Mechanism described in funding rate arbitrage guides. ([Habr][1])

---

### **2.4 Funding + Spot Hedged Strategy**

**Essence:**

* Variation of funding rate arb: hedge spot risk while earning funding differentials.
* Combines features of two strategies above.
  **Notes:** Requires careful net exposure control.

---

### **2.5 Pre-Positioned Capital Cross-Exchange Arbitrage**

**Essence:**

* Maintain assets on multiple exchanges to execute arbitrage instantly without transfer delays.
  **Notes:** Requires funding and balance planning but avoids execution lag.

---

### **2.6 Limit-Orders Spread Capture (Passive Arbitrage)**

**Essence:**

* Place limit orders within trading spread to capture small inefficiencies (maker fees / rebates).
  **Notes:** Requires continuous order book monitoring.

---

### **2.7 Temporal / Latency Arbitrage**

**Essence:**

* Price discrepancies appear due to data propagation delays between exchanges.
* Exploit very short-lived mismatches.
  **Notes:** Highly latency-sensitive.

---

### **2.8 Stablecoin Peg Arbitrage**

**Essence:**

* Stablecoins sometimes deviate from their peg (e.g., USDT ≠ USDC).
* Capture tiny deviations across markets.

**Note:** Requires careful accounting for fees and slippage.

---

### **2.9 Convergence (Statistical) Arbitrage**

**Essence:**

* Trade correlated pairs that diverge and expected to revert.
* Can be extended to pairs like BTC/ETH, BTC/BCH, etc.
  **Note:** More advanced statistical model. ([Википедия][4])

---

### **2.10 New Listing Cross-Exchange Arbitrage**

**Essence:**

* Tokens often appear earlier on one exchange than another; initial pricing inefficiencies.
* Exploit mispricing immediately after listing.

---

## **3. Слои архитектуры арбитражного движка**

---

### 🧩 **3.1 Data Layer (Market Feeds)**

**Tasks:**

* Normalize all market data into unified format regardless of exchange.
* Subscribe to public WebSocket streams for real-time tickers, order book and trades.
* Fallback REST polling for backups.

**Components:**

* **WS Connectors** for each exchange
* **REST pollers** for depth, tickers, funding
* **Normalizer** → unified structures (Tickers, Depth, FundingRate)
* **Latency Monitor** → track delay for each feed

---

### 🧠 **3.2 Signal Detection Layer**

**Tasks:**

* Real-time computation of spreads, basis, funding differentials.
* Generate **raw signals** for each strategy.

**Works with:**

* Normalized tickers
* Depth snapshots
* Funding updates

**Outputs:**

* `RawSignal { strategy: StrategyId, legs: ..., metrics: {...} }`

---

### 🧪 **3.3 Opportunity Filters**

Apply business constraints:

* Minimum basis threshold
* Minimum expected profit *after fees*
* Liquidity constraints (depth @ top levels)
* Exchange symbol matches
* Funding rate limits
* Latency thresholds

---

### 📊 **3.4 Signal Scoring / Confidence**

Each opportunity gets a *confidence score* based on:

| Factor              | Role                  |
| ------------------- | --------------------- |
| Spread strength     | Magnitude of edge     |
| Liquidity           | Execution feasibility |
| Latency             | Feed freshness        |
| Funding correctness | Stability             |

**Note:** Avoid simplistic scores; use multi-factor model.

---

### 🚀 **3.5 Execution Planner & Router**

Generates execution plan:

* SOR (Smart Order Routing)
* Order sizing (risk / notional buckets)
* Fee and slippage adjustments
* Position sizing

---

### 📈 **3.6 Risk & Execution Controller**

Handles:

* Order submission & fills
* Position tracking
* Risk limits:

  * max per pair
  * max exposure
  * funding risk
* Rebalancing & stop logic

---

### 🪙 **3.7 Logging / Analytics / Telemetry**

Stores:

* All raw feeds
* Signals
* Orders and fills
* PnL reports

Build analytics dashboards / alerts.

---

## **4. Data models (normalized)**

**MarketTick**

```rust
struct MarketTick {
    exchange: ExchangeId,
    symbol: Symbol,
    bid: Decimal,
    ask: Decimal,
    last: Decimal,
    timestamp: DateTime<Utc>,
}
```

**OrderBook**

```rust
struct OrderBook {
    bids: Vec<OrderLevel>,
    asks: Vec<OrderLevel>,
    timestamp: DateTime<Utc>,
}
```

**FundingRate**

```rust
struct FundingRate {
    exchange: ExchangeId,
    symbol: Symbol,
    rate: Decimal,
    next_update: DateTime<Utc>,
}
```

**Signal**

```rust
struct Signal {
    strategy_id: StrategyId,
    legs: Vec<TradeLeg>,
    expected_profit: Decimal,
    confidence: f64,
    timestamp: DateTime<Utc>,
}
```

---

## **5. TradeLeg** (atomic order component)

```rust
enum Side { Buy, Sell }
struct TradeLeg {
    exchange: ExchangeId,
    symbol: Symbol,
    price: Decimal,
    side: Side,
    amount: Decimal,
}
```

---

## **6. ExecutionPlan**

```rust
struct ExecutionPlan {
    legs: Vec<TradeOrder>,
    expected_profit: Decimal,
    worst_case_slippage: Decimal,
    time_in_force: TimeInForce,
}
```

---

## **7. Risk Controls**

* Max position size per exchange
* Max daily exposure
* Funding rate cut-offs
* Roll-rate exposure limits

*Arbitrage uses hedged or neutral positions whenever possible to reduce market risk.* ([Habr][1])

---

## **8. Rate Limits, Throttling & Delays**

* WS: fastest real-time signal source (~10–100 ms)
* REST fallback: slower (~500 ms–2 s)
* Never rely on REST for primary signal detection — use it for confirmation

---

## **9. Backtesting & Simulation**

Before live trading:

* Replay historical WS data
* Simulate order book fills
* Apply fee & slippage models
* Compute PnL, drawdown, Sharpe

---

## **10. Deployment & Scaling**

* Local worker for each exchange feed
* Central signal aggregator
* Execution workers per strategy
* Persistence: time-series database (InfluxDB / Timescale) for feeds + logs

---