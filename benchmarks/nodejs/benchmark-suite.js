const fs = require('fs-extra');
const path = require('path');
const os = require('os');
const TestDataGenerator = require('./test-data');

// Try to import vectrust, fallback to simulation if not available
let LocalIndex;
try {
    const vectrust = require('vectrust');
    LocalIndex = vectrust.LocalIndex;
} catch (error) {
    // Fallback simulation for testing the benchmark structure
    LocalIndex = class {
        constructor(folderPath, indexName) {
            this.folderPath = folderPath;
            this.indexName = indexName || 'index.json';
            this.items = [];
        }
        
        async createIndex(config) {
            await fs.ensureDir(this.folderPath);
            const indexPath = path.join(this.folderPath, this.indexName);
            await fs.writeJson(indexPath, { version: 1, items: [] });
        }
        
        async isIndexCreated() {
            const indexPath = path.join(this.folderPath, this.indexName);
            return await fs.pathExists(indexPath);
        }
        
        async insertItem(item) {
            this.items.push(item);
            return item;
        }
        
        async getItem(id) {
            return this.items.find(item => item.id === id) || null;
        }
        
        async listItems() {
            return this.items;
        }
        
        async queryItems(vector, topK) {
            // Simple simulation - return random items with scores
            const results = this.items.slice(0, topK || 10).map(item => ({
                item,
                score: Math.random()
            }));
            return results.sort((a, b) => b.score - a.score);
        }
        
        async updateItem(updateRequest) {
            const item = this.items.find(item => item.id === updateRequest.id);
            if (item) {
                if (updateRequest.vector) item.vector = updateRequest.vector;
                if (updateRequest.metadata) Object.assign(item.metadata, updateRequest.metadata);
                item.version++;
                item.updatedAt = new Date().toISOString();
            }
            return { id: updateRequest.id, version: item?.version || 1 };
        }
        
        async deleteItem(id) {
            const index = this.items.findIndex(item => item.id === id);
            if (index !== -1) {
                this.items.splice(index, 1);
            }
        }
    };
    
    console.log('📝 Using vectrust simulation (vectrust not found)');
}

class IndexBenchmarkSuite {
    constructor(legacyFormat) {
        this.legacyFormat = legacyFormat;
    }
    
    async benchmarkIndexCreation(vectorCount, dimensions) {
        const tempDir = await fs.mkdtemp(path.join(os.tmpdir(), 'vectra-bench-'));
        const index = new LocalIndex(tempDir);
        
        await index.createIndex(JSON.stringify({
            delete_if_exists: true
        }));
        
        // Add some vectors to make it realistic
        const testData = new TestDataGenerator(dimensions);
        const vectors = testData.generateVectors(Math.min(vectorCount, 1000));
        
        for (const vector of vectors) {
            await index.insertItem(JSON.stringify(vector));
        }
        
        // Clean up
        await fs.remove(tempDir);
    }
    
    async benchmarkIndexLoading() {
        const tempDir = await fs.mkdtemp(path.join(os.tmpdir(), 'vectra-bench-'));
        const index = new LocalIndex(tempDir);
        
        await index.createIndex();
        
        // Add some data
        const testData = new TestDataGenerator(384);
        const vectors = testData.generateVectors(100);
        
        for (const vector of vectors) {
            await index.insertItem(JSON.stringify(vector));
        }
        
        // Create new index instance (simulates loading)
        const loadedIndex = new LocalIndex(tempDir);
        const exists = await loadedIndex.isIndexCreated();
        
        // Clean up
        await fs.remove(tempDir);
        
        if (!exists) {
            throw new Error('Index loading failed');
        }
    }
}

class ItemBenchmarkSuite {
    constructor(legacyFormat) {
        this.legacyFormat = legacyFormat;
        this.tempDir = null;
        this.index = null;
        this.insertedItems = [];
        this.setupComplete = false;
    }
    
    async setup() {
        if (!this.setupComplete) {
            this.tempDir = await fs.mkdtemp(path.join(os.tmpdir(), 'vectra-bench-'));
            this.index = new LocalIndex(this.tempDir);
            await this.index.createIndex();
            this.setupComplete = true;
        }
    }
    
    async benchmarkSingleInsert(item) {
        await this.setup();
        const insertedJson = await this.index.insertItem(JSON.stringify(item));
        const inserted = JSON.parse(insertedJson);
        this.insertedItems.push(inserted);
    }
    
    async benchmarkSingleGet() {
        if (this.insertedItems.length === 0) {
            throw new Error('No items to get');
        }
        
        const item = this.insertedItems[0];
        await this.index.getItem(item.id);
    }
    
    async benchmarkBatchInsert(items) {
        await this.setup();
        
        for (const item of items) {
            const insertedJson = await this.index.insertItem(JSON.stringify(item));
        const inserted = JSON.parse(insertedJson);
            this.insertedItems.push(inserted);
        }
    }
    
    async benchmarkItemUpdate(updatedItem) {
        if (this.insertedItems.length === 0) {
            throw new Error('No items to update');
        }
        
        const existing = this.insertedItems[0];
        await this.index.updateItem({
            id: existing.id,
            vector: updatedItem.vector,
            metadata: updatedItem.metadata
        });
    }
    
    async benchmarkItemDeletion() {
        if (this.insertedItems.length === 0) {
            throw new Error('No items to delete');
        }
        
        const item = this.insertedItems.pop();
        await this.index.deleteItem(item.id);
    }
    
    async cleanup() {
        if (this.tempDir) {
            await fs.remove(this.tempDir);
        }
    }
}

