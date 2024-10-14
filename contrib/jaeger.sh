#!/bin/bash
# Script for standing up a local instance of Jaeger, so we can use it for
# running fntests, for example.
#
# Adapted from: https://www.jaegertracing.io/docs/1.6/getting-started/

OTLP_GRPC_PORT=4317
WEB_PORT=16686

docker run -d \
  --name jaeger \
  -e COLLECTOR_ZIPKIN_HTTP_PORT=9411 \
  -e COLLECTOR_OTLP_ENABLED=true \
  -p $OTLP_GRPC_PORT:4317 \
  -p 4318:4318 \
  -p 5775:5775/udp \
  -p 6831:6831/udp \
  -p 6832:6832/udp \
  -p 5778:5778 \
  -p $WEB_PORT:16686 \
  -p 14268:14268 \
  -p 9411:9411 \
  jaegertracing/all-in-one:1.6.0

echo "Set 'export STRATA_OTLP_URL=http://127.0.0.1:$OTLP_GRPC_PORT'."
echo "Open http://localhost:$WEB_PORT/ in your browser."
