FROM ubuntu:impish
RUN apt-get update \
  && apt-get install -y libcurl4 \
  && rm -rf /var/lib/apt/lists/*

COPY target/release/wallet-api /app/wallet-api
COPY wallet/api /app/api
COPY wallet/public /app/public
COPY wallet/official_assets/cape_v1_official_assets.lib /.espresso/verified_assets
RUN chmod +x /app/wallet-api

# Point at the Goerli testnet deployment by default; all of these settings can be overridden with
# command line options.
ENV CAPE_EQS_URL=https://eqs.goerli.cape.tech
ENV CAPE_RELAYER_URL=https://relayer.goerli.cape.tech
ENV CAPE_ADDRESS_BOOK_URL=https://address-book.goerli.cape.tech

# Set the storage directory to allow the wallet to access the official assets library.
ENV CAPE_WALLET_STORAGE=/.espresso

WORKDIR /app/
CMD ./wallet-api
EXPOSE 60000
