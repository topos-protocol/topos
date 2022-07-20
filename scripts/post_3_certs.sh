#!/bin/zsh

curl -d @cert1.json http://localhost:8012/certs
sleep 5
curl -d @cert2.json http://localhost:8012/certs
sleep 5
curl -d @cert3.json http://localhost:8012/certs
