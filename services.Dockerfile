FROM nix-base-docker

#Wait for it convenience script
COPY bin/wait-for-it.sh /bin/wait-for-it.sh
RUN chmod +x /bin/wait-for-it.sh

# EQS
# docker run --workdir /app/eqs -it cape/services /app/eqs/eqs
COPY target/release/eqs /app/eqs/eqs
RUN chmod +x /app/eqs/eqs
COPY eqs/api /app/eqs/api

# Minimal relayer
# docker run -it cape/services /app/relayer/minimal-relayer
COPY target/release/minimal-relayer /app/relayer/minimal-relayer
RUN chmod +x /app/relayer/minimal-relayer

# Faucet
# docker run -it cape/services /app/faucet/faucet
COPY target/release/faucet /app/faucet/faucet
RUN chmod +x /app/faucet/faucet

# Address book
# docker run -it cape/services /app/address-book/address-book
COPY target/release/address-book /app/address-book/address-book
RUN chmod +x /app/address-book/address-book

WORKDIR /app/
