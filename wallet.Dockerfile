FROM ubuntu:impish
RUN apt-get update \
  && apt-get install -y libcurl4 \
  && rm -rf /var/lib/apt/lists/*

COPY target/release/wallet-api /app/wallet-api
COPY wallet/api /app/api
COPY wallet/public /app/public
RUN chmod +x /app/wallet-api

# Point at the Goerli testnet deployment by default; all of these settings can be overridden with
# command line options.
ENV CAPE_EQS_URL=https://eqs.goerli.cape.tech
ENV CAPE_RELAYER_URL=https://relayer.goerli.cape.tech
ENV CAPE_ADDRESS_BOOK_URL=https://address-book.goerli.cape.tech
ENV CAPE_WEB3_PROVIDER_URL=https://rpc.goerli.mudit.blog
ENV CAPE_CONTRACT_ADDRESS=0x5594f9F32876d89bB50561DE2051f1F85BA1b6dd

WORKDIR /app/
CMD ./wallet-api
EXPOSE 60000
