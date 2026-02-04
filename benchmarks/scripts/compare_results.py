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

    # Generate and save reports
    report_data = compile_report_data(rust_data, nodejs_data, rust_times, nodejs_times, common_benchmarks)
    save_reports(report_data, output_dir)
    print_summary(report_data)

def compile_report_data(rust_data, nodejs_data, rust_times, nodejs_times, common_benchmarks):
    """Compile all report data into a structured format."""
    speedups = calculate_all_speedups(rust_times, nodejs_times, common_benchmarks)

    return {
        'rust_data': rust_data,
        'nodejs_data': nodejs_data,
        'rust_times': rust_times,
        'nodejs_times': nodejs_times,
        'common_benchmarks': common_benchmarks,
        'speedups': speedups,
        'statistics': calculate_statistics(speedups),
        'categories': categorize_benchmarks(common_benchmarks),
        'best_benchmarks': find_best_performers(rust_times, nodejs_times, common_benchmarks)
    }

def calculate_all_speedups(rust_times, nodejs_times, common_benchmarks):
    """Calculate speedup for all common benchmarks."""
    speedups = []
    for benchmark in common_benchmarks:
        rust_time = rust_times[benchmark]
        nodejs_time = nodejs_times[benchmark]
        speedup = calculate_speedup(rust_time, nodejs_time)
        if speedup != float('inf'):
            speedups.append(speedup)
    return speedups

def calculate_statistics(speedups):
    """Calculate summary statistics for speedups."""
    if not speedups:
        return {'avg': 0, 'max': 0, 'min': 0}
    return {
        'avg': sum(speedups) / len(speedups),
        'max': max(speedups),
        'min': min(speedups)
    }

def categorize_benchmarks(common_benchmarks):
    """Categorize benchmarks by type."""
    return {
        'index': [b for b in common_benchmarks if 'index' in b],
        'insert': [b for b in common_benchmarks if 'insert' in b],
        'search': [b for b in common_benchmarks if 'search' in b],
        'scale': [b for b in common_benchmarks if 'scale' in b],
        'batch': [b for b in common_benchmarks if 'batch' in b]
    }

def find_best_performers(rust_times, nodejs_times, common_benchmarks):
    """Find the best performing benchmarks."""
    return sorted(
        [(b, calculate_speedup(rust_times[b], nodejs_times[b])) for b in common_benchmarks],
        key=lambda x: x[1], reverse=True
    )[:3]

def save_reports(report_data, output_dir):
    """Save markdown and JSON reports."""
    markdown_content = generate_markdown_report(report_data)
    json_content = generate_json_report(report_data)

    # Save markdown report
    report_path = Path(output_dir) / f"performance_comparison_{datetime.now().strftime('%Y%m%d_%H%M%S')}.md"
    with open(report_path, 'w') as f:
        f.write(markdown_content)

    # Save JSON report
    json_path = Path(output_dir) / f"performance_comparison_{datetime.now().strftime('%Y%m%d_%H%M%S')}.json"
    with open(json_path, 'w') as f:
        json.dump(json_content, f, indent=2)

    report_data['report_path'] = report_path
    report_data['json_path'] = json_path

def generate_markdown_report(report_data):
    """Generate the markdown report content."""
    lines = []

    # Add header
    lines.extend(generate_report_header(report_data))

    # Add summary if statistics available
    if report_data['speedups']:
        lines.extend(generate_summary_section(report_data))

    # Add detailed results
    lines.extend(generate_detailed_results(report_data))

    # Add category analysis
    lines.extend(generate_category_analysis(report_data))

    # Add insights
    lines.extend(generate_insights_section(report_data))

    # Add implementation notes
    lines.extend(generate_implementation_notes())

    return "\n".join(lines)

def generate_report_header(report_data):
    """Generate report header."""
    return [
        "# Vectra Performance Comparison Report",
        "=" * 50,
        "",
        f"**Generated:** {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}",
        f"**Rust Results:** {report_data['rust_data'].get('timestamp', 'Unknown')}",
        f"**Node.js Results:** {report_data['nodejs_data'].get('timestamp', 'Unknown')}",
        ""
    ]

