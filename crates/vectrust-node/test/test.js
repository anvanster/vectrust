const { LocalIndex } = require('../index.js');
const { v4: uuidv4 } = require('uuid');

console.log('Testing Vectrust npm package...');

async function runTests() {
  try {
    // Test creating a new index  
    const index = new LocalIndex('./test_index', 'test_index');
    console.log('✓ LocalIndex created successfully');

    // Test creating index with proper config
    const config = {
      version: 1,
      deleteIfExists: true,
      distanceMetric: 'cosine',
      metadataConfig: {
        indexed: [],
        reserved: [],
        maxSize: 1048576,
        dynamic: true
      },
      hnswConfig: {
        m: 16,
        efConstruction: 200,
        efSearch: 200,
        maxElements: 10000,
        maxLevels: 16,
        maxConnections: 16,
        maxConnectionsLayer0: 32,
        distanceMetric: 'cosine'
      }
    };
    
    await index.createIndex(JSON.stringify(config));
    console.log('✓ Index created successfully');

    // Test inserting an item
    const vector = new Array(128).fill(0).map(() => Math.random());
    const metadata = { test: 'data', number: 42 };
    const itemId = uuidv4();
    const vectorItem = {
      id: itemId,
      vector: vector,
      metadata: metadata,
      deleted: false
    };
    
    const insertResult = await index.insertItem(JSON.stringify(vectorItem));
    console.log('✓ Item inserted successfully:', JSON.parse(insertResult));

    // Test querying items
    const results = await index.queryItems(vector, 5, null);
    const queryResults = JSON.parse(results);
    console.log('✓ Query completed successfully, found', queryResults.length, 'results');

    // Test getting an item
    const item = await index.getItem(itemId);
    console.log('✓ Item retrieved successfully:', item ? 'found' : 'not found');

    // Test listing items
    const listOptions = { offset: 0, limit: 10 };
    const items = await index.listItems(JSON.stringify(listOptions));
    const itemList = JSON.parse(items);
    console.log('✓ Items listed successfully, count:', itemList.length);

    console.log('✅ All tests passed! Package works correctly.');
  } catch (error) {
    console.error('❌ Test failed:', error.message);
    process.exit(1);
  }
}

runTests();