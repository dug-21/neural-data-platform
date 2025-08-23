# Neural Trader V2 - Test Data Generators

## Overview

Comprehensive test data generation framework providing realistic, deterministic, and edge-case data for thorough testing of Neural Trader V2 components.

## Data Generation Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                 Test Data Factory System                   │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ ┌────────┐ │
│  │   Market    │ │   Trading   │ │  Portfolio  │ │ User   │ │
│  │    Data     │ │    Data     │ │    Data     │ │ Data   │ │
│  │  Generator  │ │  Generator  │ │  Generator  │ │Generate│ │
│  └─────────────┘ └─────────────┘ └─────────────┘ └────────┘ │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ ┌────────┐ │
│  │ Performance │ │   Config    │ │   System    │ │ Chaos  │ │
│  │    Data     │ │    Data     │ │   Events    │ │ Data   │ │
│  │  Generator  │ │  Generator  │ │  Generator  │ │Generate│ │
│  └─────────────┘ └─────────────┘ └─────────────┘ └────────┘ │
└─────────────────────────────────────────────────────────────┘
```

## 1. Core Factory System

### Base Factory Interface
```typescript
// tests/generators/base-factory.ts
export interface FactoryOptions {
  count?: number;
  seed?: number;
  realistic?: boolean;
  edgeCases?: boolean;
}

export abstract class BaseFactory<T> {
  protected faker: Faker;
  protected options: FactoryOptions;

  constructor(options: FactoryOptions = {}) {
    this.options = {
      count: 1,
      seed: 42,
      realistic: true,
      edgeCases: false,
      ...options,
    };

    this.faker = new Faker({
      locale: [en],
      seed: this.options.seed,
    });
  }

  abstract generate(): T;
  abstract generateBatch(count?: number): T[];
  abstract generateEdgeCases(): T[];

  protected randomChoice<U>(items: U[]): U {
    return items[Math.floor(Math.random() * items.length)];
  }

  protected randomFloat(min: number, max: number): number {
    return Math.random() * (max - min) + min;
  }

  protected randomInt(min: number, max: number): number {
    return Math.floor(Math.random() * (max - min + 1)) + min;
  }
}
```

## 2. Market Data Generators

### Real-time Market Data
```typescript
// tests/generators/market-data-factory.ts
export interface MarketDataPoint {
  symbol: string;
  price: number;
  volume: number;
  timestamp: Date;
  bid: number;
  ask: number;
  high24h: number;
  low24h: number;
  change24h: number;
  changePercent24h: number;
}

export class MarketDataFactory extends BaseFactory<MarketDataPoint> {
  private static readonly SYMBOLS = [
    'BTCUSD', 'ETHUSD', 'ADAUSD', 'SOLUSD', 'DOTUSD',
    'LINKUSD', 'AVAXUSD', 'MATICUSD', 'ALGOUSD', 'ATOMUSD'
  ];

  private static readonly BASE_PRICES: Record<string, number> = {
    BTCUSD: 50000,
    ETHUSD: 3000,
    ADAUSD: 0.5,
    SOLUSD: 100,
    DOTUSD: 25,
    LINKUSD: 15,
    AVAXUSD: 35,
    MATICUSD: 0.8,
    ALGOUSD: 0.3,
    ATOMUSD: 12,
  };

  generate(): MarketDataPoint {
    const symbol = this.randomChoice(MarketDataFactory.SYMBOLS);
    const basePrice = MarketDataFactory.BASE_PRICES[symbol];
    
    return this.generateForSymbol(symbol, basePrice);
  }

