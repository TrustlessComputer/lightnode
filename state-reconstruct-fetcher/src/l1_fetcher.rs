use std::{cmp, fs::File, future::Future, ops::Deref, str::FromStr, sync::Arc};

use ethers::{
    abi::{Contract, Function, Token},
    prelude::*,
    types::{transaction::eip2718::TypedTransaction, Address},
};
use eyre::Result;
use rand::random;
use state_reconstruct_storage::reconstruction::ReconstructionDatabase;
use thiserror::Error;
use tokio::{
    sync::{mpsc, Mutex},
    time::{sleep, Duration, Instant},
};
use tokio_util::sync::CancellationToken;

use crate::{
    blob_http_client::BlobHttpClient,
    constants::{
        ethereum::{
            BOOJUM_BLOCK, GENESIS_BLOCK, NUM_CONFIRMATIONS,
            VERIFY_HELPER_ADDR, ZK_SYNC_ADDR, 
        },
        btc::{
            BTC_RPC_ENDPOINT, BTC_SIGNER_ADDR, BTC_RPC_USERNAME, BTC_RPC_PASSWORD, SIGNATURE_LENGTH, CHECKPOINT_BLOCK_NUMBERS,
        },
    },
    metrics::L1Metrics,
    types::{CommitBlock, ParseError, Status},
};
use bitcoincore_rpc::{bitcoin::{self, secp256k1}, Auth, Client, RpcApi};

/// `MAX_RETRIES` is the maximum number of retries on failed L1 call.
const MAX_RETRIES: u8 = 5;
/// The interval in seconds to wait before retrying to fetch a previously failed transaction.
const FAILED_FETCH_RETRY_INTERVAL_S: u64 = 10;
/// The interval in seconds in which to print metrics.
const METRICS_PRINT_INTERVAL_S: u64 = 10;

#[allow(clippy::enum_variant_names)]
#[derive(Error, Debug)]
pub enum L1FetchError {
    #[error("get logs failed")]
    GetLogs,

    #[error("get tx failed")]
    GetTx,

    #[error("get end block number failed")]
    GetEndBlockNumber,
}

pub struct L1FetcherOptions {
    /// The Ethereum JSON-RPC HTTP URL to use.
    pub http_url: String,
    pub da_url: String,
    /// The Ethereum blob storage URL base.
    pub blobs_url: String,
    /// Ethereum block number to start state import from.
    pub start_block: u64,
    /// The number of blocks to process from Ethereum.
    pub block_count: Option<u64>,
    /// The amount of blocks to step over on each log iterration.
    pub block_step: u64,
    /// If present, don't poll for new blocks after reaching the end.
    pub disable_polling: bool,
}

#[derive(Clone)]
struct FetcherCancellationToken(CancellationToken);

impl FetcherCancellationToken {
    const LONG_TIMEOUT_S: u64 = 120;

    pub fn new() -> FetcherCancellationToken {
        FetcherCancellationToken(CancellationToken::new())
    }

    pub async fn cancelled_else_long_timeout(&self) {
        tokio::select! {
            _ = self.cancelled() => {}
            _ = tokio::time::sleep(Duration::from_secs(Self::LONG_TIMEOUT_S)) => {}
        }
    }
}

impl Deref for FetcherCancellationToken {
    type Target = CancellationToken;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Clone)]
struct Contracts {
    v1: Contract,
    v2: Contract,
}

struct FullBlock {
    raw_data: Vec<u8>,
    block_number: u64,
    txid: bitcoin::Txid,
}

pub struct L1Fetcher {
    provider: Provider<Http>,
    daprovider: Provider<Http>,
    contracts: Contracts,
    config: L1FetcherOptions,
    inner_db: Option<Arc<Mutex<ReconstructionDatabase>>>,
    metrics: Arc<Mutex<L1Metrics>>,
}

impl L1Fetcher {
    pub fn new(
        config: L1FetcherOptions,
        inner_db: Option<Arc<Mutex<ReconstructionDatabase>>>,
    ) -> Result<Self> {
        let provider = Provider::<Http>::try_from(&config.http_url)
            .expect("could not instantiate HTTP Provider");
        let daprovider =
            Provider::<Http>::try_from(&config.da_url).expect("could not instantiate DAProvider");
        let v1 = Contract::load(File::open("./abi/IZkSync.json")?)?;
        let v2 = Contract::load(File::open("./abi/IZkSyncV2.json")?)?;
        let contracts = Contracts { v1, v2 };

        let initial_l1_block = if inner_db.is_none() {
            GENESIS_BLOCK
        } else {
            config.start_block
        };
        let metrics = Arc::new(Mutex::new(L1Metrics::new(initial_l1_block)));

        Ok(L1Fetcher {
            provider,
            daprovider,
            contracts,
            config,
            inner_db,
            metrics,
        })
    }

