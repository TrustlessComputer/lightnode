# Supersonic State Reconstruction Tool

Step 1: Check out the `release/mainnet` branch

Step 2: Build
```
cargo build --release
```

Step 3: Run
```
./target/release/state-reconstruct reconstruct l1 --btc-url <BITCOIN_RPC_URL> --da-url <POLYGON_DA_URL>
```

Note: Please replace `<BITCOIN_RPC_URL>` and `<POLYGON_DA_URL>` with your respective RPC URLs. You can create free accounts on Quicknode and Infura to obtain these.
