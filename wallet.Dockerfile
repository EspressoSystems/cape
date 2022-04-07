FROM nix-base-docker
COPY target/release/wallet-api /app/wallet-api
COPY wallet/api /app/api
COPY wallet/public /app/public
RUN chmod +x /app/wallet-api

# Point at the test deployment by default; all of these settings can be overridden with command line
# options.
ENV CAPE_EQS_URL=https://eqs.test.cape.tech
ENV CAPE_RELAYER_URL=https://relayer.test.cape.tech
ENV CAPE_ADDRESS_BOOK_URL=https://address-book.test.cape.tech
ENV CAPE_WEB3_PROVIDER_URL=https://geth.test.cape.tech
ENV CAPE_CONTRACT_ADDRESS=0xCf7Ed3AccA5a467e9e704C703E8D87F634fB0Fc9

WORKDIR /app/
CMD ./wallet-api
EXPOSE 60000
