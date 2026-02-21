#!/bin/bash

# Status Network Kubernetes Deployment Script
# This script deploys the Status Network to an EKS cluster

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$(dirname "$SCRIPT_DIR")")"
TERRAFORM_DIR="$SCRIPT_DIR/../terraform"
HELM_DIR="$SCRIPT_DIR/../helm/status-network"

# Default values
NAMESPACE="${NAMESPACE:-status-network}"
RELEASE_NAME="${RELEASE_NAME:-status-network}"
AWS_REGION="${AWS_REGION:-us-east-1}"
VALUES_FILE="${VALUES_FILE:-values.yaml}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

usage() {
    cat <<EOF
Usage: $0 [OPTIONS]

Deploy Status Network to Kubernetes

Options:
    -h, --help              Show this help message
    -n, --namespace NAME    Kubernetes namespace (default: status-network)
    -r, --release NAME      Helm release name (default: status-network)
    --region REGION         AWS region (default: us-east-1)
    --values FILE           Values file to use (default: values.yaml)
    --terraform-only        Only run Terraform (create EKS cluster)
    --helm-only             Only run Helm (assumes cluster exists)
    --destroy               Destroy the deployment
    --dry-run               Show what would be done without making changes

Example:
    $0 --namespace status-testnet --values values-testnet.yaml
    $0 --terraform-only --region us-west-2
    $0 --helm-only
    $0 --destroy
EOF
}

check_dependencies() {
    log_info "Checking dependencies..."

    local deps=(terraform helm kubectl aws)
    for dep in "${deps[@]}"; do
        if ! command -v "$dep" &> /dev/null; then
            log_error "$dep is not installed. Please install it first."
            exit 1
        fi
    done

    log_info "All dependencies are installed"
}

deploy_terraform() {
    log_info "Deploying EKS cluster with Terraform..."

    cd "$TERRAFORM_DIR"

    # Initialize Terraform
    terraform init

    # Plan
    terraform plan -var="aws_region=$AWS_REGION" -out=tfplan

    if [ "$DRY_RUN" = true ]; then
        log_info "Dry run - skipping Terraform apply"
        return
    fi

    # Apply
    terraform apply tfplan

    # Configure kubectl
    log_info "Configuring kubectl..."
    CLUSTER_NAME=$(terraform output -raw cluster_name)
    aws eks update-kubeconfig --region "$AWS_REGION" --name "$CLUSTER_NAME"

    log_info "EKS cluster deployed successfully"
}

deploy_helm() {
    log_info "Deploying Status Network with Helm..."

    cd "$HELM_DIR"

    # Create namespace if it doesn't exist
    if ! kubectl get namespace "$NAMESPACE" &> /dev/null; then
        log_info "Creating namespace $NAMESPACE..."
        kubectl create namespace "$NAMESPACE"
    fi

    # Check if values file exists
    if [ ! -f "$VALUES_FILE" ]; then
        log_error "Values file $VALUES_FILE not found"
        exit 1
    fi

    if [ "$DRY_RUN" = true ]; then
        log_info "Dry run - showing Helm template..."
        helm template "$RELEASE_NAME" . \
            --namespace "$NAMESPACE" \
            --values "$VALUES_FILE" \
            --debug
        return
    fi

    # Install or upgrade the Helm chart
    helm upgrade --install "$RELEASE_NAME" . \
        --namespace "$NAMESPACE" \
        --values "$VALUES_FILE" \
        --wait \
        --timeout 15m

    log_info "Helm deployment completed"
}

wait_for_pods() {
    log_info "Waiting for pods to be ready..."

    local timeout=600
    local start_time=$(date +%s)

    while true; do
        local not_ready=$(kubectl get pods -n "$NAMESPACE" --no-headers 2>/dev/null | grep -v "Running\|Completed" | wc -l)

        if [ "$not_ready" -eq 0 ]; then
            log_info "All pods are ready"
            break
        fi

        local current_time=$(date +%s)
        local elapsed=$((current_time - start_time))

        if [ "$elapsed" -ge "$timeout" ]; then
            log_error "Timeout waiting for pods to be ready"
            kubectl get pods -n "$NAMESPACE"
            exit 1
        fi

        echo -n "."
        sleep 10
    done
}

get_rpc_url() {
    log_info "Getting RPC URL..."

    local rpc_url=$(kubectl get svc l2-rpc-lb -n "$NAMESPACE" -o jsonpath='{.status.loadBalancer.ingress[0].hostname}' 2>/dev/null)

    if [ -z "$rpc_url" ]; then
        rpc_url=$(kubectl get svc l2-rpc-lb -n "$NAMESPACE" -o jsonpath='{.status.loadBalancer.ingress[0].ip}' 2>/dev/null)
    fi

    if [ -n "$rpc_url" ]; then
        echo ""
        log_info "============================================"
        log_info "L2 RPC Endpoint: http://$rpc_url:8545"
        log_info "============================================"
        echo ""
        echo "Test with:"
        echo "  curl http://$rpc_url:8545 -X POST -H 'Content-Type: application/json' -d '{\"jsonrpc\":\"2.0\",\"method\":\"eth_blockNumber\",\"params\":[],\"id\":1}'"
    else
        log_warn "LoadBalancer not ready yet. Run '$SCRIPT_DIR/get-rpc-url.sh' later."
    fi
}

destroy() {
    log_info "Destroying Status Network deployment..."

    read -p "Are you sure you want to destroy the deployment? (y/N) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        log_info "Aborted"
        exit 0
    fi

    # Delete Helm release
    log_info "Deleting Helm release..."
    helm uninstall "$RELEASE_NAME" --namespace "$NAMESPACE" 2>/dev/null || true

    # Delete namespace
    log_info "Deleting namespace..."
    kubectl delete namespace "$NAMESPACE" 2>/dev/null || true

    # Destroy Terraform
    if [ -d "$TERRAFORM_DIR" ]; then
        read -p "Do you also want to destroy the EKS cluster? (y/N) " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            log_info "Destroying EKS cluster..."
            cd "$TERRAFORM_DIR"
            terraform destroy -var="aws_region=$AWS_REGION" -auto-approve
        fi
    fi

    log_info "Destruction completed"
}

# Parse arguments
TERRAFORM_ONLY=false
HELM_ONLY=false
DESTROY=false
DRY_RUN=false

while [[ $# -gt 0 ]]; do
    case $1 in
        -h|--help)
            usage
            exit 0
            ;;
        -n|--namespace)
            NAMESPACE="$2"
            shift 2
            ;;
        -r|--release)
            RELEASE_NAME="$2"
            shift 2
            ;;
        --region)
            AWS_REGION="$2"
            shift 2
            ;;
        --values)
            VALUES_FILE="$2"
            shift 2
            ;;
        --terraform-only)
            TERRAFORM_ONLY=true
            shift
            ;;
        --helm-only)
            HELM_ONLY=true
            shift
            ;;
        --destroy)
            DESTROY=true
            shift
            ;;
        --dry-run)
            DRY_RUN=true
            shift
            ;;
        *)
            log_error "Unknown option: $1"
            usage
            exit 1
            ;;
    esac
done

# Main execution
check_dependencies

if [ "$DESTROY" = true ]; then
    destroy
    exit 0
fi

if [ "$TERRAFORM_ONLY" = true ]; then
    deploy_terraform
    exit 0
fi

if [ "$HELM_ONLY" = true ]; then
    deploy_helm
    wait_for_pods
    get_rpc_url
    exit 0
fi

# Full deployment
deploy_terraform
deploy_helm
wait_for_pods
get_rpc_url

log_info "Deployment completed successfully!"