  generateForSymbol(symbol: string, basePrice?: number): MarketDataPoint {
    const price = basePrice || MarketDataFactory.BASE_PRICES[symbol] || 1000;
    const volatility = this.getVolatility(symbol);
    
    const priceChange = (Math.random() - 0.5) * price * volatility;
    const currentPrice = price + priceChange;
    
    const spread = currentPrice * this.getSpread(symbol);
    const bid = currentPrice - spread / 2;
    const ask = currentPrice + spread / 2;
    
    // Generate 24h data
    const high24h = currentPrice * (1 + Math.random() * 0.05);
    const low24h = currentPrice * (1 - Math.random() * 0.05);
    const change24h = (Math.random() - 0.5) * currentPrice * 0.1;
    const changePercent24h = (change24h / currentPrice) * 100;

    return {
      symbol,
      price: this.roundToSignificantDecimals(currentPrice, 6),
      volume: this.randomFloat(100, 10000),
      timestamp: new Date(),
      bid: this.roundToSignificantDecimals(bid, 6),
      ask: this.roundToSignificantDecimals(ask, 6),
      high24h: this.roundToSignificantDecimals(high24h, 6),
      low24h: this.roundToSignificantDecimals(low24h, 6),
      change24h: this.roundToSignificantDecimals(change24h, 6),
      changePercent24h: this.roundToSignificantDecimals(changePercent24h, 2),
    };
  }

  generateBatch(count: number = this.options.count || 1): MarketDataPoint[] {
    return Array.from({ length: count }, () => this.generate());
  }

  generateTimeSeries(
    symbol: string,
    startTime: Date,
    intervalMs: number,
    count: number
  ): MarketDataPoint[] {
    const series: MarketDataPoint[] = [];
    let currentTime = new Date(startTime);
    let currentPrice = MarketDataFactory.BASE_PRICES[symbol] || 1000;

    for (let i = 0; i < count; i++) {
      const volatility = this.getVolatility(symbol);
      const priceChange = (Math.random() - 0.5) * currentPrice * volatility;
      currentPrice = Math.max(currentPrice + priceChange, currentPrice * 0.01);

      const dataPoint = this.generateForSymbol(symbol, currentPrice);
      dataPoint.timestamp = new Date(currentTime);
      
      series.push(dataPoint);
      currentTime = new Date(currentTime.getTime() + intervalMs);
    }

    return series;
  }

  generateEdgeCases(): MarketDataPoint[] {
    return [
      // Zero price
      { ...this.generate(), price: 0, bid: 0, ask: 0 },
      
      // Extremely high price
      { ...this.generate(), price: Number.MAX_SAFE_INTEGER },
      
      // Negative spread (invalid)
      { ...this.generate(), bid: 100, ask: 99 },
      
      // Zero volume
      { ...this.generate(), volume: 0 },
      
      // Future timestamp
      { ...this.generate(), timestamp: new Date(Date.now() + 86400000) },
      
      // Past timestamp (very old)
      { ...this.generate(), timestamp: new Date(Date.now() - 86400000 * 365) },
    ];
  }

  private getVolatility(symbol: string): number {
    const volatilityMap: Record<string, number> = {
      BTCUSD: 0.003,   // 0.3%
      ETHUSD: 0.004,   // 0.4%
      ADAUSD: 0.008,   // 0.8%
      SOLUSD: 0.01,    // 1.0%
      DOTUSD: 0.008,   // 0.8%
      LINKUSD: 0.008,  // 0.8%
      AVAXUSD: 0.01,   // 1.0%
      MATICUSD: 0.01,  // 1.0%
      ALGOUSD: 0.015,  // 1.5%
      ATOMUSD: 0.008,  // 0.8%
    };
    
    return volatilityMap[symbol] || 0.005;
  }

  private getSpread(symbol: string): number {
    const spreadMap: Record<string, number> = {
      BTCUSD: 0.0001,   // 0.01%
      ETHUSD: 0.0001,   // 0.01%
      ADAUSD: 0.0002,   // 0.02%
      SOLUSD: 0.0002,   // 0.02%
      DOTUSD: 0.0002,   // 0.02%
      LINKUSD: 0.0002,  // 0.02%
      AVAXUSD: 0.0002,  // 0.02%
      MATICUSD: 0.0003, // 0.03%
      ALGOUSD: 0.0005,  // 0.05%
      ATOMUSD: 0.0002,  // 0.02%
    };
    
    return spreadMap[symbol] || 0.0002;
  }

