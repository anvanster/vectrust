#!/usr/bin/env node

const { Command } = require('commander');
const fs = require('fs-extra');
const path = require('path');
const { v4: uuidv4 } = require('uuid');
const cliProgress = require('cli-progress');

// Import vectrust
let LocalIndex;
try {
    const vectrust = require('vectrust');
    LocalIndex = vectrust.LocalIndex;
} catch (error) {
    console.warn('⚠️  vectrust not found. Please install with: npm install');
    console.warn('   Using simulation mode for testing...');
    LocalIndex = require('./vectra-sim');
}

const TestDataGenerator = require('./test-data');
const BenchmarkSuite = require('./benchmark-suite');

const program = new Command();

program
    .name('vectra-nodejs-benchmark')
    .description('Comprehensive Vectra Node.js benchmarks')
    .version('1.0.0')
    .option('-o, --output <path>', 'Output directory for results', '../results')
    .option('-v, --vectors <number>', 'Number of vectors to test with', '10000')
    .option('-d, --dimensions <number>', 'Vector dimensions', '384')
    .option('-b, --benchmark <type>', 'Specific benchmark to run')
    .option('-i, --iterations <number>', 'Number of iterations for timing', '5')
    .option('--legacy', 'Use legacy storage format')
    .option('--verbose', 'Verbose output');

program.parse();

const options = program.opts();

async function main() {
    console.log('🟢 Vectra Node.js Benchmark Suite');
    console.log('==================================');
    console.log(`Vectors: ${options.vectors}`);
    console.log(`Dimensions: ${options.dimensions}`);
    console.log(`Iterations: ${options.iterations}`);
    console.log(`Storage: ${options.legacy ? 'Legacy JSON' : 'Default'}`);
    console.log();
    
    const results = new BenchmarkResults();
    
    try {
        switch (options.benchmark) {
            case 'index':
                await runIndexBenchmarks(options, results);
                break;
            case 'items':
                await runItemBenchmarks(options, results);
                break;
            case 'search':
                await runSearchBenchmarks(options, results);
                break;
            case 'scale':
                await runScaleBenchmarks(options, results);
                break;
            default:
                // Run full suite
                await runIndexBenchmarks(options, results);
                await runItemBenchmarks(options, results);
                await runSearchBenchmarks(options, results);
                await runScaleBenchmarks(options, results);
                break;
        }
        
        await results.save(options.output);
        results.printSummary();
        
    } catch (error) {
        console.error('❌ Benchmark failed:', error.message);
        if (options.verbose) {
            console.error(error.stack);
        }
        process.exit(1);
    }
}

async function runIndexBenchmarks(options, results) {
    console.log('📁 Running Index Operation Benchmarks');
    console.log('======================================');
    
    const suite = new BenchmarkSuite.IndexBenchmarkSuite(options.legacy);
    
    // Index creation
    const creationTime = await timeOperation(
        'Index Creation',
        parseInt(options.iterations),
        () => suite.benchmarkIndexCreation(parseInt(options.vectors), parseInt(options.dimensions))
    );
    results.add('index_creation', creationTime);
    
    // Index loading  
    const loadingTime = await timeOperation(
        'Index Loading',
        parseInt(options.iterations),
        () => suite.benchmarkIndexLoading()
    );
    results.add('index_loading', loadingTime);
    
    console.log();
}

async function runItemBenchmarks(options, results) {
    console.log('📝 Running Item Operation Benchmarks');
    console.log('=====================================');
    
    const suite = new BenchmarkSuite.ItemBenchmarkSuite(options.legacy);
    const testData = new TestDataGenerator(parseInt(options.dimensions));
    const vectors = testData.generateVectors(parseInt(options.vectors));
    
    // Single item operations
    const insertTime = await timeOperation(
        'Single Item Insert',
        parseInt(options.iterations),
        () => suite.benchmarkSingleInsert(vectors[0])
    );
    results.add('single_insert', insertTime);
    
    const getTime = await timeOperation(
        'Single Item Get', 
        parseInt(options.iterations),
        () => suite.benchmarkSingleGet()
    );
    results.add('single_get', getTime);
    
    // Batch operations
    const batchSizes = [100, 1000, 5000];
    for (const batchSize of batchSizes) {
        if (batchSize <= parseInt(options.vectors)) {
            const batch = vectors.slice(0, batchSize);
            const batchTime = await timeOperation(
                `Batch Insert (${batchSize})`,
                parseInt(options.iterations),
                () => suite.benchmarkBatchInsert(batch)
            );
            results.add(`batch_insert_${batchSize}`, batchTime);
        }
    }
    
    console.log();
}

