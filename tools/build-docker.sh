#!/bin/bash
# Build and push KataPulse Docker image to harbor registry
# Real-time metrics for Kata Containers. cadvisor-compatible monitoring agent.

set -e

IMAGE_NAME="harbor.internal.appbahn.eu/library/kata-pulse:latest"

echo "Building KataPulse Docker image: $IMAGE_NAME"
podman build -t "$IMAGE_NAME" .

echo "Pushing Docker image to registry..."
podman push "$IMAGE_NAME"

echo "Done! KataPulse image built and pushed successfully."
echo "Image: $IMAGE_NAME"
