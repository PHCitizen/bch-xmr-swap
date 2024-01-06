# BCH-XMR-SWAP PoC



https://github.com/PHCitizen/bch-xmr-swap/assets/75726261/00596f8d-e98f-4597-8656-d282f12509d5



- SwapLock Contract on video: `bchtest:prf659upqyz96d7l4auuxt567fdrnrr4dyt6ddc8s5`
- SwapLock Tx claim by "Alice": `5d9c13db8c40b2ab29b58a1f480bc90ba0746a7512ed78ceb2467f5084c7193a`


Run client and server with auto-reload on save
```
cargo watch -c -q -w web-server -w protocol  -x "run --bin web-server"
cargo watch -c -q -w client -w protocol  -x "run --bin client"
```

Monero cli/rpc version used 
```
monero-linux-x64-v0.18.3.1.tar.bz2
```

Example regtest for monero development
```
monerod --regtest --offline --fixed-difficulty=1 --rpc-bind-ip=0.0.0.0 --confirm-external-bind 

monero-wallet-rpc --disable-rpc-login --log-level=3 --daemon-address=http://localhost:18081 --untrusted-daemon --confirm-external-bind --rpc-bind-ip=0.0.0.0 --rpc-bind-port=8081 --wallet-dir=wallet_dir --allow-mismatched-daemon-version

monero-wallet-cli --log-level=3 --daemon-address=http://localhost:18081 --untrusted-daemon --allow-mismatched-daemon-version
```

### Mainnet Transactions

> Video are provided at the root of this repository ending it .mp4
> File is >10mb so i cant embed it. You can just download it.

#### Happy Path
- SwapLock -> Alice: 91b9ab4ec54d22b46330c6ba9e5bb07a104513d7d132c2b6b7c48c76c921f40b
- Sweep Alice private key: 76d98630bc1ddd68d42905de1eaa41ae2e024dc75aa0622e1423de130caf0e71
- XMR Shared Address: 43unSddtX9iREffuDzs8gHPvVsu56Bfb4D4RaHjqFyQYd1PPwyJUWKX2ZmX9dxM3kiDq3Ct6mzeYDH6zsJJWjz6vFamaatk
- Alice Keys `.trades/ongoing/eXQBT0jL3e-client.json` TradeId: eXQBT0jL3e
- Bob Keys `.trades/ongoing/eXQBT0jL3e-server.json` TradeId: eXQBT0jL3e

#### Alice Failed to lock XMR
- SwapLock Contract: bitcoincash:pzavf0mxs2kfec8xsj7u8s6pquussw9dgs7mnmknl4
- SwapLock -> Refund: 1604dc533f241c643ad66aa8e64910298c39367dbf8bce1159bdbc5f5bb25e58
- Refund -> Bob Output: 2746ec141696a5b4dafc13eb8ce98ab3d4c4451967480d6d5af4996515241eeb
- Bob Spend: 8482981fd76ce8e8d82c2d299828941070fedebfdc1193edf70c704af5b01922
- Alice Keys `.trades/ongoing/vlKFnqips8-client.json` TradeId: eXQBT0jL3e
- Bob Keys `.trades/ongoing/vlKFnqips8-server.json` TradeId: eXQBT0jL3e