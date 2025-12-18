#!/bin/bash
set -e

# Configuration
SIGNAL_API_URL=${SIGNAL_API_URL:-"http://localhost:8080"}

show_help() {
    echo "Usage: $0 [command] [phone_number]"
    echo ""
    echo "Commands:"
    echo "  register <phone>      - Request SMS verification code"
    echo "  verify <phone> <core> - Verify SMS code"
    echo "  profile <phone> <name> - Set profile name"
    echo "  accounts              - List registered accounts"
}

if [ -z "$1" ]; then
    show_help
    exit 1
fi

case "$1" in
    register)
        if [ -z "$2" ]; then echo "Phone number required"; exit 1; fi
        echo "Requesting registration for $2..."
        curl -X POST "$SIGNAL_API_URL/v1/register/$2"
        echo -e "\nIf successful, you will receive an SMS code."
        ;;
    verify)
        if [ -z "$2" ] || [ -z "$3" ]; then echo "Phone and code required"; exit 1; fi
        echo "Verifying code $3 for $2..."
        curl -X POST "$SIGNAL_API_URL/v1/register/$2/verify/$3"
        echo -e "\nAccount verified."
        ;;
    profile)
        if [ -z "$2" ] || [ -z "$3" ]; then echo "Phone and name required"; exit 1; fi
        echo "Setting profile for $2 to '$3'..."
        curl -X PUT "$SIGNAL_API_URL/v1/profiles/$2" \
            -H "Content-Type: application/json" \
            -d "{\"name\": \"$3\"}"
        echo -e "\nProfile updated."
        ;;
    accounts)
        echo "Registered accounts:"
        curl -s "$SIGNAL_API_URL/v1/accounts" | jq .
        ;;    *)
        show_help
        exit 1
        ;;esac