  private roundToSignificantDecimals(num: number, decimals: number): number {
    return Math.round(num * Math.pow(10, decimals)) / Math.pow(10, decimals);
  }
}
```

### Historical Market Data Generator
```typescript
// tests/generators/historical-data-factory.ts
export class HistoricalDataFactory extends MarketDataFactory {
  generateHistoricalCandles(
    symbol: string,
    timeframe: '1m' | '5m' | '15m' | '1h' | '4h' | '1d',
    count: number,
    endTime?: Date
  ): OHLCV[] {
    const intervalMs = this.getIntervalMs(timeframe);
    const end = endTime || new Date();
    const start = new Date(end.getTime() - (count * intervalMs));
    
    const candles: OHLCV[] = [];
    let currentTime = start;
    let currentPrice = MarketDataFactory.BASE_PRICES[symbol] || 1000;

    for (let i = 0; i < count; i++) {
      const { open, high, low, close, volume } = this.generateCandle(
        symbol,
        currentPrice,
        intervalMs
      );

      candles.push({
        symbol,
        timeframe,
        timestamp: new Date(currentTime),
        open,
        high,
        low,
        close,
        volume,
      });

      currentPrice = close;
      currentTime = new Date(currentTime.getTime() + intervalMs);
    }

    return candles;
  }

  private generateCandle(
    symbol: string,
    startPrice: number,
    intervalMs: number
  ): Omit<OHLCV, 'symbol' | 'timeframe' | 'timestamp'> {
    const volatility = this.getVolatility(symbol);
    const trendStrength = (Math.random() - 0.5) * 0.1; // -5% to +5% trend
    
    const open = startPrice;
    const priceRange = startPrice * volatility * (intervalMs / 60000); // Scale by time
    
    // Generate random walk with trend
    const prices = [open];
    const steps = Math.max(4, Math.floor(intervalMs / 15000)); // At least 4 steps per candle
    
    for (let i = 0; i < steps; i++) {
      const randomChange = (Math.random() - 0.5) * priceRange;
      const trendChange = trendStrength * priceRange * (i / steps);
      const lastPrice = prices[prices.length - 1];
      
      const newPrice = Math.max(
        lastPrice + randomChange + trendChange,
        lastPrice * 0.001 // Prevent negative prices
      );
      
      prices.push(newPrice);
    }

    const high = Math.max(...prices);
    const low = Math.min(...prices);
    const close = prices[prices.length - 1];
    const volume = this.randomFloat(
      intervalMs / 60000 * 10,    // Minimum volume based on timeframe
      intervalMs / 60000 * 1000   // Maximum volume based on timeframe
    );

    return { open, high, low, close, volume };
  }

  private getIntervalMs(timeframe: string): number {
    const intervals: Record<string, number> = {
      '1m': 60 * 1000,
      '5m': 5 * 60 * 1000,
      '15m': 15 * 60 * 1000,
      '1h': 60 * 60 * 1000,
      '4h': 4 * 60 * 60 * 1000,
      '1d': 24 * 60 * 60 * 1000,
    };
    
    return intervals[timeframe];
  }
}

interface OHLCV {
  symbol: string;
  timeframe: string;
  timestamp: Date;
  open: number;
  high: number;
  low: number;
  close: number;
  volume: number;
}
```

## 3. Trading Data Generators

### Trade Data Factory
```typescript
// tests/generators/trade-factory.ts
export interface Trade {
  id: string;
  userId: string;
  symbol: string;
  side: 'buy' | 'sell';
  type: 'market' | 'limit' | 'stop' | 'stop-limit';
  quantity: number;
  price: number;
  executedQuantity: number;
  executedPrice: number;
  status: 'pending' | 'partial' | 'filled' | 'cancelled' | 'rejected';
  timestamp: Date;
  executionTime?: Date;
  fees: number;
  feeCurrency: string;
}

export class TradeFactory extends BaseFactory<Trade> {
  private static tradeIdCounter = 1;

