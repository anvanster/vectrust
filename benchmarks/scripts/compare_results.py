#!/usr/bin/env python3
"""
Vectra Performance Comparison Tool

Analyzes benchmark results from both Rust and Node.js implementations
and generates detailed performance comparison reports.
"""

import json
import glob
import os
import sys
from pathlib import Path
from datetime import datetime
import argparse

def load_benchmark_results(results_dir):
    """Load all benchmark result files from the results directory."""
    rust_results = []
    nodejs_results = []
    
    results_path = Path(results_dir)
    
    # Load Rust results
    rust_files = list(results_path.glob("rust_benchmark_*.json"))
    for file_path in rust_files:
        try:
            with open(file_path, 'r') as f:
                data = json.load(f)
                rust_results.append(data)
        except Exception as e:
            print(f"⚠️  Warning: Could not load {file_path}: {e}")
    
    # Load Node.js results
    nodejs_files = list(results_path.glob("nodejs_benchmark_*.json"))
    for file_path in nodejs_files:
        try:
            with open(file_path, 'r') as f:
                data = json.load(f)
                nodejs_results.append(data)
        except Exception as e:
            print(f"⚠️  Warning: Could not load {file_path}: {e}")
    
    return rust_results, nodejs_results

def get_latest_results(results_list):
    """Get the most recent benchmark results."""
    if not results_list:
        return None
    
    # Sort by timestamp and return the latest
    sorted_results = sorted(results_list, key=lambda x: x.get('timestamp', ''), reverse=True)
    return sorted_results[0]

def calculate_speedup(rust_time, nodejs_time):
    """Calculate speedup factor (how many times faster Rust is)."""
    if nodejs_time == 0:
        return float('inf')
    return nodejs_time / rust_time

def format_time(seconds):
    """Format time in appropriate units."""
    if seconds < 0.001:
        return f"{seconds * 1000000:.1f}μs"
    elif seconds < 1.0:
        return f"{seconds * 1000:.1f}ms"
    else:
        return f"{seconds:.3f}s"

def format_speedup(speedup):
    """Format speedup value with appropriate precision and emoji."""
    if speedup == float('inf'):
        return "∞x 🚀"
    elif speedup >= 10:
        return f"{speedup:.1f}x 🚀"
    elif speedup >= 2:
        return f"{speedup:.1f}x ⚡"
    elif speedup >= 1.5:
        return f"{speedup:.1f}x 📈"
    elif speedup >= 1.1:
        return f"{speedup:.2f}x ➕"
    elif speedup >= 0.9:
        return f"{speedup:.2f}x ≈"
    else:
        return f"{speedup:.2f}x 📉"

