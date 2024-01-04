BCH-XMR-SWAP POC



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