    pub async fn run(&self, sink: mpsc::Sender<CommitBlock>) -> Result<()> {
        // Start fetching from the `GENESIS_BLOCK` unless the `start_block` argument is supplied,
        // in which case, start from that instead. If no argument was supplied and a state snapshot
        // exists, start from the block number specified in that snapshot.
        let mut current_l1_block_number = U64::from(self.config.start_block);
        // User might have supplied their own start block, in that case we shouldn't enforce the
        // use of the snapshot value.
        if current_l1_block_number == GENESIS_BLOCK.into() {
            if let Some(snapshot) = &self.inner_db {
                let snapshot_latest_l1_block_number =
                    snapshot.lock().await.get_latest_l1_batch_number()?;
                if snapshot_latest_l1_block_number > current_l1_block_number {
                    current_l1_block_number = snapshot_latest_l1_block_number;
                    tracing::info!(
                        "Found snapshot, starting from L1 block {current_l1_block_number}"
                    );
                }
            };
        }

        let end_block = self
            .config
            .block_count
            .map(|count| (current_l1_block_number + count));

        // Initialize metrics with last state, if it exists.
        {
            let mut metrics = self.metrics.lock().await;
            metrics.first_l1_block_num = current_l1_block_number.as_u64();
            metrics.latest_l1_block_num = current_l1_block_number.as_u64();
            if let Some(snapshot) = &self.inner_db {
                metrics.latest_l2_block_num = snapshot.lock().await.get_latest_l2_batch_number()?;
                metrics.first_l2_block_num = metrics.latest_l2_block_num;
            }
        }

        tokio::spawn({
            let metrics = self.metrics.clone();
            async move {
                loop {
                    metrics.lock().await.print();
                    tokio::time::sleep(Duration::from_secs(METRICS_PRINT_INTERVAL_S)).await;
                }
            }
        });

        // Wait for shutdown signal in background.
        let token = FetcherCancellationToken::new();
        let cloned_token = token.clone();
        tokio::spawn(async move {
            match tokio::signal::ctrl_c().await {
                Ok(()) => {
                    tracing::info!("Shutdown signal received, finishing up and shutting down...");
                }
                Err(err) => {
                    tracing::error!("Shutdown signal failed: {err}");
                }
            };

            cloned_token.cancel();
        });

        // let (hash_tx, hash_rx) = mpsc::channel(5);
        // let (calldata_tx, calldata_rx) = mpsc::channel(5);
        let (raw_block_tx, raw_block_rx) = mpsc::channel(2);

        // If an `end_block` was supplied we shouldn't poll for newer blocks.
        let mut disable_polling = self.config.disable_polling;
        if end_block.is_some() {
            disable_polling = true;
        }

        // Split L1 block processing into three async steps:
        // - BlockCommit event filter (main).
        // - Referred L1 block fetch (tx).
        // - Calldata parsing (parse).
        // let tx_handle = self.spawn_tx_handler(
        //     hash_rx,
        //     calldata_tx,
        //     token.clone(),
        //     current_l1_block_number.as_u64(),
        // );
        // let parse_handle = self.spawn_parsing_handler(calldata_rx, sink, token.clone())?;
        let parse_handle = self.spawn_parsing_handler_btc(raw_block_rx, sink, token.clone())?;
        let main_handle = self.spawn_main_handler_btc(
            raw_block_tx,
            token,
            current_l1_block_number,
            end_block,
            disable_polling,
        )?;

        // tx_handle.await?;
        let last_processed_l1_block_num = parse_handle.await?;
        let mut last_fetched_l1_block_num = main_handle.await?;

        // Store our current L1 block number so we can resume from where we left
        // off, we also make sure to update the metrics before printing them.
        if let Some(block_num) = last_processed_l1_block_num {
            if let Some(snapshot) = &self.inner_db {
                snapshot
                    .lock()
                    .await
                    .set_latest_l1_batch_number(block_num)?;
            }

            // Fetching is naturally ahead of parsing, but the data
            // that wasn't parsed on program interruption/error is
            // now lost and will have to be fetched again...
            if last_fetched_l1_block_num > block_num {
                tracing::debug!(
                    "L1 Blocks fetched but not parsed: {}",
                    last_fetched_l1_block_num - block_num
                );
                last_fetched_l1_block_num = block_num;
            }

            let mut metrics = self.metrics.lock().await;
            metrics.latest_l1_block_num = last_fetched_l1_block_num;
            metrics.print();
        } else {
            tracing::warn!("No new blocks were processed");
        }

        Ok(())
    }

