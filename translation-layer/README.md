# Translation Layer

## Running with docker

First build the docker image (from the root translation layer directory): 

```shell
docker build -t rust-translation-layer -f translation-layer/Dockerfile .
```

Run the server on port 3000:

```shell
docker run -p 3000:3000 rust-translation-layer
``` 

## Running locally

Ensure that you have a local node running in another terminal. 

```shell
cd ../
cargo run --bin sxt-node -- --chain dev --alice --validator
```

Start the translation layer (run from sxt-node/translation-layer):

```shell
RUST_LOG=info cargo run
```

## Fetching OpenApi Spec

```shell
wget -O openapi.json http://localhost:3000/api-docs/openapi.json
```

## Client Generation 

```shell
docker run --rm \
    -v ${PWD}:/local openapitools/openapi-generator-cli generate \
    -i /local/openapi.json \
    -g java \
    -o /local/java-client \
    --library webclient
```