  generate(): Trade {
    const symbols = ['BTCUSD', 'ETHUSD', 'ADAUSD'];
    const symbol = this.randomChoice(symbols);
    const side = this.randomChoice(['buy', 'sell'] as const);
    const type = this.randomChoice(['market', 'limit', 'stop'] as const);
    
    const basePrice = MarketDataFactory.BASE_PRICES[symbol] || 1000;
    const price = basePrice * (1 + (Math.random() - 0.5) * 0.01); // ±0.5%
    const quantity = this.randomFloat(0.001, 10);
    
    const isExecuted = Math.random() > 0.2; // 80% execution rate
    const executedQuantity = isExecuted ? 
      quantity * (0.8 + Math.random() * 0.2) : 0; // 80-100% fill
    
    const status = this.determineStatus(type, isExecuted, executedQuantity, quantity);
    
    return {
      id: `trade-${TradeFactory.tradeIdCounter++}`,
      userId: `user-${this.randomInt(1, 100)}`,
      symbol,
      side,
      type,
      quantity: this.roundToSignificantDecimals(quantity, 8),
      price: this.roundToSignificantDecimals(price, 2),
      executedQuantity: this.roundToSignificantDecimals(executedQuantity, 8),
      executedPrice: isExecuted ? 
        this.roundToSignificantDecimals(price * (1 + (Math.random() - 0.5) * 0.001), 2) : 
        0,
      status,
      timestamp: new Date(Date.now() - this.randomInt(0, 86400000)), // Last 24h
      executionTime: isExecuted ? 
        new Date(Date.now() - this.randomInt(0, 86400000)) : 
        undefined,
      fees: this.roundToSignificantDecimals(quantity * price * 0.001, 6), // 0.1% fee
      feeCurrency: 'USD',
    };
  }

  generateBatch(count: number = this.options.count || 1): Trade[] {
    return Array.from({ length: count }, () => this.generate());
  }

  generateEdgeCases(): Trade[] {
    return [
      // Zero quantity
      { ...this.generate(), quantity: 0 },
      
      // Extremely large quantity
      { ...this.generate(), quantity: 1000000 },
      
      // Zero price
      { ...this.generate(), price: 0 },
      
      // Negative price (invalid)
      { ...this.generate(), price: -100 },
      
      // Partial fill with zero executed
      { ...this.generate(), status: 'partial', executedQuantity: 0 },
      
      // Future timestamp
      { 
        ...this.generate(), 
        timestamp: new Date(Date.now() + 86400000),
        executionTime: new Date(Date.now() + 86400000) 
      },
    ];
  }

  generateTradingSession(
    userId: string,
    duration: number,
    tradeFrequency: number
  ): Trade[] {
    const trades: Trade[] = [];
    const startTime = Date.now() - duration;
    const tradeInterval = duration / tradeFrequency;
    
    for (let i = 0; i < tradeFrequency; i++) {
      const trade = this.generate();
      trade.userId = userId;
      trade.timestamp = new Date(startTime + (i * tradeInterval));
      
      if (trade.executionTime) {
        trade.executionTime = new Date(trade.timestamp.getTime() + this.randomInt(100, 5000));
      }
      
      trades.push(trade);
    }
    
    return trades;
  }

  private determineStatus(
    type: string,
    isExecuted: boolean,
    executedQuantity: number,
    quantity: number
  ): Trade['status'] {
    if (!isExecuted) return 'pending';
    if (executedQuantity === 0) return 'cancelled';
    if (executedQuantity < quantity) return 'partial';
    return 'filled';
  }

  private roundToSignificantDecimals(num: number, decimals: number): number {
    return Math.round(num * Math.pow(10, decimals)) / Math.pow(10, decimals);
  }
}
```

### Portfolio Data Factory
```typescript
// tests/generators/portfolio-factory.ts
export interface Portfolio {
  userId: string;
  totalValue: number;
  totalCost: number;
  totalPnl: number;
  totalPnlPercent: number;
  positions: Position[];
  cash: number;
  marginUsed: number;
  marginAvailable: number;
  lastUpdated: Date;
}

export interface Position {
  symbol: string;
  quantity: number;
  averagePrice: number;
  marketPrice: number;
  marketValue: number;
  unrealizedPnl: number;
  unrealizedPnlPercent: number;
  realizedPnl: number;
  side: 'long' | 'short';
  openDate: Date;
}

export class PortfolioFactory extends BaseFactory<Portfolio> {
  private marketDataFactory: MarketDataFactory;
  private tradeFactory: TradeFactory;

  constructor(options: FactoryOptions = {}) {
    super(options);
    this.marketDataFactory = new MarketDataFactory(options);
    this.tradeFactory = new TradeFactory(options);
  }

