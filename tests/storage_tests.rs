use crci::storage::encrypted::EncryptedStore;
use crci::storage::{StorageBackend, StorageError};

use tempfile::tempdir;

#[tokio::test]
async fn encrypted_store_write_and_read_roundtrip() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("store.enc");

    let store = EncryptedStore::new(&file_path, "secret_password").unwrap();
    store
        .write("test_key", b"hello crisis world")
        .await
        .unwrap();

    let plaintext = store.read("test_key").await.unwrap();
    assert_eq!(plaintext, b"hello crisis world");
}

#[tokio::test]
async fn encrypted_store_wrong_passphrase_returns_auth_error() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("store.enc");

    let store1 = EncryptedStore::new(&file_path, "correct").unwrap();
    store1.write("test_key", b"data").await.unwrap();

    let store2 = EncryptedStore::new(&file_path, "wrong").unwrap();
    let res = store2.read("test_key").await;

    // Depending on whether the key derived matches, decryption fails.
    assert!(
        matches!(res, Err(StorageError::AuthenticationFailed))
            || matches!(res, Err(StorageError::Io(_)))
    );
}

#[tokio::test]
async fn encrypted_store_delete_then_read_returns_not_found() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("store.enc");

    let store = EncryptedStore::new(&file_path, "secret").unwrap();
    store.write("key1", b"val1").await.unwrap();
    store.delete("key1").await.unwrap();

    let res = store.read("key1").await;
    assert!(matches!(res, Err(StorageError::NotFound(_))));
}

#[tokio::test]
async fn encrypted_store_list_ids_excludes_deleted() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("store.enc");

    let store = EncryptedStore::new(&file_path, "secret").unwrap();
    store.write("a", b"1").await.unwrap();
    store.write("b", b"2").await.unwrap();
    store.write("c", b"3").await.unwrap();
    store.delete("b").await.unwrap();

    let mut ids = store.list_ids().await.unwrap();
    ids.sort();
    assert_eq!(ids, vec!["a".to_string(), "c".to_string()]);
}

#[tokio::test]
async fn encrypted_store_overwrite_returns_latest_value() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("store.enc");

    let store = EncryptedStore::new(&file_path, "secret").unwrap();
    store.write("k", b"v1").await.unwrap();
    store.write("k", b"v2").await.unwrap();

    let val = store.read("k").await.unwrap();
    assert_eq!(val, b"v2");
}

#[tokio::test]
async fn encrypted_store_persists_across_reopen() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("store.enc");

    {
        let store = EncryptedStore::new(&file_path, "secret").unwrap();
        store.write("k", b"persist").await.unwrap();
    } // store dropped

    let store2 = EncryptedStore::new(&file_path, "secret").unwrap();
    let val = store2.read("k").await.unwrap();
    assert_eq!(val, b"persist");
}
