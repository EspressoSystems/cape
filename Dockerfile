FROM ubuntu:impish
RUN apt-get update \
  && apt-get install -y libcurl4 \
  && rm -rf /var/lib/apt/lists/*
COPY target/release/web_server /app/web_server
COPY wallet/api /app/api
COPY wallet/public /app/public
RUN chmod +x /app/web_server
WORKDIR /app/
CMD ./web_server
EXPOSE 60000
