FROM ubuntu:jammy
RUN apt-get update \
  && apt-get install -y libcurl4 \
  && rm -rf /var/lib/apt/lists/*

COPY target/release/wallet-api /app/wallet-api
COPY target/release/wallet-cli /app/wallet-cli
COPY wallet/api /app/api
COPY wallet/public /app/public
COPY wallet/official_assets/cape_v2_official_assets.lib /.espresso/verified_assets
COPY bin/wait-for-it.sh /bin/wait-for-it.sh
RUN chmod +x /app/wallet-api
RUN chmod +x /app/wallet-cli
RUN chmod +x /bin/wait-for-it.sh


ENV CAPE_WALLET_ASSET_LIBRARY_VERIFIER_KEY=SCHNORRVERKEY~P-ZcYMUYtJ6O5UTpIeBCvfqekOVD_3i2PSEkD8feUJdp

# Point at the Goerli testnet deployment by default; all of these settings can be overridden with
# command line options.
ENV CAPE_EQS_URL=https://eqs.arbitrum-goerli.cape.tech
ENV CAPE_RELAYER_URL=https://relayer.arbitrum-goerli.cape.tech
ENV CAPE_ADDRESS_BOOK_URL=https://address-book.arbitrum-goerli.cape.tech

# Set the storage directory to allow the wallet to access the official assets library.
ENV CAPE_WALLET_STORAGE=/.espresso

WORKDIR /app/
CMD ./wallet-api
EXPOSE 60000