class SearchBenchmarkSuite {
    constructor(legacyFormat) {
        this.legacyFormat = legacyFormat;
        this.tempDir = null;
        this.index = null;
        this.setupComplete = false;
    }
    
    async setupIndex(vectorCount, dimensions) {
        this.tempDir = await fs.mkdtemp(path.join(os.tmpdir(), 'vectra-bench-'));
        this.index = new LocalIndex(this.tempDir);
        
        await this.index.createIndex();
        
        const testData = new TestDataGenerator(dimensions);
        const vectors = testData.generateVectors(vectorCount);
        
        for (const vector of vectors) {
            await this.index.insertItem(vector);
        }
        
        this.setupComplete = true;
    }
    
    async benchmarkSingleSearch(queryVector, k) {
        if (!this.setupComplete) {
            throw new Error('Index not set up');
        }
        
        await this.index.queryItems(queryVector.vector, k);
    }
    
    async benchmarkBatchSearch(queryVectors, k) {
        for (const query of queryVectors) {
            await this.benchmarkSingleSearch(query, k);
        }
    }
    
    async benchmarkFilteredSearch(queryVector, k, filter) {
        if (!this.setupComplete) {
            throw new Error('Index not set up');
        }
        
        // Note: This depends on the actual vectra-enhanced API for filtered search
        // The simulation doesn't support filters
        await this.index.queryItems(queryVector.vector, k);
    }
    
    async cleanup() {
        if (this.tempDir) {
            await fs.remove(this.tempDir);
        }
    }
}

class ScaleBenchmarkSuite {
    constructor(legacyFormat) {
        this.legacyFormat = legacyFormat;
    }
    
    async benchmarkIndexCreationWithData(vectors) {
        const tempDir = await fs.mkdtemp(path.join(os.tmpdir(), 'vectra-bench-'));
        const index = new LocalIndex(tempDir);
        
        await index.createIndex();
        
        for (const vector of vectors) {
            await index.insertItem(JSON.stringify(vector));
        }
        
        // Clean up
        await fs.remove(tempDir);
    }
    
    async benchmarkSearchPerformance(queryVector, k) {
        // This would typically use a pre-setup index
        // For simplicity, we'll create a minimal one
        const tempDir = await fs.mkdtemp(path.join(os.tmpdir(), 'vectra-bench-'));
        const index = new LocalIndex(tempDir);
        
        await index.createIndex();
        
        // Add a few items for the search to work
        const testData = new TestDataGenerator(queryVector.vector.length);
        const vectors = testData.generateVectors(100);
        
        for (const vector of vectors) {
            await index.insertItem(JSON.stringify(vector));
        }
        
        await index.queryItems(queryVector.vector, k);
        
        // Clean up
        await fs.remove(tempDir);
    }
    
    async benchmarkConcurrentOperations(vectors) {
        const tempDir = await fs.mkdtemp(path.join(os.tmpdir(), 'vectra-bench-'));
        const index = new LocalIndex(tempDir);
        
        await index.createIndex();
        
        // Simulate concurrent operations by processing in chunks
        const chunkSize = 100;
        const promises = [];
        
        for (let i = 0; i < vectors.length; i += chunkSize) {
            const chunk = vectors.slice(i, i + chunkSize);
            const promise = (async () => {
                for (const vector of chunk) {
                    await index.insertItem(vector);
                }
            })();
            promises.push(promise);
        }
        
        await Promise.all(promises);
        
        // Clean up
        await fs.remove(tempDir);
    }
}

class ConcurrencyBenchmarkSuite {
    constructor(legacyFormat) {
        this.legacyFormat = legacyFormat;
        this.tempDir = null;
        this.index = null;
    }
    
    async setup(vectorCount, dimensions) {
        this.tempDir = await fs.mkdtemp(path.join(os.tmpdir(), 'vectra-bench-'));
        this.index = new LocalIndex(this.tempDir);
        
        await this.index.createIndex();
        
        const testData = new TestDataGenerator(dimensions);
        const vectors = testData.generateVectors(vectorCount);
        
        for (const vector of vectors) {
            await this.index.insertItem(vector);
        }
    }
    
    async benchmarkConcurrentReads(queryVectors, numThreads) {
        const queriesPerThread = Math.floor(queryVectors.length / numThreads);
        const promises = [];
        
        for (let i = 0; i < numThreads; i++) {
            const startIdx = i * queriesPerThread;
            const endIdx = i === numThreads - 1 ? queryVectors.length : (i + 1) * queriesPerThread;
            const queries = queryVectors.slice(startIdx, endIdx);
            
            const promise = (async () => {
                for (const query of queries) {
                    await this.index.queryItems(query.vector, 10);
                }
            })();
            
            promises.push(promise);
        }
        
        await Promise.all(promises);
    }
    
    async benchmarkMixedWorkload(readVectors, writeVectors) {
        const readPromises = readVectors.map(async (query) => {
            return this.index.queryItems(query.vector, 10);
        });
        
        const writePromises = writeVectors.map(async (item) => {
            return this.index.insertItem(item);
        });
        
        await Promise.all([...readPromises, ...writePromises]);
    }
    
    async cleanup() {
        if (this.tempDir) {
            await fs.remove(this.tempDir);
        }
    }
}

module.exports = {
    IndexBenchmarkSuite,
    ItemBenchmarkSuite,
    SearchBenchmarkSuite,
    ScaleBenchmarkSuite,
    ConcurrencyBenchmarkSuite
};