pub mod ethereum {
    /// Number of Ethereum blocks to advance in one filter step.
    pub const BLOCK_STEP: u64 = 50;

    /// Block number in Ethereum for zkSync genesis block.
    pub const GENESIS_BLOCK: u64 = 1346490;

    /// Block number in Ethereum of the first Boojum-formatted block.
    pub const BOOJUM_BLOCK: u64 = 1346491;

    /// Block number in Ethereum of the first block storing pubdata within blobs.
    pub const BLOB_BLOCK: u64 = 1346491;

    /// zkSync smart contract address.
    pub const ZK_SYNC_ADDR: &str = "0x055F3a81CAc3a7BB14569913d383AFee8cc4C299";

    /// Default Ethereum blob storage URL base.
    pub const BLOBS_URL: &str = "http://localhost:8555";

    pub const DA_URL: &str = "...";

    pub const HTTP_URL: &str = "...";

    pub const NUM_CONFIRMATIONS: u64 = 1;

    pub const VERIFY_HELPER_ADDR: &str = "0xBCaB99B40692573698dcd288f13Ef59F1Dd691A6";
}

pub mod l3 {
    pub const CONTRACT_ADDRESSES: [&str; 1] = [
        "0xE369C9FA45DDFD44dFaf737EbCe483e8c0D2E402",
    ];

    pub const SLOT_ID_START: &str = "0xe16da923a2d88192e5070f37b4571d58682c0d66212ec634d495f33de3f77ab5";
    pub const PROOF_SIZE: u64 = 2148;
}

pub mod btc {
    pub const BTC_SIGNER_ADDR: &str = "1DiK3RGwdFsxQ45hHR2v4PzGuN6242ekh2";

    pub const BTC_RPC_ENDPOINT: &str = "http://34.142.169.143:18443/";

    pub const BTC_RPC_USERNAME: &str = "trustless";

    pub const BTC_RPC_PASSWORD: &str = "notrespassing";

    pub const SIGNATURE_LENGTH: usize = 65;

    pub const CHECKPOINT_BLOCK_NUMBERS: [u64; 132] = 
    [];
}

pub mod storage {
    /// The path to the initial state file.
    pub const INITAL_STATE_PATH: &str = "InitialState.csv";

    /// The default name of the database.
    pub const DEFAULT_DB_NAME: &str = "db";

    /// The name of the index-to-key database folder.
    pub const INNER_DB_NAME: &str = "inner_db";
}

pub mod zksync {
    /// Bytes in raw L2 to L1 log.
    pub const L2_TO_L1_LOG_SERIALIZE_SIZE: usize = 88;
    // The bitmask by applying which to the compressed state diff metadata we retrieve its operation.
    pub const OPERATION_BITMASK: u8 = 7;
    // The number of bits shifting the compressed state diff metadata by which we retrieve its length.
    pub const LENGTH_BITS_OFFSET: u8 = 3;
    // Size of `CommitBatchInfo.pubdataCommitments` item.
    pub const PUBDATA_COMMITMENT_SIZE: usize = 144;
    // The number of trailing bytes to ignore when using calldata post-blobs. Contains unused blob commitments.
    pub const CALLDATA_SOURCE_TAIL_SIZE: usize = 32;
}
