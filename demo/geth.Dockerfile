FROM ethereum/client-go:v1.10.15
COPY demo/start-geth-docker /start-geth-docker
COPY scratch/geth-data-dir /data
ENTRYPOINT []
CMD ["/bin/sh", "/start-geth-docker"]
EXPOSE 8545
