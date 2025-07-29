const { v4: uuidv4 } = require('uuid');

class TestDataGenerator {
    constructor(dimensions) {
        this.dimensions = dimensions;
        this.seed = 42; // Fixed seed for reproducible benchmarks
        this._random = this._seededRandom(this.seed);
    }
    
    // Simple seeded random number generator for reproducible tests
    _seededRandom(seed) {
        let x = Math.sin(seed) * 10000;
        return () => {
            x = Math.sin(x) * 10000;
            return x - Math.floor(x);
        };
    }
    
    generateVectors(count) {
        const vectors = [];
        for (let i = 0; i < count; i++) {
            vectors.push(this.generateVector(i));
        }
        return vectors;
    }
    
    generateVector(index) {
        // Generate random vector
        const vector = [];
        for (let i = 0; i < this.dimensions; i++) {
            vector.push((this._random() - 0.5) * 2); // Range -1 to 1
        }
        
        // Normalize the vector
        const norm = Math.sqrt(vector.reduce((sum, x) => sum + x * x, 0));
        const normalizedVector = norm > 0 ? vector.map(x => x / norm) : vector;
        
        return {
            id: uuidv4(),
            vector: normalizedVector,
            metadata: this.generateMetadata(index),
            createdAt: new Date().toISOString(),
            updatedAt: new Date().toISOString(),
            version: 1,
            deleted: false
        };
    }
    
    generateMetadata(index) {
        const categories = ['technology', 'science', 'art', 'sports', 'music', 'travel', 'food', 'health'];
        const authors = ['Alice', 'Bob', 'Charlie', 'Diana', 'Eve', 'Frank', 'Grace', 'Henry'];
        
        return {
            title: `Document ${index}`,
            category: categories[index % categories.length],
            author: authors[Math.floor(this._random() * authors.length)],
            score: this._random(),
            tags: this.generateTags(),
            length: Math.floor(this._random() * 4900) + 100,
            created: new Date().toISOString()
        };
    }
    
    generateTags() {
        const allTags = [
            'important', 'urgent', 'draft', 'published', 'archived',
            'featured', 'trending', 'popular', 'new', 'updated',
            'experimental', 'stable', 'beta', 'alpha', 'deprecated'
        ];
        
        const numTags = Math.floor(this._random() * 5) + 1;
        const tags = [];
        
        for (let i = 0; i < numTags; i++) {
            const tag = allTags[Math.floor(this._random() * allTags.length)];
            if (!tags.includes(tag)) {
                tags.push(tag);
            }
        }
        
        return tags;
    }
    
    generateClusteredVectors(count, numClusters) {
        const vectors = [];
        const clusterSize = Math.floor(count / numClusters);
        
        for (let clusterId = 0; clusterId < numClusters; clusterId++) {
            // Generate cluster center
            const center = [];
            for (let j = 0; j < this.dimensions; j++) {
                center.push((this._random() - 0.5) * 2);
            }
            
            // Generate vectors around this center
            for (let i = 0; i < clusterSize; i++) {
                const vector = [];
                
                for (let j = 0; j < this.dimensions; j++) {
                    const noise = (this._random() - 0.5) * 0.6; // ±0.3 noise
                    vector.push(center[j] + noise);
                }
                
                // Normalize
                const norm = Math.sqrt(vector.reduce((sum, x) => sum + x * x, 0));
                const normalizedVector = norm > 0 ? vector.map(x => x / norm) : vector;
                
                vectors.push({
                    id: uuidv4(),
                    vector: normalizedVector,
                    metadata: {
                        cluster: clusterId,
                        clusterMember: i,
                        title: `Cluster ${clusterId} Item ${i}`
                    },
                    createdAt: new Date().toISOString(),
                    updatedAt: new Date().toISOString(),
                    version: 1,
                    deleted: false
                });
            }
        }
        
        // Fill remaining vectors
        while (vectors.length < count) {
            vectors.push(this.generateVector(vectors.length));
        }
        
        return vectors;
    }
    
    generateSparseVectors(count, sparsity) {
        const vectors = [];
        
        for (let i = 0; i < count; i++) {
            const vector = new Array(this.dimensions).fill(0);
            const nonZeroCount = Math.floor(this.dimensions * (1.0 - sparsity));
            
            // Randomly select positions for non-zero values
            const positions = Array.from({length: this.dimensions}, (_, i) => i);
            
            // Shuffle positions
            for (let j = positions.length - 1; j > 0; j--) {
                const k = Math.floor(this._random() * (j + 1));
                [positions[j], positions[k]] = [positions[k], positions[j]];
            }
            
            // Set non-zero values
            for (let j = 0; j < nonZeroCount; j++) {
                vector[positions[j]] = (this._random() - 0.5) * 2;
            }
            
            // Normalize
            const norm = Math.sqrt(vector.reduce((sum, x) => sum + x * x, 0));
            const normalizedVector = norm > 0 ? vector.map(x => x / norm) : vector;
            
            vectors.push({
                id: uuidv4(),
                vector: normalizedVector,
                metadata: {
                    type: 'sparse',
                    sparsity: sparsity,
                    index: i
                },
                createdAt: new Date().toISOString(),
                updatedAt: new Date().toISOString(),
                version: 1,
                deleted: false
            });
        }
        
        return vectors;
    }
}

module.exports = TestDataGenerator;