    fn spawn_main_handler_btc(
        &self,
        raw_block_tx: mpsc::Sender<FullBlock>,
        cancellation_token: FetcherCancellationToken,
        mut current_l1_block_number: U64,
        max_end_block: Option<U64>,
        disable_polling: bool,
    ) -> Result<tokio::task::JoinHandle<u64>> {
        if let Some(end_block_limit) = max_end_block {
            assert!(current_l1_block_number <= end_block_limit);
        }

        let metrics = self.metrics.clone();
        let event = self.contracts.v1.events_by_name("BlockCommit")?[0].clone();
        let provider_clone = self.provider.clone();
        let block_step = self.config.block_step;

        let url = BTC_RPC_ENDPOINT;

        let rpc = Client::new(&url, Auth::UserPass(BTC_RPC_USERNAME.to_owned(), BTC_RPC_PASSWORD.to_owned())).unwrap();
        let mut current_block_height = current_l1_block_number.as_u64();

        Ok(tokio::spawn({
            async move {
                let mut target_end_block = rpc.get_block_count().unwrap() - 1 - NUM_CONFIRMATIONS;
                if let Some(end_block_limit) = max_end_block {
                    if target_end_block > end_block_limit.as_u64() {
                        target_end_block = end_block_limit.as_u64();
                    }
                }
                let checkpoint_block_numbers = CHECKPOINT_BLOCK_NUMBERS;
                // [];
                let mut current_checkpoint_index = 0;

                let mut batch_count = 0;
                loop {
                    if current_checkpoint_index < checkpoint_block_numbers.len() {
                        if current_block_height < checkpoint_block_numbers[current_checkpoint_index] {
                            current_block_height = checkpoint_block_numbers[current_checkpoint_index];
                        } else {
                            current_checkpoint_index += 1;
                            continue;
                        }
                    } else {
                        if target_end_block > current_block_height {
                            current_block_height += 1;
                        } else {
                            break current_block_height;
                        }
                    }

                    let best_block_hash_by_height =
                        rpc.get_block_hash(current_block_height).unwrap();

                    let blk;
                    match rpc.get_block(&best_block_hash_by_height) {
                        Ok(b) => blk = b,
                        Err(e) => {
                            tracing::warn!("Cannot get block: {}", current_block_height);
                            continue;
                        }
                    }

                    for tx in blk.txdata {
                        // get the raw witness of each input
                        for input in &tx.input {
                            let witness = &input.witness;
                            let wv = &witness.to_vec();
                            if wv.len() < 2 {
                                // tracing::debug!("block height {} witness length not matched, skipping", current_block_height);
                                continue;
                            }

                            let content = &witness.to_vec()[1];
                            if content.len() < 71 {
                                tracing::debug!("block height {} witness content length not matched, skipping", current_block_height);
                                continue;
                            }

                            // remove the first 70 bytes & last 1 byte
                            let content = &content[70..content.len() - 1];
                            // filter by id
                            let first_byte = content[0];
                            if first_byte != 8 {
                                tracing::debug!("witness type does not match, skipping");
                                continue;
                            }
                            let chunks: Vec<Vec<u8>> =
                                content.chunks(523).map(|c| c.to_vec()).collect();
                            // manually collect the chunks into a Vec<u8>
                        
                            let mut result = Vec::new();
                            for i in 0..chunks.len() {
                                if i == chunks.len() - 1 {
                                    // pad 3 bytes at the end
                                    let c = chunks[i].clone();
                                    result.extend(c.clone());
                                } else if i == chunks.len() - 2 {
                                    // pad 2 bytes at the end
                                    let c = chunks[i].clone();
                                    result.extend(&c[..c.len() - 3].to_vec());
                                    let d = chunks[i].clone();
                                    let last_two_bytes = &d[d.len() - 2..];
                                    if last_two_bytes != [0x0E, 0x01] {
                                        // Add the last 2 bytes if they are not equal to 0x0E01
                                        result.extend(&d[d.len() - 2..].to_vec());
                                    }
                                    
                                } else {
                                    let c = chunks[i].clone();
                                    result.extend(&c[..c.len() - 3].to_vec());
                                }
                            }

                            let content = result;

                            // check signature
                            let rawsig = &content[1..1 + SIGNATURE_LENGTH];
                            let signed_msg = hex::encode(&content[1 + SIGNATURE_LENGTH..]);
                            let msg_hash = bitcoin::sign_message::signed_msg_hash(&signed_msg);
                            let sig = bitcoin::sign_message::MessageSignature::from_slice(&rawsig).unwrap();
                            let secp = secp256k1::Secp256k1::new();
                            tracing::info!("DEBUG Message hash {:?} btcAddr {:?} sig {:?}", hex::encode(msg_hash), BTC_SIGNER_ADDR, hex::encode(rawsig));
                            let signature_valid = match sig.is_signed_by_address(&secp, &bitcoin::Address::from_str(BTC_SIGNER_ADDR).unwrap().assume_checked(), msg_hash) {
                                Ok(res) => res,
                                Err(e) => {
                                    tracing::error!("signature error: {e}");
                                    false
                                },
                            };

                            
                            
                            if !signature_valid {
                                let content = &content[1 + SIGNATURE_LENGTH..];
                                let commit_data = &content[..1636];
                                let prove_data = &content[1636..3784];
                                let execute_data = &content[3784..content.len()];
                                tracing::debug!("commit: {:?}", hex::encode(commit_data));
                                tracing::debug!("prove data: {:?}", hex::encode(prove_data));
                                tracing::debug!("execute: {:?}", hex::encode(execute_data));
                                tracing::warn!("invalid signature, skipping");
                                continue;
                            } else {
                                tracing::info!("signature valid");
                            }
                            let content = &content[1 + SIGNATURE_LENGTH..];

                            if let Err(e) = raw_block_tx.send(FullBlock {
                                raw_data: content.to_vec(),
                                block_number: current_block_height,
                                txid: tx.txid(),
                            }).await {
                                if cancellation_token.is_cancelled() {
                                    tracing::debug!("Shutting down tx sender...");
                                } else {
                                    tracing::error!("Cannot send tx hash: {e}");
                                    cancellation_token.cancel();
                                }

                                return current_block_height;
                            } else {
                                batch_count += 1;
                                tracing::info!("block #{}, batch #{} tx rawdata sent", current_block_height, batch_count);
                            }
                        }
                    }
                }
            }
        }))
    }

