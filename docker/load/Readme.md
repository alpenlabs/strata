docker build -t load -f ./docker/load/Dockerfile .


curl http://localhost:8545 -X POST -H "Content-Type: application/json" --data '{
        "method":"eth_getBlockByHash",
        "params":["0x25e3e71be3e4f995720cdb01fa4911b5b00923bd6bb8ba1814616f6a18e49cd2",false],
        "id":1,
        "jsonrpc":"2.0"
}'

- rm -rf logs/ && python3 load_generation.py 
- docker logs strata_prover_client > prover_client.logs