async function runSearchBenchmarks(options, results) {
    console.log('🔍 Running Vector Search Benchmarks');
    console.log('====================================');
    
    const suite = new BenchmarkSuite.SearchBenchmarkSuite(options.legacy);
    const testData = new TestDataGenerator(parseInt(options.dimensions));
    
    // Setup index with data
    await suite.setupIndex(parseInt(options.vectors), parseInt(options.dimensions));
    
    const queryVectors = testData.generateVectors(100);
    
    // Single vector search
    const searchTime = await timeOperation(
        'Single Vector Search',
        parseInt(options.iterations),
        () => suite.benchmarkSingleSearch(queryVectors[0], 10)
    );
    results.add('single_search', searchTime);
    
    // Batch search
    const batchSearchTime = await timeOperation(
        'Batch Vector Search (100 queries)',
        parseInt(options.iterations), 
        () => suite.benchmarkBatchSearch(queryVectors, 10)
    );
    results.add('batch_search', batchSearchTime);
    
    // Different K values
    for (const k of [1, 5, 10, 50, 100]) {
        const kTime = await timeOperation(
            `Search Top-${k}`,
            parseInt(options.iterations),
            () => suite.benchmarkSingleSearch(queryVectors[0], k)
        );
        results.add(`search_top_${k}`, kTime);
    }
    
    console.log();
}

async function runScaleBenchmarks(options, results) {
    console.log('📊 Running Scale Benchmarks');
    console.log('============================');
    
    const suite = new BenchmarkSuite.ScaleBenchmarkSuite(options.legacy);
    const testData = new TestDataGenerator(parseInt(options.dimensions));
    
    // Test different dataset sizes
    const datasetSizes = [1000, 5000, 10000, 25000];
    
    for (const size of datasetSizes) {
        if (size <= parseInt(options.vectors) * 3) {
            console.log(`Testing with ${size} vectors...`);
            
            const vectors = testData.generateVectors(size);
            
            // Index creation time
            const creationTime = await timeOperation(
                `Index Creation (${size})`,
                1,
                () => suite.benchmarkIndexCreationWithData(vectors)
            );
            results.add(`scale_creation_${size}`, creationTime);
            
            // Search performance
            const query = testData.generateVectors(1);
            const searchTime = await timeOperation(
                `Search Performance (${size})`,
                parseInt(options.iterations),
                () => suite.benchmarkSearchPerformance(query[0], 10)
            );
            results.add(`scale_search_${size}`, searchTime);
        }
    }
    
    console.log();
}

async function timeOperation(name, iterations, operation) {
    const progressBar = new cliProgress.SingleBar({
        format: `${name} [{bar}] {percentage}% | {value}/{total} | ETA: {eta}s`,
        barCompleteChar: '\u2588',
        barIncompleteChar: '\u2591',
        hideCursor: true
    });
    
    progressBar.start(iterations, 0);
    
    let totalTime = 0;
    
    for (let i = 0; i < iterations; i++) {
        const start = process.hrtime.bigint();
        await operation();
        const end = process.hrtime.bigint();
        
        totalTime += Number(end - start) / 1000000; // Convert to milliseconds
        progressBar.increment();
    }
    
    progressBar.stop();
    
    const averageTime = totalTime / iterations;
    console.log(`${name} - Average: ${averageTime.toFixed(3)}ms`);
    
    return averageTime / 1000; // Return in seconds to match Rust benchmarks
}

class BenchmarkResults {
    constructor() {
        this.results = new Map();
    }
    
    add(name, time) {
        this.results.set(name, time);
    }
    
    async save(outputDir) {
        await fs.ensureDir(outputDir);
        
        const timestamp = new Date().toISOString().replace(/[:.]/g, '-').split('T')[0] + '_' + 
                         new Date().toISOString().replace(/[:.]/g, '-').split('T')[1].split('.')[0];
        const filename = path.join(outputDir, `nodejs_benchmark_${timestamp}.json`);
        
        const jsonData = {
            timestamp: new Date().toISOString(),
            implementation: 'nodejs',
            results: Object.fromEntries(this.results)
        };
        
        await fs.writeFile(filename, JSON.stringify(jsonData, null, 2));
        console.log(`📄 Results saved to: ${filename}`);
    }
    
    printSummary() {
        console.log('📈 Benchmark Summary');
        console.log('===================');
        
        const sortedResults = Array.from(this.results.entries()).sort((a, b) => a[0].localeCompare(b[0]));
        
        for (const [name, time] of sortedResults) {
            console.log(`${name.padEnd(30)} ${(time * 1000).toFixed(3).padStart(10)}ms`);
        }
    }
}

if (require.main === module) {
    main().catch(console.error);
}

module.exports = { timeOperation, BenchmarkResults };