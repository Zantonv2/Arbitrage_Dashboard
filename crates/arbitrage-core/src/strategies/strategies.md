## **1. CEX ↔ CEX Price Arbitrage**

**Principle**
Simultaneously buy on the exchange with the lowest ask and sell on the exchange with the highest bid using pre-funded balances.

**Endpoints**

* **OKX**

  * WS: `wss://ws.okx.com:8443/ws/v5/public` → `tickers`, `books`, `trades`
  * REST: `/api/v5/market/tickers`, `/api/v5/market/books`
* **Bybit**

  * WS: `wss://stream.bybit.com/v5/public/spot` → `tickers`, `orderbook`, `trade`
  * REST: `/v5/market/tickers`, `/v5/market/orderbook`
* **MEXC**

  * WS: `wss://wbs.mexc.com/ws` → `spot.tickers`, `spot.depth`
  * REST: `/api/v3/ticker/bookTicker`
* **Gate.io**

  * WS: `wss://api.gateio.ws/ws/v4/` → `spot.tickers`, `spot.order_book`
  * REST: `/api/v4/spot/tickers`
* **Bitstamp**

  * WS: `wss://ws.bitstamp.net` → `order_book`, `live_trades`
  * REST: `/api/v2/order_book/{pair}/`
* **Kraken**

  * WS: `wss://ws.kraken.com` → `ticker`, `book`
  * REST: `/0/public/Ticker`, `/0/public/Depth`

---

## **2. Spot ↔ Perpetual (Futures) Arbitrage**

**Principle**
Hedge spot and perpetual positions to capture price divergence while remaining market-neutral.

**Endpoints**

* **OKX**

  * WS: `tickers`, `books` (SPOT + SWAP)
  * REST: `/api/v5/market/tickers`, `/api/v5/market/books`
* **Bybit**

  * WS: `spot@tickers`, `linear@tickers`
  * REST: `/v5/market/tickers`
* **MEXC**

  * WS: `spot.tickers`, `contract.tickers`
  * REST: `/api/v3/ticker/price`, `/api/v1/contract/ticker`

---

## **3. Funding Rate Arbitrage (Roll Yield)**

**Principle**
Earn periodic funding payments by holding the side of perpetual contracts that receives funding.

**Endpoints**

* **OKX**

  * WS: `funding-rate`
  * REST: `/api/v5/public/funding-rate`
* **Bybit**

  * WS: `funding`
  * REST: `/v5/market/funding-rate`
* **MEXC**

  * WS: `contract.funding_rate`
  * REST: `/api/v1/contract/funding_rate`

---

## **4. Funding + Spot Hedged Strategy**

**Principle**
Hold spot while hedging with perpetuals to capture funding with minimal price exposure.

**Endpoints**

* **OKX**

  * WS: `funding-rate`, `tickers`, `books`
  * REST: `/api/v5/public/funding-rate`
* **Bybit**

  * WS: `funding`, `spot@tickers`
  * REST: `/v5/market/funding-rate`
* **MEXC**

  * WS: `contract.funding_rate`, `spot.tickers`
  * REST: `/api/v1/contract/funding_rate`

---

## **5. Pre-Positioned Capital Cross-Exchange Arbitrage**

**Principle**
Execute instant arbitrage trades by keeping balances on multiple exchanges.

**Endpoints**

* Same as **CEX ↔ CEX Price Arbitrage**
* Core requirement: `tickers` + `order book` WS on all exchanges

---

## **6. Limit Order Spread Capture**

**Principle**
Place passive limit orders inside the spread to capture micro inefficiencies.

**Endpoints**

* **OKX**

  * WS: `books`, `trades`
  * REST: `/api/v5/market/books`
* **Bybit**

  * WS: `orderbook`, `trade`
  * REST: `/v5/market/orderbook`
* **MEXC**

  * WS: `spot.depth`
  * REST: `/api/v3/depth`
* **Gate.io**

  * WS: `spot.order_book`
  * REST: `/api/v4/spot/order_book`

---

## **7. Temporal (Latency) Arbitrage**

**Principle**
Exploit short-lived price differences caused by slower market data propagation.

**Endpoints**

* WS only (critical)

  * OKX: `tickers`
  * Bybit: `tickers`
  * MEXC: `spot.tickers`
  * Gate.io: `spot.tickers`

---

## **8. Stablecoin Peg Arbitrage**

**Principle**
Trade temporary deviations between stablecoins and their target peg.

**Endpoints**

* **OKX**

  * WS: `tickers`
  * REST: `/api/v5/market/tickers`
* **Bybit**

  * WS: `spot@tickers`
  * REST: `/v5/market/tickers`
* **Gate.io**

  * WS: `spot.tickers`
  * REST: `/api/v4/spot/tickers`

---

## **9. Convergence Arbitrage**

**Principle**
Exploit mean reversion between correlated instruments across markets.

**Endpoints**

* Same as **Spot ↔ Perpetual Arbitrage**
* Requires synchronized `tickers` + `books`

---

## **10. New Listing Cross-Exchange Arbitrage**

**Principle**
Exploit inefficient price discovery immediately after a token listing.

**Endpoints**

* **MEXC**

  * WS: `spot.tickers`
  * REST: `/api/v3/exchangeInfo`
* **Gate.io**

  * WS: `spot.tickers`
  * REST: `/api/v4/spot/currency_pairs`
* **OKX**

  * WS: `tickers`
  * REST: `/api/v5/public/instruments`
* **Bybit**

  * WS: `spot@tickers`
  * REST: `/v5/market/instruments-info`