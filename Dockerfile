FROM nix-base-docker
COPY target/release/web_server /app/web_server
COPY wallet/api /app/api
COPY wallet/public /app/public
RUN chmod +x /app/web_server
WORKDIR /app/
CMD ./web_server
EXPOSE 60000
