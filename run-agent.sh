#!/bin/bash
# Run kore-agent in a sandboxed Docker container

set -e

# Build the image
echo "Building kore-agent container..."
docker build -t kore-agent .

# Run with minimal privileges
echo "Starting kore-agent..."
docker run -it --rm \
    --name kore-agent \
    --read-only \
    --cap-drop=ALL \
    --security-opt=no-new-privileges \
    --network=bridge \
    -e ANTHROPIC_API_KEY="${ANTHROPIC_API_KEY}" \
    kore-agent

# Security notes:
# --read-only: Filesystem is read-only
# --cap-drop=ALL: No Linux capabilities
# --security-opt=no-new-privileges: Cannot gain privileges
# --network=bridge: Only outbound internet, no host network
# No -v mounts: No access to host filesystem
# No --privileged: No hardware access
# No --device: No device access
