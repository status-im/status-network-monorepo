#!/usr/bin/env bash
# Migrate RLN user_limit (spam_limit) for all existing users.
#
# When --spam-limit is changed on the prover CLI, existing users retain the old
# value in their stored identity and Merkle tree leaf. This causes proof
# generation to fail with "Message id (N) is not within user_message_limit (M)"
# once the tx counter exceeds the old limit.
#
# This script:
#   1. Scales down the RLN prover
#   2. Runs a dry run to verify what would change
#   3. Asks for confirmation
#   4. Runs the migration for real
#   5. Verifies the result
#   6. Scales the prover back up
#
# Usage:
#   DB_URL=postgres://user:pass@host:5432/prover_db ./scripts/migrate-spam-limit.sh
#
# Environment variables:
#   DB_URL          - PostgreSQL connection URL (REQUIRED)
#   NAMESPACE       - K8s namespace (default: status-network)
#   NEW_LIMIT       - New spam limit (default: 1000000, max: 1048575 due to circuit constraint)
#   TREE_DEPTH      - Merkle tree depth (default: 20)
#   MIGRATE_IMAGE   - Docker image (default: 0xnadeem/migrate-spam-limit:latest)
#   PROVER_DEPLOY   - Prover deployment name (default: rln-prover)
#   SKIP_SCALE      - Set to "true" to skip scaling prover down/up (default: false)

set -euo pipefail

NAMESPACE="${NAMESPACE:-status-network}"
NEW_LIMIT="${NEW_LIMIT:-1000000}"
TREE_DEPTH="${TREE_DEPTH:-20}"
MIGRATE_IMAGE="${MIGRATE_IMAGE:-0xnadeem/migrate-spam-limit:latest}"
PROVER_DEPLOY="${PROVER_DEPLOY:-rln-prover}"
SKIP_SCALE="${SKIP_SCALE:-false}"

# --- Validation ---

if [ -z "${DB_URL:-}" ]; then
  echo "ERROR: DB_URL is required."
  echo ""
  echo "Usage:"
  echo "  DB_URL=postgres://user:pass@host:5432/prover_db $0"
  echo ""
  echo "Environment variables:"
  echo "  DB_URL          PostgreSQL connection URL (REQUIRED)"
  echo "  NAMESPACE       K8s namespace (default: status-network)"
  echo "  NEW_LIMIT       New spam limit (default: 1000000, max: 1048575)"
  echo "  TREE_DEPTH      Merkle tree depth (default: 20)"
  echo "  MIGRATE_IMAGE   Docker image (default: 0xnadeem/migrate-spam-limit:latest)"
  echo "  PROVER_DEPLOY   Prover deployment name (default: rln-prover)"
  echo "  SKIP_SCALE      Skip scaling prover down/up (default: false)"
  exit 1
fi

# Redact password for display
DISPLAY_DB=$(echo "${DB_URL}" | sed -E 's|(://[^:]+:)[^@]+(@)|\1****\2|')

echo "============================================"
echo "  RLN Spam Limit Migration"
echo "============================================"
echo ""
echo "  Namespace:  ${NAMESPACE}"
echo "  DB:         ${DISPLAY_DB}"
echo "  New limit:  ${NEW_LIMIT}"
echo "  Tree depth: ${TREE_DEPTH}"
echo "  Image:      ${MIGRATE_IMAGE}"
echo "  Prover:     ${PROVER_DEPLOY}"
echo ""

# --- Helper to run the migration container ---
run_migrate() {
  local extra_args="${1:-}"
  kubectl -n "${NAMESPACE}" run "migrate-spam-limit-$$" \
    --rm -i \
    --image="${MIGRATE_IMAGE}" \
    --restart=Never \
    --override-type=strategic \
    -- \
    --db "${DB_URL}" \
    --new-limit "${NEW_LIMIT}" \
    --tree-depth "${TREE_DEPTH}" \
    ${extra_args}
}

# --- Step 1: Check kubectl access ---
echo "[1/6] Checking cluster access..."
if ! kubectl -n "${NAMESPACE}" get namespace "${NAMESPACE}" &>/dev/null; then
  echo "ERROR: Cannot access namespace '${NAMESPACE}'. Check your kubeconfig."
  exit 1
fi
echo "  OK"
echo ""

# --- Step 2: Scale down prover ---
if [ "${SKIP_SCALE}" != "true" ]; then
  echo "[2/6] Scaling down ${PROVER_DEPLOY}..."
  CURRENT_REPLICAS=$(kubectl -n "${NAMESPACE}" get deployment "${PROVER_DEPLOY}" -o jsonpath='{.spec.replicas}' 2>/dev/null || echo "0")
  if [ "${CURRENT_REPLICAS}" != "0" ]; then
    kubectl -n "${NAMESPACE}" scale deployment "${PROVER_DEPLOY}" --replicas=0
    echo "  Scaled from ${CURRENT_REPLICAS} to 0. Waiting for pods to terminate..."
    kubectl -n "${NAMESPACE}" rollout status deployment "${PROVER_DEPLOY}" --timeout=60s 2>/dev/null || true
    # Give a moment for DB connections to close
    sleep 2
  else
    echo "  Already at 0 replicas."
  fi
