
#[tokio::test]
async fn test_create_and_query_index() -> Result<()> {
    let temp_dir = TempDir::new().unwrap();
    let index_path = temp_dir.path();
    
    // Create index
    let index = LocalIndex::new(index_path, None)?;
    
    let config = CreateIndexConfig {
        version: 1,
        delete_if_exists: true,
        ..Default::default()
    };
    
    index.create_index(Some(config)).await?;
    
    // Verify index was created
    assert!(index.is_index_created().await);
    
    // Insert test item
    let test_item = VectorItem {
        id: Uuid::new_v4(),
        vector: vec![1.0, 0.0, 0.0],
        metadata: serde_json::json!({
            "text": "test item",
            "category": "test"
        }),
        ..Default::default()
    };
    
    let inserted = index.insert_item(test_item.clone()).await?;
    assert_eq!(inserted.vector, test_item.vector);
    
    // Query the item back
    let retrieved = index.get_item(&inserted.id).await?;
    assert!(retrieved.is_some());
    
    let retrieved_item = retrieved.unwrap();
    assert_eq!(retrieved_item.id, inserted.id);
    assert_eq!(retrieved_item.vector, test_item.vector);
    
    // Test vector search
    let query_vector = vec![1.0, 0.0, 0.0];
    let results = index.query_items(query_vector, Some(10), None).await?;
    
    assert!(!results.is_empty());
    assert_eq!(results[0].item.id, inserted.id);
    assert!(results[0].score > 0.9); // Should be very similar
    
    Ok(())
}

#[tokio::test]
async fn test_legacy_compatibility() -> Result<()> {
    let temp_dir = TempDir::new().unwrap();
    let index_path = temp_dir.path();
    
    // Create a legacy-format index file manually
    let legacy_index = r#"{
        "version": 1,
        "metadata_config": {
            "indexed": [],
            "reserved": [],
            "maxSize": 1048576,
            "dynamic": true
        },
        "items": [
            {
                "id": "550e8400-e29b-41d4-a716-446655440000",
                "vector": [0.1, 0.2, 0.3],
                "metadata": {"text": "legacy item"},
                "deleted": false,
                "created_at": "2024-01-01T00:00:00Z",
                "updated_at": "2024-01-01T00:00:00Z",
                "version": 1
            }
        ]
    }"#;
    
    let index_file_path = index_path.join("index.json");
    tokio::fs::write(&index_file_path, legacy_index).await.unwrap();
    
    // Open with Rust implementation - should auto-detect legacy format
    let index = LocalIndex::new(index_path, None)?;
    
    assert!(index.is_index_created().await);
    
    // Should be able to read the legacy item
    let uuid = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
    let item = index.get_item(&uuid).await?;
    
    assert!(item.is_some());
    let item = item.unwrap();
    assert_eq!(item.vector, vec![0.1, 0.2, 0.3]);
    assert_eq!(item.metadata["text"], "legacy item");
    
    Ok(())
}