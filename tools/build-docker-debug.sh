#!/bin/bash
# Build and push KataPulse debug Docker image to harbor registry
# Real-time metrics for Kata Containers. cadvisor-compatible monitoring agent (debug variant).

set -e

IMAGE_NAME="harbor.internal.appbahn.eu/library/kata-pulse:debug"

echo "Building KataPulse debug Docker image: $IMAGE_NAME"
podman build -f Dockerfile.debug -t "$IMAGE_NAME" .

echo "Pushing Docker image to registry..."
podman push "$IMAGE_NAME"

echo "Done! KataPulse debug image built and pushed successfully."
echo "Image: $IMAGE_NAME"
echo ""
echo "To debug:"
echo "  1. Deploy the debug daemonset: kubectl apply -f daemonset.debug.yaml"
echo "  2. Port forward gdbserver: kubectl port-forward pod/kata-pulse-debug-<pod-id> 1339:1339"
echo "  3. Connect with gdb: gdb target remote localhost:1339"