else
  echo "[2/6] Skipping scale-down (SKIP_SCALE=true)"
  CURRENT_REPLICAS="1"
fi
echo ""

# --- Step 3: Dry run ---
echo "[3/6] Running dry run..."
echo ""
DRY_OUTPUT=$(run_migrate "--dry-run" 2>&1) || {
  echo "${DRY_OUTPUT}"
  echo ""
  echo "ERROR: Dry run failed. Scaling prover back up."
  if [ "${SKIP_SCALE}" != "true" ]; then
    kubectl -n "${NAMESPACE}" scale deployment "${PROVER_DEPLOY}" --replicas="${CURRENT_REPLICAS}"
  fi
  exit 1
}
echo "${DRY_OUTPUT}"
echo ""

# --- Step 4: Parse dry run output and confirm ---
WOULD_UPDATE=$(echo "${DRY_OUTPUT}" | grep -c "WOULD update" || true)
ALREADY_OK=$(echo "${DRY_OUTPUT}" | grep -c "already at limit" || true)
ERRORS=$(echo "${DRY_OUTPUT}" | grep -c "FAILED" || true)

echo "  Users to update:       ${WOULD_UPDATE}"
echo "  Already at new limit:  ${ALREADY_OK}"
echo "  Errors:                ${ERRORS}"
echo ""

if [ "${ERRORS}" -gt 0 ]; then
  echo "ERROR: ${ERRORS} users failed deserialization in dry run. Aborting."
  if [ "${SKIP_SCALE}" != "true" ]; then
    kubectl -n "${NAMESPACE}" scale deployment "${PROVER_DEPLOY}" --replicas="${CURRENT_REPLICAS}"
  fi
  exit 1
fi

if [ "${WOULD_UPDATE}" -eq 0 ]; then
  echo "Nothing to migrate — all users already at the correct limit."
  if [ "${SKIP_SCALE}" != "true" ] && [ "${CURRENT_REPLICAS}" != "0" ]; then
    echo "Scaling prover back up..."
    kubectl -n "${NAMESPACE}" scale deployment "${PROVER_DEPLOY}" --replicas="${CURRENT_REPLICAS}"
  fi
  exit 0
fi

read -p "Proceed with migration of ${WOULD_UPDATE} users? [y/N] " -n 1 -r
echo ""
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
  echo "Aborted. Scaling prover back up."
  if [ "${SKIP_SCALE}" != "true" ]; then
    kubectl -n "${NAMESPACE}" scale deployment "${PROVER_DEPLOY}" --replicas="${CURRENT_REPLICAS}"
  fi
  exit 0
fi

# --- Step 5: Run migration for real ---
echo ""
echo "[4/6] Running migration..."
echo ""
MIGRATE_OUTPUT=$(run_migrate "" 2>&1) || {
  echo "${MIGRATE_OUTPUT}"
  echo ""
  echo "ERROR: Migration failed! Check output above."
  echo "The prover has NOT been scaled back up. Investigate before restarting."
  exit 1
}
echo "${MIGRATE_OUTPUT}"
echo ""

# --- Step 6: Verify ---
echo "[5/6] Verifying migration..."
echo ""
VERIFY_OUTPUT=$(run_migrate "--dry-run" 2>&1) || {
  echo "${VERIFY_OUTPUT}"
  echo ""
  echo "WARNING: Verification run failed. Check manually."
  exit 1
}

STILL_NEED_UPDATE=$(echo "${VERIFY_OUTPUT}" | grep -c "WOULD update" || true)
NOW_OK=$(echo "${VERIFY_OUTPUT}" | grep -c "already at limit" || true)

if [ "${STILL_NEED_UPDATE}" -gt 0 ]; then
  echo "  WARNING: ${STILL_NEED_UPDATE} users still need updating!"
  echo "${VERIFY_OUTPUT}"
  echo ""
  echo "Migration may have partially failed. Investigate before restarting the prover."
  exit 1
fi

echo "  All ${NOW_OK} users verified at limit ${NEW_LIMIT}."
echo ""

# --- Step 7: Scale prover back up ---
if [ "${SKIP_SCALE}" != "true" ] && [ "${CURRENT_REPLICAS}" != "0" ]; then
  echo "[6/6] Scaling ${PROVER_DEPLOY} back up to ${CURRENT_REPLICAS} replicas..."
  kubectl -n "${NAMESPACE}" scale deployment "${PROVER_DEPLOY}" --replicas="${CURRENT_REPLICAS}"
  echo "  Done."
else
  echo "[6/6] Skipping scale-up."
fi

echo ""
echo "============================================"
echo "  Migration complete!"
echo ""
echo "  Make sure the prover has --spam-limit ${NEW_LIMIT}"
echo "  in its deployment args."
echo "============================================"
