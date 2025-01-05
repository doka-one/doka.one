use log::*;
use rayon::iter::IntoParallelRefIterator;
use tokio::task;

use commons_error::*;
use dkcrypto::dk_crypto::CypherMode::AES;
use dkcrypto::dk_crypto::DkEncrypt;

pub(crate) struct KvStore {
    pub bucket: String,
    pub secret16: String,
}

impl KvStore {
    pub fn new(bucket: &str, secret16: &str) -> KvStore {
        KvStore {
            bucket: bucket.to_string(),
            secret16: "0123456789ABCDEF".to_string(),
        }
    }

    pub async fn read_from_nats(&self, key: &str) -> anyhow::Result<Option<Vec<u8>>> {
        // Connect to the NATS server
        let client = async_nats::connect("localhost:4222").await?;
        log_info!(
            "Connected to NATS for reading, {} {}",
            &self.bucket,
            &key[0..12]
        );

        // Create a JetStream context
        let jetstream = async_nats::jetstream::new(client);

        // Create or access a Key-Value store TODO with user/password

        let kv = jetstream.get_key_value(&self.bucket).await?;
        log_info!("Key-Value store '{}' ready", &self.bucket);

        let hash_key = DkEncrypt::hash_word(key);
        let mut data = Vec::new();
        let mut i = 0;
        let mut encrypted_chunks: Vec<Vec<u8>> = vec![];

        // // place un départ de timer ici
        // let start_time = std::time::Instant::now();
        loop {
            let key_i = format!("{}-{}", &hash_key, i);

            // Retrieve and print the stored data
            if let Some(entry) = kv.entry(key_i).await? {
                let chunk = entry.value.to_vec();
                encrypted_chunks.push(chunk);
            } else {
                log_info!("Number of parts : '{}'", i);
                break;
            }
            i += 1;
        }

        // TODO Use standards error handling

        let secret = self.secret16.clone();
        let decrypted_chunks: Vec<Vec<u8>> = task::spawn_blocking(move || {
            use rayon::iter::ParallelIterator;
            encrypted_chunks
                .par_iter()
                .map(|chunk| {
                    // TODO we can switch to the self.secret16 as soon as we placed the IV
                    //      at the first 16 bytes of the data
                    log_info!("x - Decrypted");
                    DkEncrypt::new(AES).decrypt_vec(&chunk, &secret).unwrap()
                })
                .collect()
        })
        .await
        .expect("Task panicked");

        // TODO Place some standard logs

        for decrypted_chunk in decrypted_chunks.iter() {
            data.extend_from_slice(decrypted_chunk);
        }

        if i == 0 {
            Ok(None)
        } else {
            Ok(Some(data))
        }
    }

    /// Store the data in the NATS server
    /// # Arguments
    /// * `key` - The key name
    /// * `data` - The data to store
    pub async fn store_to_nats(&self, key: &str, data: Vec<u8>) -> anyhow::Result<()> {
        // Connect to the NATS server
        let client = async_nats::connect("localhost:4222").await?;
        log_info!(
            "Connected to NATS for storing, {} {}, size: {}",
            &self.bucket,
            &key[0..12],
            data.len()
        );

        // Create a JetStream context
        let jetstream = async_nats::jetstream::new(client);

        // Create or access a Key-Value store
        let kv = jetstream.get_key_value(&self.bucket.to_string()).await?;
        log_info!("Key-Value store '{}' ready", &&self.bucket);

        // Define chunk size (1 MB) minus some bytes for the encryption overhead
        const CHUNK_SIZE: usize = 1 * 1024 * 1024 - 40; // 1 MB - 40 bytes

        let hash_key = DkEncrypt::hash_word(key);

        // Process each chunk in parallel using Rayon
        use rayon::iter::ParallelIterator;

        let chunks: Vec<(usize, Vec<u8>)> = data
            .chunks(CHUNK_SIZE)
            .enumerate()
            .map(|(i, chunk)| (i, chunk.to_vec()))
            .collect();

        let secret = self.secret16.clone();
        let encrypted_chunks: Vec<Vec<u8>> = task::spawn_blocking(move || {
            use rayon::iter::ParallelIterator;
            chunks
                .par_iter()
                .map(|(i, chunk)| DkEncrypt::new(AES).encrypt_vec(chunk, &secret).unwrap())
                .collect()
        })
        .await
        .expect("Task panicked");

        for (i, encrypted) in encrypted_chunks.into_iter().enumerate() {
            // Generate a unique key for the chunk
            let key_i = format!("{}-{}", &hash_key, i);
            let count = encrypted.len();
            // Store the encrypted chunk in the KV store
            if let Err(e) = kv.put(&key_i, encrypted.into()).await {
                log_error!(">> Failed to store chunk {}: {:?}", key_i, e);
            } else {
                log_info!(">> Data stored with key '{}', size {}", &key_i, &count);
            }
        }

        Ok(())
    }
}