    fn spawn_main_handler(
        &self,
        hash_tx: mpsc::Sender<H256>,
        cancellation_token: FetcherCancellationToken,
        mut current_l1_block_number: U64,
        max_end_block: Option<U64>,
        disable_polling: bool,
    ) -> Result<tokio::task::JoinHandle<u64>> {
        if let Some(end_block_limit) = max_end_block {
            assert!(current_l1_block_number <= end_block_limit);
        }

        let metrics = self.metrics.clone();
        let event = self.contracts.v1.events_by_name("BlockCommit")?[0].clone();
        let provider_clone = self.provider.clone();
        let block_step = self.config.block_step;

        Ok(tokio::spawn({
            async move {
                let mut latest_l2_block_number = U256::zero();
                let mut previous_hash = None;
                let mut end_block = None;
                loop {
                    // Break on the receivement of a `ctrl_c` signal.
                    if cancellation_token.is_cancelled() {
                        tracing::debug!("Shutting down main handler...");
                        break;
                    }

                    let Some(end_block_number) = end_block else {
                        if let Ok(new_end) = L1Fetcher::retry_call(
                            || provider_clone.get_block(BlockNumber::Finalized),
                            L1FetchError::GetEndBlockNumber,
                        )
                        .await
                        {
                            if let Some(found_block) = new_end {
                                if let Some(ebn) = found_block.number {
                                    let end_block_number =
                                        if let Some(end_block_limit) = max_end_block {
                                            if end_block_limit < ebn {
                                                end_block_limit
                                            } else {
                                                ebn
                                            }
                                        } else {
                                            ebn
                                        };
                                    end_block = Some(end_block_number);
                                    metrics.lock().await.last_l1_block = end_block_number.as_u64();
                                }
                            }
                        } else {
                            tracing::debug!("Cannot get latest block number...");
                            cancellation_token.cancelled_else_long_timeout().await;
                        }

                        continue;
                    };

                    if current_l1_block_number > end_block_number {
                        // Any place in this function that increases `current_l1_block_number`
                        // beyond end_block_number must check the `disabled_polling`
                        // case first.
                        // For external callers, this function must not be called w/
                        // `current_l1_block_number > end_block_number`.
                        assert!(!disable_polling);
                        tracing::debug!("Waiting for upstream to move on...");
                        cancellation_token.cancelled_else_long_timeout().await;
                        end_block = None;
                        continue;
                    }

                    // Create a filter showing only `BlockCommit`s from the [`ZK_SYNC_ADDR`].
                    // TODO: Filter by executed blocks too.
                    // Don't go beyond `end_block_number` - tip of the chain might still change.
                    let filter_end_block_number =
                        cmp::min(current_l1_block_number + block_step - 1, end_block_number);
                    let filter = Filter::new()
                        .address(ZK_SYNC_ADDR.parse::<Address>().unwrap())
                        .topic0(event.signature())
                        .from_block(current_l1_block_number)
                        .to_block(filter_end_block_number);

                    // Grab all relevant logs.
                    let before = Instant::now();
                    if let Ok(logs) = L1Fetcher::retry_call(
                        || provider_clone.get_logs(&filter),
                        L1FetchError::GetLogs,
                    )
                    .await
                    {
                        let duration = before.elapsed();
                        metrics.lock().await.log_acquisition.add(duration);

                        for log in logs {
                            // log.topics:
                            // topics[1]: L2 block number.
                            // topics[2]: L2 block hash.
                            // topics[3]: L2 commitment.

                            let new_l2_block_number =
                                U256::from_big_endian(log.topics[1].as_fixed_bytes());
                            if new_l2_block_number <= latest_l2_block_number {
                                continue;
                            }
                            tracing::info!("log.transaction_hash {:?}", log.transaction_hash);

                            if let Some(tx_hash) = log.transaction_hash {
                                if let Some(prev_hash) = previous_hash {
                                    if prev_hash == tx_hash {
                                        tracing::debug!(
                                            "Transaction hash {:?} already known - not sending.",
                                            tx_hash
                                        );
                                        continue;
                                    }
                                }

                                if let Err(e) = hash_tx.send(tx_hash).await {
                                    if cancellation_token.is_cancelled() {
                                        tracing::debug!("Shutting down tx sender...");
                                    } else {
                                        tracing::error!("Cannot send tx hash: {e}");
                                        cancellation_token.cancel();
                                    }

                                    return current_l1_block_number.as_u64();
                                }

                                previous_hash = Some(tx_hash);
                            }

                            latest_l2_block_number = new_l2_block_number;
                        }
                    } else {
                        cancellation_token.cancelled_else_long_timeout().await;
                        continue;
                    };

                    metrics.lock().await.latest_l1_block_num = current_l1_block_number.as_u64();

                    let next_l1_block_number = current_l1_block_number + U64::from(block_step);
                    if next_l1_block_number > end_block_number {
                        // Some of the `block_step` blocks asked for in this iteration
                        // probably didn't exist yet, so we set `current_l1_block_number`
                        // appropriately as to not skip them.
                        if current_l1_block_number < end_block_number {
                            current_l1_block_number = end_block_number;
                        } else {
                            // `current_l1_block_number == end_block_number`,
                            // IOW, no more blocks can be retrieved until new ones
                            // have been published on L1.
                            if disable_polling {
                                tracing::debug!("Fetching finished...");
                                return current_l1_block_number.as_u64();
                            }

                            current_l1_block_number = end_block_number + U64::one();
                            // `current_l1_block_number > end_block_number`,
                            // IOW, end block will be reset in the next
                            // iteration & updated afterwards.
                        }
                    } else {
                        // We haven't reached past `end_block` yet, stepping by `block_step`.
                        current_l1_block_number = next_l1_block_number;
                    }
                }

                current_l1_block_number.as_u64()
            }
        }))
    }

