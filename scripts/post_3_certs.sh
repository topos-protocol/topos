#!/bin/zsh

echo "1-st"
curl -d @cert1.json http://localhost:8012/certs
echo ""
sleep 5

echo "2-nd"
curl -d @cert2.json http://localhost:8012/certs
echo ""
sleep 5

echo "3-rd"
curl -d @cert3.json http://localhost:8012/certs
echo ""
