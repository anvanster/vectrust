# Vectra Performance Comparison Report
==================================================

**Generated:** 2025-09-19 20:55:51
**Rust Results:** 20250729_105420
**Node.js Results:** 2025-07-29T10:25:05.295Z

## 📊 Summary

- **Average Speedup:** 5090.9x 🚀
- **Best Speedup:** 10097.0x 🚀
- **Worst Speedup:** 84.8x 🚀
- **Benchmarks Compared:** 2

## 🔍 Detailed Results

| Benchmark | Rust | Node.js | Speedup |
|-----------|------|---------|---------|
| index_creation | 561.1μs | 47.6ms | 84.8x 🚀 |
| index_loading | 8.2μs | 83.3ms | 10097.0x 🚀 |

## 📈 Performance by Category

**Index Operations:** 5090.9x 🚀 average

## 💡 Key Insights

**Top Performance Gains:**
- index_loading: 10097.0x 🚀
- index_creation: 84.8x 🚀

## 🔧 Implementation Notes

- Rust implementation uses optimized memory-mapped storage and HNSW indexing
- Node.js results are from vectra-enhanced library
- All benchmarks use identical test data and parameters
- Times are averaged across multiple iterations