    fn spawn_tx_handler(
        &self,
        mut hash_rx: mpsc::Receiver<H256>,
        l1_tx_tx: mpsc::Sender<Transaction>,
        cancellation_token: FetcherCancellationToken,
        mut last_block: u64,
    ) -> tokio::task::JoinHandle<()> {
        let metrics = self.metrics.clone();
        let provider = self.provider.clone();

        tokio::spawn({
            async move {
                while let Some(hash) = hash_rx.recv().await {
                    let tx = loop {
                        let before = Instant::now();
                        match L1Fetcher::retry_call(
                            || provider.get_transaction(hash),
                            L1FetchError::GetTx,
                        )
                        .await
                        {
                            Ok(Some(tx)) => {
                                let duration = before.elapsed();
                                metrics.lock().await.tx_acquisition.add(duration);
                                break tx;
                            }
                            _ => {
                                // Task has been cancelled by user, abort loop.
                                if cancellation_token.is_cancelled() {
                                    tracing::debug!("Shutting down tx handler...");
                                    return;
                                }

                                tracing::error!(
                                    "Failed to get transaction for hash: {}, retrying in a bit...",
                                    hash
                                );
                                tokio::time::sleep(Duration::from_secs(
                                    FAILED_FETCH_RETRY_INTERVAL_S,
                                ))
                                .await;
                            }
                        };
                    };

                    if let Some(current_block) = tx.block_number {
                        let current_block = current_block.as_u64();
                        if last_block < current_block {
                            metrics.lock().await.latest_l1_block_num = current_block;
                            last_block = current_block;
                        }
                    }

                    if let Err(e) = l1_tx_tx.send(tx).await {
                        if cancellation_token.is_cancelled() {
                            tracing::debug!("Shutting down tx task...");
                        } else {
                            tracing::error!("Cannot send tx: {e}");
                        }

                        return;
                    }
                }
            }
        })
    }

