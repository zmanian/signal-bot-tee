#!/bin/bash
set -e

# Check for phala CLI
if ! command -v phala &> /dev/null;
then
    echo "Error: phala CLI not found. Install with: npm install -g phala"
    exit 1
fi

# Configuration
APP_NAME=${APP_NAME:-"signal-bot-tee"}
COMPOSE_FILE=${COMPOSE_FILE:-"./docker/phala-compose.yaml"}

echo "Deploying $APP_NAME to Phala Cloud..."
echo "Using compose file: $COMPOSE_FILE"

phala cvms create \
  --name "$APP_NAME" \
  --compose "$COMPOSE_FILE" \
  --vcpu 2 \
  --memory 4096 \
  --disk-size 20

echo ""
echo "Deployment initiated. Check status with: phala cvms list"