def generate_comparison_report(rust_results, nodejs_results, output_dir):
    """Generate a detailed comparison report."""
    if not rust_results or not nodejs_results:
        print("❌ Cannot generate comparison: missing results from one or both implementations")
        return
    
    rust_data = get_latest_results(rust_results)
    nodejs_data = get_latest_results(nodejs_results)
    
    if not rust_data or not nodejs_data:
        print("❌ Cannot find valid benchmark data")
        return
    
    rust_times = rust_data['results']
    nodejs_times = nodejs_data['results']
    
    # Find common benchmarks
    common_benchmarks = set(rust_times.keys()) & set(nodejs_times.keys())
    
    if not common_benchmarks:
        print("❌ No common benchmarks found between Rust and Node.js results")
        return
    
    # Generate report
    report_lines = []
    report_lines.append("# Vectra Performance Comparison Report")
    report_lines.append("=" * 50)
    report_lines.append("")
    report_lines.append(f"**Generated:** {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
    report_lines.append(f"**Rust Results:** {rust_data.get('timestamp', 'Unknown')}")
    report_lines.append(f"**Node.js Results:** {nodejs_data.get('timestamp', 'Unknown')}")
    report_lines.append("")
    
    # Summary statistics
    speedups = []
    for benchmark in common_benchmarks:
        rust_time = rust_times[benchmark]
        nodejs_time = nodejs_times[benchmark]
        speedup = calculate_speedup(rust_time, nodejs_time)
        if speedup != float('inf'):
            speedups.append(speedup)
    
    if speedups:
        avg_speedup = sum(speedups) / len(speedups)
        max_speedup = max(speedups)
        min_speedup = min(speedups)
        
        report_lines.append("## 📊 Summary")
        report_lines.append("")
        report_lines.append(f"- **Average Speedup:** {format_speedup(avg_speedup)}")
        report_lines.append(f"- **Best Speedup:** {format_speedup(max_speedup)}")
        report_lines.append(f"- **Worst Speedup:** {format_speedup(min_speedup)}")
        report_lines.append(f"- **Benchmarks Compared:** {len(common_benchmarks)}")
        report_lines.append("")
    
    # Detailed comparison table
    report_lines.append("## 🔍 Detailed Results")
    report_lines.append("")
    report_lines.append("| Benchmark | Rust | Node.js | Speedup |")
    report_lines.append("|-----------|------|---------|---------|")
    
    # Sort benchmarks by category for better readability
    sorted_benchmarks = sorted(common_benchmarks)
    
    for benchmark in sorted_benchmarks:
        rust_time = rust_times[benchmark]
        nodejs_time = nodejs_times[benchmark]
        speedup = calculate_speedup(rust_time, nodejs_time)
        
        report_lines.append(f"| {benchmark} | {format_time(rust_time)} | {format_time(nodejs_time)} | {format_speedup(speedup)} |")
    
    report_lines.append("")
    
    # Category analysis
    categories = {
        'index': [b for b in common_benchmarks if 'index' in b],
        'insert': [b for b in common_benchmarks if 'insert' in b],
        'search': [b for b in common_benchmarks if 'search' in b],
        'scale': [b for b in common_benchmarks if 'scale' in b],
        'batch': [b for b in common_benchmarks if 'batch' in b]
    }
    
    report_lines.append("## 📈 Performance by Category")
    report_lines.append("")
    
    for category, benchmarks in categories.items():
        if not benchmarks:
            continue
            
        category_speedups = []
        for benchmark in benchmarks:
            rust_time = rust_times[benchmark]
            nodejs_time = nodejs_times[benchmark]
            speedup = calculate_speedup(rust_time, nodejs_time)
            if speedup != float('inf'):
                category_speedups.append(speedup)
        
        if category_speedups:
            avg_speedup = sum(category_speedups) / len(category_speedups)
            report_lines.append(f"**{category.title()} Operations:** {format_speedup(avg_speedup)} average")
    
    report_lines.append("")
    
    # Performance insights
    report_lines.append("## 💡 Key Insights")
    report_lines.append("")
    
    # Find best performing operations
    best_benchmarks = sorted(
        [(b, calculate_speedup(rust_times[b], nodejs_times[b])) for b in common_benchmarks],
        key=lambda x: x[1], reverse=True
    )[:3]
    
    report_lines.append("**Top Performance Gains:**")
    for benchmark, speedup in best_benchmarks:
        if speedup != float('inf'):
            report_lines.append(f"- {benchmark}: {format_speedup(speedup)}")
    
    report_lines.append("")
    
    # Find areas needing improvement (if any)
    slow_benchmarks = [(b, s) for b, s in best_benchmarks if s < 1.0]
    if slow_benchmarks:
        report_lines.append("**Areas for Improvement:**")
        for benchmark, speedup in slow_benchmarks[:3]:
            report_lines.append(f"- {benchmark}: {format_speedup(speedup)}")
        report_lines.append("")
    
    # Implementation notes
    report_lines.append("## 🔧 Implementation Notes")
    report_lines.append("")
    report_lines.append("- Rust implementation uses optimized memory-mapped storage and HNSW indexing")
    report_lines.append("- Node.js results are from vectra-enhanced library")
    report_lines.append("- All benchmarks use identical test data and parameters")
    report_lines.append("- Times are averaged across multiple iterations")
    report_lines.append("")
    
    # Save report
    report_content = "\n".join(report_lines)
    
    # Save as markdown
    report_path = Path(output_dir) / f"performance_comparison_{datetime.now().strftime('%Y%m%d_%H%M%S')}.md"
    with open(report_path, 'w') as f:
        f.write(report_content)
    
    # Also save as JSON for programmatic access
    json_report = {
        'timestamp': datetime.now().isoformat(),
        'rust_timestamp': rust_data.get('timestamp'),
        'nodejs_timestamp': nodejs_data.get('timestamp'),
        'summary': {
            'average_speedup': avg_speedup if speedups else 0,
            'max_speedup': max_speedup if speedups else 0,
            'min_speedup': min_speedup if speedups else 0,
            'benchmarks_compared': len(common_benchmarks)
        },
        'detailed_results': {
            benchmark: {
                'rust_time': rust_times[benchmark],
                'nodejs_time': nodejs_times[benchmark],
                'speedup': calculate_speedup(rust_times[benchmark], nodejs_times[benchmark])
            }
            for benchmark in common_benchmarks
        }
    }
    
    json_path = Path(output_dir) / f"performance_comparison_{datetime.now().strftime('%Y%m%d_%H%M%S')}.json"
    with open(json_path, 'w') as f:
        json.dump(json_report, f, indent=2)
    
    # Print summary to console
    print("📊 Performance Comparison Summary")
    print("=" * 40)
    if speedups:
        print(f"Average Speedup: {format_speedup(avg_speedup)}")
        print(f"Best Speedup: {format_speedup(max_speedup)}")
        print(f"Benchmarks: {len(common_benchmarks)}")
    print(f"\n📄 Full report saved to: {report_path}")
    print(f"📄 JSON data saved to: {json_path}")

def main():
    parser = argparse.ArgumentParser(description='Compare Vectra benchmark results')
    parser.add_argument('results_dir', help='Directory containing benchmark results')
    parser.add_argument('--verbose', '-v', action='store_true', help='Verbose output')
    
    args = parser.parse_args()
    
    if not os.path.exists(args.results_dir):
        print(f"❌ Results directory not found: {args.results_dir}")
        sys.exit(1)
    
    try:
        rust_results, nodejs_results = load_benchmark_results(args.results_dir)
        
        if args.verbose:
            print(f"📁 Found {len(rust_results)} Rust result files")
            print(f"📁 Found {len(nodejs_results)} Node.js result files")
        
        generate_comparison_report(rust_results, nodejs_results, args.results_dir)
        
    except Exception as e:
        print(f"❌ Error generating comparison report: {e}")
        if args.verbose:
            import traceback
            traceback.print_exc()
        sys.exit(1)

if __name__ == '__main__':
    main()