    fn spawn_parsing_handler_btc(
        &self,
        mut raw_block_rx: mpsc::Receiver<FullBlock>,
        sink: mpsc::Sender<CommitBlock>,
        cancellation_token: FetcherCancellationToken,
    ) -> Result<tokio::task::JoinHandle<Option<u64>>> {
        let metrics = self.metrics.clone();
        let contracts = self.contracts.clone();
        let provider = self.provider.clone();
        let dap = self.daprovider.clone();
        let client = BlobHttpClient::new(self.config.blobs_url.clone())?;
        Ok(tokio::spawn({
            async move {
                let commit_fn = contracts.v2.functions_by_name("commitBatches").unwrap()[0].clone();
                let mut last_block_number_processed = None;
                tracing::info!("waiting for raw tx data");

                while let Some(FullBlock { raw_data: content, block_number , txid}) = raw_block_rx.recv().await {
                    if cancellation_token.is_cancelled() {
                        tracing::debug!("Shutting down parsing handler...");
                        return last_block_number_processed;
                    }

                    let commit_data = &content[..1636];
                    let prove_data = &content[1636..3784];
                    let execute_data = &content[3784..content.len()];
                    tracing::debug!("commit: {:?}", hex::encode(commit_data));
                    tracing::debug!("prove data: {:?}", hex::encode(prove_data));
                    tracing::debug!("execute: {:?}", hex::encode(execute_data));

                    // verify proof by calling contract
                    let contract_address = VERIFY_HELPER_ADDR;
                    let contract_call_tx = TransactionRequest::new()
                        .to(contract_address.parse::<Address>().unwrap())
                        .data(prove_data.to_vec());

                    let proof_valid = match provider
                        .call(&TypedTransaction::Legacy(contract_call_tx), None)
                        .await {
                        Ok(result) => {
                            let verify_result_str = hex::encode(result);
                            verify_result_str == "0000000000000000000000000000000000000000000000000000000000000001"
                        },
                        Err(e) => {
                            tracing::error!("contract call error: {e}");
                            false
                        },
                    };

                    if !proof_valid {
                        tracing::error!("verify proof failed");
                        cancellation_token.cancel();
                        return last_block_number_processed;
                    }
                    tracing::info!("verify proof succeeded");

                    let blocks = loop {
                        match parse_calldata(block_number, &commit_fn, commit_data, &client, &dap).await {
                            Ok(blks) => break blks,
                            Err(e) => match e {
                                ParseError::BlobStorageError(_) => {
                                    if cancellation_token.is_cancelled() {
                                        tracing::debug!("Shutting down parsing...");
                                        return last_block_number_processed;
                                    }
                                    cancellation_token.cancelled_else_long_timeout().await;
                                }
                                ParseError::BlobFormatError(data, inner) => {
                                    tracing::error!("Cannot parse {}: {}", data, inner);
                                    cancellation_token.cancel();
                                    return last_block_number_processed;
                                }
                                _ => {
                                    tracing::error!("Failed to parse calldata: {e}");
                                    cancellation_token.cancel();
                                    return last_block_number_processed;
                                }
                            },
                        }
                    };

                    let mut metrics = metrics.lock().await;
                    for blk in blocks {
                        metrics.latest_l2_block_num = blk.l2_block_number;
                        if let Err(e) = sink.send(blk).await {
                            if cancellation_token.is_cancelled() {
                                tracing::debug!("Shutting down parsing task...");
                            } else {
                                tracing::error!("Cannot send block: {e}");
                                cancellation_token.cancel();
                            }

                            return last_block_number_processed;
                        } else {
                            tracing::info!("commit block sent");
                            tracing::info!("DEBUG Base Block number {:?}", metrics.latest_l2_block_num);
                            tracing::info!("DEBUG Bitcoin txid {:?}", txid);
                            
                            let da_txhash = match get_da_txhash(&commit_fn, commit_data).await {
                                Ok(da_txhash) => {
                                    tracing::info!("DEBUG DA txid {:?}", da_txhash);
                                    da_txhash
                                },
                                Err(e) => {
                                    tracing::error!("Cannot get DA txhash: {e}");
                                    cancellation_token.cancel();
                                    return last_block_number_processed;
                                }
                            };
                            
                            //  write info of block to file with json format
                            //  new object Status with batch_data is empty
                            let status = Status {
                                base_batch_number: metrics.latest_l2_block_num.to_string(),
                                bitcoin_tx_hash: txid.to_string(),
                                da_tx_hash: format!("{:?}", da_txhash),
                                batch_data: String::new(),
                            };
                            
                            {
                                let file_path = format!("./db-status/{}.json", metrics.latest_l2_block_num);
                                let file_path_str = &file_path;
                                tracing::info!("DEBUG File path {:?}", file_path_str);
                                let write_status = status.write_to_file(file_path_str).unwrap();
                            
                                if write_status {
                                    tracing::info!("DEBUG Status file written");
                                } else {
                                    tracing::error!("Cannot write status file");
                                    cancellation_token.cancel();
                                    return last_block_number_processed;
                                }
                            }
                            last_block_number_processed = Some(block_number);
                        }
                    }
                }

                // Return the last processed l1 block number, so we can resume from the same point later on.
                last_block_number_processed
            }
        }))
    }