  generate(): Portfolio {
    const userId = `user-${this.randomInt(1, 1000)}`;
    const positionCount = this.randomInt(0, 10);
    const positions = this.generatePositions(positionCount);
    
    const totalValue = positions.reduce((sum, pos) => sum + pos.marketValue, 0);
    const totalCost = positions.reduce((sum, pos) => sum + (pos.quantity * pos.averagePrice), 0);
    const totalPnl = totalValue - totalCost;
    const totalPnlPercent = totalCost > 0 ? (totalPnl / totalCost) * 100 : 0;
    
    const cash = this.randomFloat(1000, 100000);
    const marginUsed = totalValue * 0.1; // 10% margin requirement
    const marginAvailable = cash - marginUsed;

    return {
      userId,
      totalValue,
      totalCost,
      totalPnl,
      totalPnlPercent,
      positions,
      cash,
      marginUsed,
      marginAvailable,
      lastUpdated: new Date(),
    };
  }

  generateBatch(count: number = this.options.count || 1): Portfolio[] {
    return Array.from({ length: count }, () => this.generate());
  }

  generateEdgeCases(): Portfolio[] {
    return [
      // Empty portfolio
      {
        ...this.generate(),
        positions: [],
        totalValue: 0,
        totalCost: 0,
        totalPnl: 0,
      },
      
      // Highly leveraged portfolio
      {
        ...this.generate(),
        marginUsed: 95000,
        marginAvailable: 5000,
      },
      
      // Portfolio with all losing positions
      {
        ...this.generate(),
        positions: this.generatePositions(5).map(pos => ({
          ...pos,
          marketPrice: pos.averagePrice * 0.5, // 50% loss
          marketValue: pos.quantity * pos.averagePrice * 0.5,
          unrealizedPnl: pos.quantity * pos.averagePrice * -0.5,
          unrealizedPnlPercent: -50,
        })),
      },
    ];
  }

  private generatePositions(count: number): Position[] {
    const positions: Position[] = [];
    const symbols = ['BTCUSD', 'ETHUSD', 'ADAUSD', 'SOLUSD'];
    
    for (let i = 0; i < count; i++) {
      const symbol = symbols[i % symbols.length];
      const side = this.randomChoice(['long', 'short'] as const);
      const quantity = this.randomFloat(0.1, 10);
      const averagePrice = (MarketDataFactory.BASE_PRICES[symbol] || 1000) * 
        (1 + (Math.random() - 0.5) * 0.1); // ±5% from current
      
      const marketData = this.marketDataFactory.generateForSymbol(symbol);
      const marketPrice = marketData.price;
      const marketValue = quantity * marketPrice;
      
      const priceDiff = marketPrice - averagePrice;
      const unrealizedPnl = side === 'long' ? 
        quantity * priceDiff : 
        quantity * -priceDiff;
      const unrealizedPnlPercent = (unrealizedPnl / (quantity * averagePrice)) * 100;

      positions.push({
        symbol,
        quantity: this.roundToSignificantDecimals(quantity, 8),
        averagePrice: this.roundToSignificantDecimals(averagePrice, 2),
        marketPrice: this.roundToSignificantDecimals(marketPrice, 2),
        marketValue: this.roundToSignificantDecimals(marketValue, 2),
        unrealizedPnl: this.roundToSignificantDecimals(unrealizedPnl, 2),
        unrealizedPnlPercent: this.roundToSignificantDecimals(unrealizedPnlPercent, 2),
        realizedPnl: this.roundToSignificantDecimals(this.randomFloat(-1000, 1000), 2),
        side,
        openDate: new Date(Date.now() - this.randomInt(0, 86400000 * 30)), // Last 30 days
      });
    }
    
    return positions;
  }

  private roundToSignificantDecimals(num: number, decimals: number): number {
    return Math.round(num * Math.pow(10, decimals)) / Math.pow(10, decimals);
  }
}
```

## 4. Performance Test Data Generators

### Load Test Data Generator
```typescript
// tests/generators/performance-data-factory.ts
export class PerformanceDataFactory extends BaseFactory<any> {
  generateHighVolumeMarketData(
    symbol: string,
    durationMs: number,
    ticksPerSecond: number
  ): MarketDataPoint[] {
    const totalTicks = Math.floor((durationMs / 1000) * ticksPerSecond);
    const marketFactory = new MarketDataFactory();
    const data: MarketDataPoint[] = [];
    
    let currentTime = Date.now();
    const tickInterval = 1000 / ticksPerSecond;
    
    for (let i = 0; i < totalTicks; i++) {
      const tick = marketFactory.generateForSymbol(symbol);
      tick.timestamp = new Date(currentTime);
      data.push(tick);
      
      currentTime += tickInterval;
    }
    
    return data;
  }

