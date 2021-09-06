#!/usr/bin/env bash
DATA_DIR=~/.ethereum/translucence
NETWORK_ID=889
# Remove existing test folder
if [ -d $DATA_DIR ]; then
   echo "Removing existing geth data folder..."
   rm -r $DATA_DIR
fi

mkdir -p $DATA_DIR

# Create accounts
echo "holahola" >> $DATA_DIR/password.txt
geth --verbosity 0 --datadir $DATA_DIR account new --password $DATA_DIR/password.txt

# Generate a new genesis file
node generate_genesis.js

ADDRESS_LIST=$(geth --verbosity 0 --datadir $DATA_DIR account list | cut -d ' ' -f 3 | cut -c2- | rev | cut -c2- | rev | sed -e 's/^/0x/' | sed '$!s/$/,/' | tr -d '\n')

# Create new chain
geth --datadir $DATA_DIR init $DATA_DIR/genesis.json

# Run geth
geth --nousb --networkid $NETWORK_ID --datadir $DATA_DIR  --rpc --rpccorsdomain '*' \
     --rpcport 8545 --rpcapi "admin,txpool,personal,eth,net,web3,debug,miner" \
     --mine --maxpeers 0 --nodiscover \
     --unlock $ADDRESS_LIST \
     --password $DATA_DIR/password.txt \
     --allow-insecure-unlock