    fn spawn_parsing_handler(
        &self,
        mut l1_tx_rx: mpsc::Receiver<Transaction>,
        sink: mpsc::Sender<CommitBlock>,
        cancellation_token: FetcherCancellationToken,
    ) -> Result<tokio::task::JoinHandle<Option<u64>>> {
        let metrics = self.metrics.clone();
        let contracts = self.contracts.clone();
        let dap = self.daprovider.clone();
        let client = BlobHttpClient::new(self.config.blobs_url.clone())?;
        Ok(tokio::spawn({
            async move {
                let mut boojum_mode = false;
                let mut function =
                    contracts.v1.functions_by_name("commitBlocks").unwrap()[0].clone();
                let mut last_block_number_processed = None;

                while let Some(tx) = l1_tx_rx.recv().await {
                    if cancellation_token.is_cancelled() {
                        tracing::debug!("Shutting down parsing handler...");
                        return last_block_number_processed;
                    }

                    let before = Instant::now();
                    let Some(block_number) = tx.block_number else {
                        tracing::error!("transaction has no block number");
                        break;
                    };
                    let block_number = block_number.as_u64();

                    if !boojum_mode && block_number >= BOOJUM_BLOCK {
                        tracing::debug!("Reached `BOOJUM_BLOCK`, changing commit block format");
                        boojum_mode = true;
                        function =
                            contracts.v2.functions_by_name("commitBatches").unwrap()[0].clone();
                    }

                    let blocks = loop {
                        match parse_calldata(block_number, &function, &tx.input, &client, &dap)
                            .await
                        {
                            Ok(blks) => break blks,
                            Err(e) => match e {
                                ParseError::BlobStorageError(_) => {
                                    if cancellation_token.is_cancelled() {
                                        tracing::debug!("Shutting down parsing...");
                                        return last_block_number_processed;
                                    }
                                    cancellation_token.cancelled_else_long_timeout().await;
                                }
                                ParseError::BlobFormatError(data, inner) => {
                                    tracing::error!("Cannot parse {}: {}", data, inner);
                                    cancellation_token.cancel();
                                    return last_block_number_processed;
                                }
                                _ => {
                                    tracing::error!("Failed to parse calldata: {e}");
                                    cancellation_token.cancel();
                                    return last_block_number_processed;
                                }
                            },
                        }
                    };

                    tracing::info!("DEBUG DEBUG: {:?}", tx.hash);

                    let mut metrics = metrics.lock().await;
                    for blk in blocks {
                        metrics.latest_l2_block_num = blk.l2_block_number;
                        if let Err(e) = sink.send(blk).await {
                            if cancellation_token.is_cancelled() {
                                tracing::debug!("Shutting down parsing task...");
                            } else {
                                tracing::error!("Cannot send block: {e}");
                                cancellation_token.cancel();
                            }

                            return last_block_number_processed;
                        }
                    }

                    last_block_number_processed = Some(block_number);
                    let duration = before.elapsed();
                    metrics.parsing.add(duration);
                }

                // Return the last processed l1 block number, so we can resume from the same point later on.
                last_block_number_processed
            }
        }))
    }

    async fn retry_call<T, Fut>(callback: impl Fn() -> Fut, err: L1FetchError) -> Result<T>
    where
        Fut: Future<Output = Result<T, ProviderError>>,
    {
        for attempt in 1..=MAX_RETRIES {
            match callback().await {
                Ok(x) => return Ok(x),
                Err(e) => {
                    tracing::error!("attempt {attempt}: failed to fetch from L1: {e}");
                    sleep(Duration::from_millis(50 + random::<u64>() % 500)).await;
                }
            }
        }
        Err(err.into())
    }
}

pub async fn parse_proof_calldata(
    prove_blocks_fn: &Function,
    calldata: &[u8],
    client: &BlobHttpClient,
    dap: &Provider<Http>,
) -> Result<Token, ParseError> {
    let parsed_input = prove_blocks_fn
        .decode_input(&calldata[4..])
        .map_err(|e| ParseError::InvalidCalldata(e.to_string()))?;

    if parsed_input.len() != 3 {
        return Err(ParseError::InvalidCalldata(format!(
            "invalid number of parameters (got {}, expected 2) for commitBlocks function",
            parsed_input.len()
        )));
    }

    let raw_proof = parsed_input[2].clone();
    Ok(raw_proof)
}

pub async fn get_da_txhash(commit_blocks_fn: &Function, calldata: &[u8]) -> Result<H256, ParseError> {
    let mut parsed_input = commit_blocks_fn
        .decode_input(&calldata[4..])
        .map_err(|e| ParseError::InvalidCalldata(e.to_string()))?;

    if parsed_input.len() != 2 {
        return Err(ParseError::InvalidCalldata(format!(
            "invalid number of parameters (got {}, expected 2) for commitBlocks function",
            parsed_input.len()
        )));
    }

    let new_blocks_data = parsed_input
        .pop()
        .ok_or_else(|| ParseError::InvalidCalldata("new blocks data".to_string()))?;
    let stored_block_info = parsed_input
        .pop()
        .ok_or_else(|| ParseError::InvalidCalldata("stored block info".to_string()))?;

    let abi::Token::Tuple(stored_block_info) = stored_block_info else {
        return Err(ParseError::InvalidCalldata(
            "invalid StoredBlockInfo".to_string(),
        ));
    };

    let abi::Token::Uint(_previous_l2_block_number) = stored_block_info[0].clone() else {
        return Err(ParseError::InvalidStoredBlockInfo(
            "cannot parse previous L2 block number".to_string(),
        ));
    };

    let abi::Token::Uint(_previous_enumeration_index) = stored_block_info[2].clone() else {
        return Err(ParseError::InvalidStoredBlockInfo(
            "cannot parse previous enumeration index".to_string(),
        ));
    };
    let abi::Token::Array(data) = new_blocks_data else {
        return Err(ParseError::InvalidCommitBlockInfo(
            "cannot convert newBlocksData to array".to_string(),
        ));
    };

    let raw = CommitBlock::get_pubdata_from_token_resolve(&data[0]).await?;
    let hash = H256::from_slice(&raw);
    Ok(hash)
}