  generateConcurrentTrades(
    userCount: number,
    tradesPerUser: number,
    timeWindowMs: number
  ): Trade[] {
    const tradeFactory = new TradeFactory();
    const allTrades: Trade[] = [];
    
    for (let userId = 1; userId <= userCount; userId++) {
      const userTrades = tradeFactory.generateTradingSession(
        `load-test-user-${userId}`,
        timeWindowMs,
        tradesPerUser
      );
      allTrades.push(...userTrades);
    }
    
    // Randomize timestamps to simulate concurrent access
    return allTrades.map(trade => ({
      ...trade,
      timestamp: new Date(Date.now() + Math.random() * timeWindowMs),
    })).sort((a, b) => a.timestamp.getTime() - b.timestamp.getTime());
  }

  generateMemoryStressData(targetSizeMB: number): any[] {
    const targetBytes = targetSizeMB * 1024 * 1024;
    const data: any[] = [];
    let currentSize = 0;
    
    while (currentSize < targetBytes) {
      const item = {
        id: Math.random().toString(36),
        data: 'x'.repeat(1000), // 1KB string
        timestamp: new Date(),
        nested: {
          array: new Array(100).fill(Math.random()),
          object: Object.fromEntries(
            Array.from({ length: 50 }, (_, i) => [
              `key${i}`,
              Math.random().toString(36)
            ])
          ),
        },
      };
      
      data.push(item);
      currentSize += JSON.stringify(item).length;
    }
    
    return data;
  }

  generate(): any {
    return this.generateHighVolumeMarketData('BTCUSD', 60000, 100)[0];
  }

  generateBatch(count: number = this.options.count || 1): any[] {
    return this.generateHighVolumeMarketData('BTCUSD', 1000, count);
  }

  generateEdgeCases(): any[] {
    return [
      // Extremely high frequency data
      this.generateHighVolumeMarketData('BTCUSD', 1000, 10000)[0],
      
      // Zero volume trades
      ...this.generateConcurrentTrades(1, 1, 1000).map(trade => ({
        ...trade,
        quantity: 0,
      })),
    ];
  }
}
```

## 5. Configuration Data Generator

### Config Store Test Data
```typescript
// tests/generators/config-factory.ts
export interface ConfigEntry {
  key: string;
  value: any;
  environment: 'development' | 'testing' | 'production';
  namespace: string;
  version: number;
  createdAt: Date;
  updatedAt: Date;
  metadata: Record<string, any>;
}

export class ConfigFactory extends BaseFactory<ConfigEntry> {
  private static readonly NAMESPACES = [
    'trading', 'market-data', 'auth', 'database', 'cache', 'notification'
  ];

  private static readonly CONFIG_TEMPLATES = {
    trading: {
      'max-position-size': 1000000,
      'stop-loss-percentage': 0.05,
      'take-profit-percentage': 0.15,
      'risk-management-enabled': true,
    },
    'market-data': {
      'update-interval-ms': 100,
      'symbols': ['BTCUSD', 'ETHUSD'],
      'data-retention-days': 30,
    },
    auth: {
      'session-timeout-minutes': 60,
      'max-failed-attempts': 5,
      'require-2fa': false,
    },
  };

  generate(): ConfigEntry {
    const namespace = this.randomChoice(ConfigFactory.NAMESPACES);
    const template = ConfigFactory.CONFIG_TEMPLATES[namespace] || {};
    const keys = Object.keys(template);
    const key = keys.length > 0 ? this.randomChoice(keys) : `config-key-${Math.random()}`;
    
    return {
      key,
      value: template[key] || this.generateRandomValue(),
      environment: this.randomChoice(['development', 'testing', 'production'] as const),
      namespace,
      version: this.randomInt(1, 10),
      createdAt: new Date(Date.now() - this.randomInt(0, 86400000 * 30)),
      updatedAt: new Date(Date.now() - this.randomInt(0, 86400000)),
      metadata: {
        author: `test-user-${this.randomInt(1, 10)}`,
        description: this.faker.lorem.sentence(),
        tags: [this.faker.word.noun(), this.faker.word.noun()],
      },
    };
  }