def generate_summary_section(report_data):
    """Generate summary statistics section."""
    stats = report_data['statistics']
    lines = [
        "## 📊 Summary",
        "",
        f"- **Average Speedup:** {format_speedup(stats['avg'])}",
        f"- **Best Speedup:** {format_speedup(stats['max'])}",
        f"- **Worst Speedup:** {format_speedup(stats['min'])}",
        f"- **Benchmarks Compared:** {len(report_data['common_benchmarks'])}",
        ""
    ]
    return lines

def generate_detailed_results(report_data):
    """Generate detailed results table."""
    lines = [
        "## 🔍 Detailed Results",
        "",
        "| Benchmark | Rust | Node.js | Speedup |",
        "|-----------|------|---------|---------|"]

    sorted_benchmarks = sorted(report_data['common_benchmarks'])

    for benchmark in sorted_benchmarks:
        rust_time = report_data['rust_times'][benchmark]
        nodejs_time = report_data['nodejs_times'][benchmark]
        speedup = calculate_speedup(rust_time, nodejs_time)

        lines.append(f"| {benchmark} | {format_time(rust_time)} | {format_time(nodejs_time)} | {format_speedup(speedup)} |")

    lines.append("")
    return lines

def generate_category_analysis(report_data):
    """Generate category performance analysis."""
    lines = ["## 📈 Performance by Category", ""]

    for category, benchmarks in report_data['categories'].items():
        if not benchmarks:
            continue

        category_speedups = []
        for benchmark in benchmarks:
            rust_time = report_data['rust_times'][benchmark]
            nodejs_time = report_data['nodejs_times'][benchmark]
            speedup = calculate_speedup(rust_time, nodejs_time)
            if speedup != float('inf'):
                category_speedups.append(speedup)

        if category_speedups:
            avg_speedup = sum(category_speedups) / len(category_speedups)
            lines.append(f"**{category.title()} Operations:** {format_speedup(avg_speedup)} average")

    lines.append("")
    return lines

def generate_insights_section(report_data):
    """Generate insights section."""
    lines = ["## 💡 Key Insights", "", "**Top Performance Gains:**"]

    for benchmark, speedup in report_data['best_benchmarks']:
        if speedup != float('inf'):
            lines.append(f"- {benchmark}: {format_speedup(speedup)}")

    lines.append("")

    # Check for areas needing improvement
    slow_benchmarks = [(b, s) for b, s in report_data['best_benchmarks'] if s < 1.0]
    if slow_benchmarks:
        lines.append("**Areas for Improvement:**")
        for benchmark, speedup in slow_benchmarks[:3]:
            lines.append(f"- {benchmark}: {format_speedup(speedup)}")
        lines.append("")

    return lines

def generate_implementation_notes():
    """Generate implementation notes section."""
    return [
        "## 🔧 Implementation Notes",
        "",
        "- Rust implementation uses optimized memory-mapped storage and HNSW indexing",
        "- Node.js results are from vectra-enhanced library",
        "- All benchmarks use identical test data and parameters",
        "- Times are averaged across multiple iterations",
        ""
    ]

def generate_json_report(report_data):
    """Generate JSON report content."""
    stats = report_data['statistics']
    return {
        'timestamp': datetime.now().isoformat(),
        'rust_timestamp': report_data['rust_data'].get('timestamp'),
        'nodejs_timestamp': report_data['nodejs_data'].get('timestamp'),
        'summary': {
            'average_speedup': stats['avg'],
            'max_speedup': stats['max'],
            'min_speedup': stats['min'],
            'benchmarks_compared': len(report_data['common_benchmarks'])
        },
        'detailed_results': {
            benchmark: {
                'rust_time': report_data['rust_times'][benchmark],
                'nodejs_time': report_data['nodejs_times'][benchmark],
                'speedup': calculate_speedup(report_data['rust_times'][benchmark], report_data['nodejs_times'][benchmark])
            }
            for benchmark in report_data['common_benchmarks']
        }
    }

def print_summary(report_data):
    """Print summary to console."""
    print("📊 Performance Comparison Summary")
    print("=" * 40)

    if report_data['speedups']:
        stats = report_data['statistics']
        print(f"Average Speedup: {format_speedup(stats['avg'])}")
        print(f"Best Speedup: {format_speedup(stats['max'])}")
        print(f"Benchmarks: {len(report_data['common_benchmarks'])}")

    print(f"\n📄 Full report saved to: {report_data['report_path']}")
    print(f"📄 JSON data saved to: {report_data['json_path']}")

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