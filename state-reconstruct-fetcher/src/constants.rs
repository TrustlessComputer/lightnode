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

    pub const DA_URL: &str = "https://rpc-amoy.polygon.technology/";

    pub const HTTP_URL: &str = "https://rpc-amoy.polygon.technology/";

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

    pub const CHECKPOINT_BLOCK_NUMBERS: [u64; 153] = 
    [1346491, 1346494, 1346496, 1346498, 1346500, 1346502, 1346752, 1346754, 1346810, 1346889,
    1346928, 1355885, 1355978, 1356318, 1356491, 1356798, 1356847, 1356887, 1359278, 1359345,
    1359389, 1359502, 1359552, 1359579, 1359634, 1359665, 1365917, 1365919, 1365922, 1365924,
    1365928, 1365930, 1365933, 1365935, 1365937, 1365939, 1365942, 1365944, 1365946, 1365948,
    1365951, 1365953, 1365955, 1365957, 1365959, 1365962, 1365964, 1366251, 1366253, 1366256,
    1366258, 1366260, 1366262, 1366264, 1366267, 1366269, 1366271, 1366273, 1366276, 1366278,
    1366280, 1366282, 1366285, 1366287, 1366289, 1366291, 1366293, 1366327, 1366400, 1366431,
    1369299, 1369461, 1369463, 1369465, 1369468, 1369510, 1378566, 1378595, 1379303, 1379336,
    1379384, 1379428, 1379463, 1379508, 1379562, 1379831, 1379854, 1380015, 1380061, 1380098,
    1380138, 1380161, 1380195, 1380566, 1380590, 1381789, 1381829, 1381861, 1381892, 1381921,
    1381993, 1382027, 1382191, 1382221, 1382281, 1382314, 1382356, 1382408, 1382441, 1382478,
    1382505, 1382546, 1382570, 1382612, 1382641, 1382676, 1382719, 1382755, 1382777, 1382806,
    1383253, 1383288, 1383410, 1383435, 1383550, 1383573, 1385093, 1385138, 1385300, 1385339,
    1385381, 1385409, 1388824, 1388826, 1388857, 1389020, 1389136, 1389180, 1389215, 1389523,
    1390846, 1390901, 1390959, 1390996, 1391031, 1391429, 1391477, 1391525, 1391568, 1391601,
    1391638, 1391733, 1401063,
    ];	
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