  generateBatch(count: number = this.options.count || 1): ConfigEntry[] {
    return Array.from({ length: count }, () => this.generate());
  }

  generateEdgeCases(): ConfigEntry[] {
    return [
      // Empty value
      { ...this.generate(), value: null },
      { ...this.generate(), value: '' },
      { ...this.generate(), value: [] },
      { ...this.generate(), value: {} },
      
      // Large value
      { ...this.generate(), value: 'x'.repeat(1000000) },
      
      // Special characters in key
      { ...this.generate(), key: 'key with spaces and special chars!@#$%' },
      
      // Future timestamps
      { 
        ...this.generate(), 
        createdAt: new Date(Date.now() + 86400000),
        updatedAt: new Date(Date.now() + 86400000 * 2),
      },
    ];
  }

  private generateRandomValue(): any {
    const types = ['string', 'number', 'boolean', 'object', 'array'];
    const type = this.randomChoice(types);
    
    switch (type) {
      case 'string':
        return this.faker.lorem.sentence();
      case 'number':
        return this.randomFloat(0, 1000000);
      case 'boolean':
        return Math.random() > 0.5;
      case 'object':
        return {
          nested: this.faker.word.noun(),
          value: this.randomInt(1, 100),
          enabled: Math.random() > 0.5,
        };
      case 'array':
        return Array.from({ length: this.randomInt(1, 5) }, () => this.faker.word.noun());
      default:
        return this.faker.lorem.word();
    }
  }
}
```

## 6. Integration Example

### Complete Test Data Setup
```typescript
// tests/helpers/test-data-setup.ts
export class TestDataSetup {
  private marketFactory = new MarketDataFactory();
  private tradeFactory = new TradeFactory();
  private portfolioFactory = new PortfolioFactory();
  private configFactory = new ConfigFactory();

  async setupCompleteTestScenario(): Promise<TestScenario> {
    // Generate market data
    const marketData = this.marketFactory.generateTimeSeries(
      'BTCUSD',
      new Date(Date.now() - 86400000), // 24h ago
      60000, // 1 minute intervals
      1440   // 24 hours worth
    );

    // Generate user trades
    const trades = this.tradeFactory.generateTradingSession(
      'test-user-1',
      86400000, // 24h duration
      50        // 50 trades
    );

    // Generate portfolio
    const portfolio = this.portfolioFactory.generate();

    // Generate config
    const configs = this.configFactory.generateBatch(20);

    return {
      marketData,
      trades,
      portfolio,
      configs,
    };
  }

  generateStressTestData(scale: 'small' | 'medium' | 'large'): StressTestData {
    const scales = {
      small: { users: 10, tradesPerUser: 100, marketTicks: 10000 },
      medium: { users: 100, tradesPerUser: 500, marketTicks: 100000 },
      large: { users: 1000, tradesPerUser: 1000, marketTicks: 1000000 },
    };

    const config = scales[scale];
    
    return {
      marketData: this.marketFactory.generateBatch(config.marketTicks),
      trades: this.generateConcurrentTrades(config.users, config.tradesPerUser),
      portfolios: this.portfolioFactory.generateBatch(config.users),
    };
  }

  private generateConcurrentTrades(userCount: number, tradesPerUser: number): Trade[] {
    const performanceFactory = new PerformanceDataFactory();
    return performanceFactory.generateConcurrentTrades(
      userCount,
      tradesPerUser,
      3600000 // 1 hour window
    );
  }
}

interface TestScenario {
  marketData: MarketDataPoint[];
  trades: Trade[];
  portfolio: Portfolio;
  configs: ConfigEntry[];
}

interface StressTestData {
  marketData: MarketDataPoint[];
  trades: Trade[];
  portfolios: Portfolio[];
}
```

This comprehensive test data generation framework ensures realistic, deterministic, and edge-case testing scenarios for all Neural Trader V2 components.