pub async fn parse_calldata(
    l1_block_number: u64,
    commit_blocks_fn: &Function,
    calldata: &[u8],
    client: &BlobHttpClient,
    dap: &Provider<Http>,
) -> Result<Vec<CommitBlock>, ParseError> {
    let mut parsed_input = commit_blocks_fn
        .decode_input(&calldata[4..])
        .map_err(|e| ParseError::InvalidCalldata(e.to_string()))?;

    if parsed_input.len() != 2 {
        return Err(ParseError::InvalidCalldata(format!(
            "invalid number of parameters (got {}, expected 2) for commitBlocks function",
            parsed_input.len()
        )));
    }

    let new_blocks_data = parsed_input
        .pop()
        .ok_or_else(|| ParseError::InvalidCalldata("new blocks data".to_string()))?;
    let stored_block_info = parsed_input
        .pop()
        .ok_or_else(|| ParseError::InvalidCalldata("stored block info".to_string()))?;

    let abi::Token::Tuple(stored_block_info) = stored_block_info else {
        return Err(ParseError::InvalidCalldata(
            "invalid StoredBlockInfo".to_string(),
        ));
    };

    let abi::Token::Uint(_previous_l2_block_number) = stored_block_info[0].clone() else {
        return Err(ParseError::InvalidStoredBlockInfo(
            "cannot parse previous L2 block number".to_string(),
        ));
    };

    let abi::Token::Uint(_previous_enumeration_index) = stored_block_info[2].clone() else {
        return Err(ParseError::InvalidStoredBlockInfo(
            "cannot parse previous enumeration index".to_string(),
        ));
    };

    // Parse blocks using [`CommitBlockInfoV1`] or [`CommitBlockInfoV2`]
    let mut block_infos = parse_commit_block_info(
        &new_blocks_data,
        l1_block_number,
        client,
        dap,
        commit_blocks_fn,
    )
    .await?;
    // Supplement every `CommitBlock` element with L1 block number information.
    block_infos
        .iter_mut()
        .for_each(|e| e.l1_block_number = Some(l1_block_number));
    Ok(block_infos)
}

async fn parse_commit_block_info(
    data: &abi::Token,
    l1_block_number: u64,
    client: &BlobHttpClient,
    dap: &Provider<Http>,
    commit_blocks_fn: &Function,
) -> Result<Vec<CommitBlock>, ParseError> {
    let abi::Token::Array(data) = data else {
        return Err(ParseError::InvalidCommitBlockInfo(
            "cannot convert newBlocksData to array".to_string(),
        ));
    };

    let mut extra_data: Bytes = Bytes::from(Vec::new());

    if data.len() > 0 {
        let raw = CommitBlock::get_pubdata_from_token_resolve(&data[0]).await?;
        // turn into H256
        let hash = H256::from_slice(&raw);
        let tx = match dap.get_transaction(hash).await {
            Ok(res) => match res {
                Some(tx) => tx,
                None => {
                    return Err(ParseError::InvalidCommitBlockInfo(
                        "cannot get transaction".to_string(),
                    ))
                }
            },
            _ => {
                return Err(ParseError::InvalidCommitBlockInfo(
                    "cannot get transaction".to_string(),
                ))
            }
        };

        extra_data = tx.input;
    }

    if extra_data.len() < 4 {
        return Err(ParseError::InvalidCommitBlockInfo(
            "invalid extra data length".to_string(),
        ));
    }

    let mut parsed_input = commit_blocks_fn
        .decode_input(&extra_data[4..])
        .map_err(|e| ParseError::InvalidCalldata(e.to_string()))?;

    if parsed_input.len() != 2 {
        return Err(ParseError::InvalidCalldata(format!(
            "invalid number of parameters (got {}, expected 2) for commitBlocks function",
            parsed_input.len()
        )));
    }

    let extra_pubdata = parsed_input
        .pop()
        .ok_or_else(|| ParseError::InvalidCalldata("new blocks data".to_string()))?;
    let abi::Token::Array(extra_pubdata) = extra_pubdata else {
        return Err(ParseError::InvalidCommitBlockInfo(
            "cannot convert newBlocksData to array".to_string(),
        ));
    };

    let mut result = vec![];

    for i in 0..data.len() {
        let d1 = &data[i];
        let d2 = &extra_pubdata[i];

        // let commit_block = {
        //     if l1_block_number >= BLOB_BLOCK {
        //         CommitBlock::try_from_token_resolve(d2, client).await?
        //     } else if l1_block_number >= BOOJUM_BLOCK {
        //         CommitBlock::try_from_token::<V2>(d1)?
        //     } else {
        //         CommitBlock::try_from_token::<V1>(d1)?
        //     }
        // };
        let commit_block = CommitBlock::try_from_token_resolve(d2, client).await?;
        result.push(commit_block);
    }

    Ok(result)
}
