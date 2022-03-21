FROM nix-base-docker

COPY target/release/eqs /app/target/release/eqs
RUN chmod +x /app/target/release/eqs
COPY eqs/api /app/eqs/api

COPY target/release/minimal-relayer /app/target/release/minimal-relayer
RUN chmod +x /app/target/release/minimal-relayer

COPY target/release/faucet /app/target/release/faucet
RUN chmod +x /app/target/release/faucet

COPY target/release/address-book /app/target/release/address-book
RUN chmod +x /app/target/release/address-book

COPY target/release/wallet-api /app/target/release/wallet-api
RUN chmod +x /app/target/release/wallet-api
COPY wallet/api /app/wallet/api
COPY wallet/public /app/wallet/public

COPY bin/cape-demo /app/cape-demo

WORKDIR /app/
CMD ./cape-demo

# Address book
EXPOSE 50000
# EQS
EXPOSE 50010
# Relayer
EXPOSE 50020 
# Faucet
EXPOSE 50030 
# Wallet
EXPOSE 50040 
