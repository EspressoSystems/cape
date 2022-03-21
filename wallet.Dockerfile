FROM nix-base-docker
COPY target/release/wallet-api /app/wallet-api
COPY wallet/api /app/api
COPY wallet/public /app/public
RUN chmod +x /app/wallet-api
WORKDIR /app/
CMD ./wallet-api
EXPOSE 60000
