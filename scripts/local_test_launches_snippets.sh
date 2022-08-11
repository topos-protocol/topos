
#
# deep but not obtrusive logging
#
export RUST_LOG="debug,libp2p=warn"

#
# 5 nodes - boot node + 4 peers
#

# 12D3KooWPjceQrSwdWXPyLLeABRXmuqt69Rg3sBYbU1Nft9HyQ6X /ip4/127.0.0.1/tcp/30001
cargo run -- --local-key-seed 1 --tce-local-port 30001 --web-api-local-port 8011

# 12D3KooWH3uVF6wv47WnArKHk5p6cvgCJEb74UTmxztmQDc298L3 /ip4/127.0.0.1/tcp/30002
cargo run -- --local-key-seed 2 --tce-local-port 30002 --web-api-local-port 8012 --boot-peers "12D3KooWPjceQrSwdWXPyLLeABRXmuqt69Rg3sBYbU1Nft9HyQ6X /dns4/localhost/tcp/30001"

# 12D3KooWQYhTNQdmr3ArTeUHRYzFg94BKyTkoWBDWez9kSCVe2Xo /ip4/127.0.0.1/tcp/30003
cargo run -- --local-key-seed 3 --tce-local-port 30003 --web-api-local-port 8013 --boot-peers "12D3KooWPjceQrSwdWXPyLLeABRXmuqt69Rg3sBYbU1Nft9HyQ6X /ip4/127.0.0.1/tcp/30001"

# 12D3KooWLJtG8fd2hkQzTn96MrLvThmnNQjTUFZwGEsLRz5EmSzc /ip4/127.0.0.1/tcp/30004
cargo run -- --local-key-seed 4 --tce-local-port 30004 --web-api-local-port 8014 --boot-peers "12D3KooWPjceQrSwdWXPyLLeABRXmuqt69Rg3sBYbU1Nft9HyQ6X /ip4/127.0.0.1/tcp/30001"

# 12D3KooWSHj3RRbBjD15g6wekV8y3mm57Pobmps2g2WJm6F67Lay /ip4/127.0.0.1/tcp/30005
cargo run -- --local-key-seed 5 --tce-local-port 30005 --web-api-local-port 8015 --boot-peers "12D3KooWPjceQrSwdWXPyLLeABRXmuqt69Rg3sBYbU1Nft9HyQ6X /ip4/127.0.0.1/tcp/30001"

#
# posting 3 certificates (to the second peer)
#
curl -X POST http://localhost:8012/certs -H "Content-Type: application/json" -d '{
                                                 "cert" : {
                                                     "id": 1,
                                                     "prev_cert_id": 0,
                                                     "initial_subnet_id": 100,
                                                     "calls": []
                                                 }
                                             }
'

curl -X POST http://localhost:8012/certs -H "Content-Type: application/json" -d '{
                                                 "cert" : {
                                                     "id": 2,
                                                     "prev_cert_id": 1,
                                                     "initial_subnet_id": 100,
                                                     "calls": []
                                                 }
                                             }
'

curl -X POST http://localhost:8012/certs -H "Content-Type: application/json" -d '{
                                                 "cert" : {
                                                     "id": 3,
                                                     "prev_cert_id": 2,
                                                     "initial_subnet_id": 100,
                                                     "calls": []
                                                 }
                                             }
'
